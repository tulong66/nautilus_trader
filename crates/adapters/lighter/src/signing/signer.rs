// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! Pure Rust Lighter transaction signer.
//!
//! This module provides signing functionality using `goldilocks-crypto` and `poseidon-hash`
//! crates, without requiring FFI bindings to the lighter-go library.
//!
//! # Cryptographic Stack
//!
//! - Schnorr signatures over ECgFp5 curve
//! - Poseidon2 hash function
//! - Goldilocks field (p = 2^64 - 2^32 + 1)
//!
//! # Transaction Types
//!
//! - CREATE_ORDER (14): 16 Goldilocks elements
//! - CANCEL_ORDER (15): 8 Goldilocks elements
//! - CANCEL_ALL_ORDERS (16): 8 Goldilocks elements
//!
//! # Implementation Reference
//!
//! Based on the lighter-rust SDK: <https://github.com/Bvvvp009/lighter-rust>

use poseidon_hash::{Goldilocks, Fp5Element, hash_to_quintic_extension};
use thiserror::Error;
use tracing::debug;

use super::nonce::NonceManager;

// ============================================================================
// Helper functions for Goldilocks field element conversion
// ============================================================================

/// Convert a signed i64 to a Goldilocks field element.
///
/// Handles negative values by wrapping around the Goldilocks modulus.
/// Based on lighter-rust SDK implementation.
#[inline]
fn to_goldi_i64(value: i64) -> Goldilocks {
    if value >= 0 {
        Goldilocks::from_canonical_u64(value as u64)
    } else {
        // For negative values, we need to compute (modulus + value) mod modulus
        // Goldilocks modulus p = 2^64 - 2^32 + 1 = 0xFFFFFFFF00000001
        const GOLDILOCKS_MODULUS: u64 = 0xFFFF_FFFF_0000_0001;
        let positive_repr = GOLDILOCKS_MODULUS.wrapping_add(value as u64);
        Goldilocks::from_canonical_u64(positive_repr)
    }
}

/// Convert an Fp5Element (quintic extension) to 40 bytes.
///
/// The Fp5Element contains 5 Goldilocks limbs, each 8 bytes.
#[inline]
fn fp5_to_bytes(fp5: &Fp5Element) -> [u8; 40] {
    let mut result = [0u8; 40];
    for (i, limb) in fp5.0.iter().enumerate() {
        let limb_bytes = limb.to_canonical_u64().to_le_bytes();
        result[i * 8..(i + 1) * 8].copy_from_slice(&limb_bytes);
    }
    result
}

/// Signing error types.
#[derive(Debug, Error)]
pub enum SigningError {
    /// Invalid private key format or length.
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    /// Signing operation failed.
    #[error("Signing failed: {0}")]
    SigningFailed(String),

    /// Invalid message format.
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Hex decoding error.
    #[error("Hex decode error: {0}")]
    HexDecode(String),
}

/// A signed transaction ready for submission.
#[derive(Debug, Clone)]
pub struct SignedTransaction {
    /// The hex-encoded signature (160 chars = 80 bytes).
    pub signature: String,
    /// The hash of the signed message.
    pub signed_hash: String,
    /// The nonce used for this transaction.
    pub nonce: u64,
}

/// Lighter transaction signer using pure Rust implementation.
///
/// Uses Schnorr signatures over ECgFp5 curve with Poseidon2 hash.
#[derive(Debug)]
pub struct LighterSigner {
    /// Private key bytes (40 bytes = 5 Goldilocks field elements).
    private_key: [u8; 40],
    /// Account index on Lighter.
    account_index: i64,
    /// API key index (2-254).
    api_key_index: u8,
    /// Chain ID (300=testnet, 304=mainnet).
    chain_id: u32,
    /// Nonce manager for transaction ordering.
    pub nonce_manager: NonceManager,
}

impl LighterSigner {
    /// Chain ID for testnet.
    pub const CHAIN_ID_TESTNET: u32 = 300;
    /// Chain ID for mainnet.
    pub const CHAIN_ID_MAINNET: u32 = 304;

    /// Order type: Limit.
    pub const ORDER_TYPE_LIMIT: u8 = 0;
    /// Order type: Market.
    pub const ORDER_TYPE_MARKET: u8 = 1;

    /// Time in force: Good till time (default 28 days).
    pub const TIF_GOOD_TILL_TIME: u8 = 0;
    /// Time in force: Immediate or cancel.
    pub const TIF_IMMEDIATE_OR_CANCEL: u8 = 1;
    /// Time in force: Fill or kill.
    pub const TIF_FILL_OR_KILL: u8 = 2;
    /// Time in force: Post only (maker only).
    pub const TIF_POST_ONLY: u8 = 3;

    /// Transaction type: Create order.
    pub const TX_TYPE_CREATE_ORDER: u8 = 14;
    /// Transaction type: Cancel order.
    pub const TX_TYPE_CANCEL_ORDER: u8 = 15;
    /// Transaction type: Cancel all orders.
    pub const TX_TYPE_CANCEL_ALL_ORDERS: u8 = 16;

    #[must_use]
    pub const fn strategy_signing_surface() -> [&'static str; 4] {
        [
            "create_auth_token",
            "sign_create_order",
            "sign_cancel_order",
            "sign_cancel_all_orders",
        ]
    }

    /// Create a new signer with pure Rust implementation.
    ///
    /// # Arguments
    /// * `private_key` - 40-byte API key as hex string (80 chars, with or without 0x prefix)
    /// * `chain_id` - Chain ID (300 for testnet, 304 for mainnet)
    /// * `api_key_index` - API key index (2-254)
    /// * `account_index` - Account index on Lighter
    /// * `initial_nonce` - Initial nonce value (fetch from server)
    ///
    /// # Errors
    /// Returns error if private key format is invalid.
    pub fn new(
        private_key: &str,
        chain_id: u32,
        api_key_index: u8,
        account_index: i64,
        initial_nonce: u64,
    ) -> Result<Self, SigningError> {
        // Parse private key from hex
        let key_hex = private_key.strip_prefix("0x").unwrap_or(private_key);

        if key_hex.len() != 80 {
            return Err(SigningError::InvalidPrivateKey(format!(
                "Expected 80 hex characters, got {}",
                key_hex.len()
            )));
        }

        let key_bytes = hex::decode(key_hex)
            .map_err(|e| SigningError::HexDecode(e.to_string()))?;

        let mut private_key_array = [0u8; 40];
        private_key_array.copy_from_slice(&key_bytes);

        debug!(
            "LighterSigner created: account={}, api_key_index={}, chain_id={}, initial_nonce={}",
            account_index, api_key_index, chain_id, initial_nonce
        );

        Ok(Self {
            private_key: private_key_array,
            account_index,
            api_key_index,
            chain_id,
            nonce_manager: NonceManager::new(initial_nonce, account_index as u32, api_key_index),
        })
    }

    /// Get the API key index.
    #[must_use]
    pub const fn api_key_index(&self) -> u8 {
        self.api_key_index
    }

    /// Get the account index.
    #[must_use]
    pub const fn account_index(&self) -> i64 {
        self.account_index
    }

    /// Get the chain ID.
    #[must_use]
    pub const fn chain_id(&self) -> u32 {
        self.chain_id
    }

    /// Get the current nonce.
    #[must_use]
    pub fn current_nonce(&self) -> u64 {
        self.nonce_manager.current()
    }

    /// Sign a raw 40-byte message.
    ///
    /// This is the low-level signing function that uses Schnorr signatures
    /// over the ECgFp5 curve with Poseidon2 hashing.
    ///
    /// # Arguments
    /// * `message` - 40-byte message to sign
    ///
    /// # Returns
    /// 80-byte signature (s: 40 bytes, e: 40 bytes)
    ///
    /// # Errors
    /// Returns error if signing fails.
    pub fn sign(&self, message: &[u8; 40]) -> Result<[u8; 80], SigningError> {
        // Get the current nonce and convert to bytes
        let nonce = self.nonce_manager.current();

        // Convert nonce to 40-byte array (little-endian)
        let mut nonce_bytes = [0u8; 40];
        nonce_bytes[..8].copy_from_slice(&nonce.to_le_bytes());

        // Use goldilocks-crypto to sign with Schnorr + Poseidon2
        let signature_vec = goldilocks_crypto::schnorr::sign_with_nonce(
            &self.private_key,
            message,
            &nonce_bytes,
        )
        .map_err(|e| SigningError::SigningFailed(format!("Schnorr signing failed: {}", e)))?;

        // Convert Vec<u8> to [u8; 80]
        if signature_vec.len() != 80 {
            return Err(SigningError::SigningFailed(format!(
                "Invalid signature length: expected 80, got {}",
                signature_vec.len()
            )));
        }

        let mut signature = [0u8; 80];
        signature.copy_from_slice(&signature_vec);

        debug!(
            "Signed message with nonce={}, sig_len={}",
            nonce,
            signature.len()
        );

        Ok(signature)
    }

    /// Verify a signature against a message and public key.
    ///
    /// This is primarily used for testing and debugging.
    ///
    /// # Arguments
    /// * `signature` - 80-byte signature to verify
    /// * `message` - 40-byte message that was signed
    /// * `public_key` - 40-byte public key
    ///
    /// # Returns
    /// `true` if the signature is valid, `false` otherwise
    ///
    /// # Errors
    /// Returns error if verification fails due to invalid input formats.
    pub fn verify(
        signature: &[u8; 80],
        message: &[u8; 40],
        public_key: &[u8; 40],
    ) -> Result<bool, SigningError> {
        goldilocks_crypto::schnorr::verify_signature(signature, message, public_key)
            .map_err(|e| SigningError::SigningFailed(format!("Verification failed: {}", e)))
    }

    /// Create an authentication token for WebSocket/API authentication.
    ///
    /// Uses Poseidon2 hash over Goldilocks field elements as per Lighter specification.
    ///
    /// # Arguments
    /// * `deadline` - Token expiry timestamp in seconds (Unix timestamp)
    ///
    /// # Returns
    /// Auth token string in format: "deadline:account_index:api_key_index:signature_hex"
    ///
    /// # Errors
    /// Returns error if signing fails.
    pub fn create_auth_token(&self, deadline: i64) -> Result<String, SigningError> {
        // Auth data format: "deadline:account_index:api_key_index"
        let auth_data = format!("{}:{}:{}", deadline, self.account_index, self.api_key_index);
        let auth_bytes = auth_data.as_bytes();

        // Convert auth_data bytes to Goldilocks field elements (8-byte chunks)
        let mut elements = Vec::new();
        let mut i = 0;
        while i < auth_bytes.len() {
            let next_start = (i + 8).min(auth_bytes.len());
            let chunk = &auth_bytes[i..next_start];
            let mut bytes = [0u8; 8];
            bytes[..chunk.len()].copy_from_slice(chunk);
            let val = u64::from_le_bytes(bytes);
            elements.push(Goldilocks::from_canonical_u64(val));
            i = next_start;
        }

        // Hash with Poseidon2 to get Fp5Element (quintic extension)
        let hash_fp5 = hash_to_quintic_extension(&elements);

        // Convert Fp5Element to 40 bytes (5 * 8 bytes)
        let hash_limbs = hash_fp5.0; // Access the [Goldilocks; 5] array
        let mut message = [0u8; 40];
        for (i, limb) in hash_limbs.iter().enumerate() {
            let limb_bytes = limb.to_canonical_u64().to_le_bytes();
            message[i * 8..(i + 1) * 8].copy_from_slice(&limb_bytes);
        }

        let signature = self.sign(&message)?;
        let signature_hex = hex::encode(signature);

        debug!(
            "Created auth token: deadline={}, hash_len={}, sig_len={}",
            deadline,
            message.len(),
            signature_hex.len()
        );

        Ok(format!("{}:{}", auth_data, signature_hex))
    }

    /// Sign a create order transaction.
    ///
    /// Uses Poseidon2 hash over 16 Goldilocks field elements as per Lighter specification.
    ///
    /// # Arguments
    /// * `market_index` - Market ID
    /// * `client_order_index` - Client-side order ID
    /// * `base_amount` - Order size in base units (scaled integer)
    /// * `price` - Order price (scaled integer)
    /// * `is_ask` - true for sell, false for buy
    /// * `order_type` - Order type (0=limit, 1=market)
    /// * `time_in_force` - TIF (0=GoodTillTime, 1=IOC, 2=FOK, 3=PostOnly)
    /// * `reduce_only` - Whether reduce-only
    /// * `trigger_price` - Trigger price for stop orders (0 for non-stop orders)
    /// * `order_expiry` - Order expiry timestamp in seconds (0 for no expiry)
    /// * `expired_at` - Transaction expiry timestamp in milliseconds
    ///
    /// # Returns
    /// Tuple of (SignedTransaction, nonce_used)
    ///
    /// # Errors
    /// Returns error if signing fails. Nonce is automatically rolled back on error.
    #[allow(clippy::too_many_arguments)]
    pub fn sign_create_order(
        &self,
        market_index: u16,
        client_order_index: i64,
        base_amount: i64,
        price: u32,
        is_ask: bool,
        order_type: u8,
        time_in_force: u8,
        reduce_only: bool,
        trigger_price: u32,
        order_expiry: i64,
        expired_at: i64,
    ) -> Result<(SignedTransaction, u64), SigningError> {
        let nonce = self.nonce_manager.next();

        // Build 16 Goldilocks elements for CREATE_ORDER (tx_type=14)
        // Based on lighter-rust SDK specification
        let elements = vec![
            Goldilocks::from_canonical_u64(self.chain_id as u64),           // 1. chain_id
            Goldilocks::from_canonical_u64(Self::TX_TYPE_CREATE_ORDER as u64), // 2. tx_type (14)
            to_goldi_i64(nonce as i64),                                      // 3. nonce
            to_goldi_i64(expired_at),                                        // 4. expired_at
            to_goldi_i64(self.account_index),                                // 5. account_index
            Goldilocks::from_canonical_u64(self.api_key_index as u64),      // 6. api_key_index
            Goldilocks::from_canonical_u64(market_index as u64),            // 7. market_index
            to_goldi_i64(client_order_index),                                // 8. client_order_index
            to_goldi_i64(base_amount),                                       // 9. base_amount
            Goldilocks::from_canonical_u64(price as u64),                   // 10. price
            Goldilocks::from_canonical_u64(is_ask as u64),                  // 11. is_ask
            Goldilocks::from_canonical_u64(order_type as u64),              // 12. order_type
            Goldilocks::from_canonical_u64(time_in_force as u64),           // 13. time_in_force
            Goldilocks::from_canonical_u64(reduce_only as u64),             // 14. reduce_only
            Goldilocks::from_canonical_u64(trigger_price as u64),           // 15. trigger_price
            to_goldi_i64(order_expiry),                                      // 16. order_expiry
        ];

        // Hash with Poseidon2 to get 40-byte message
        let hash_fp5 = hash_to_quintic_extension(&elements);
        let message = fp5_to_bytes(&hash_fp5);

        match self.sign(&message) {
            Ok(sig) => {
                let tx = SignedTransaction {
                    signature: hex::encode(sig),
                    signed_hash: hex::encode(&message),
                    nonce,
                };
                debug!(
                    "Signed create order: market={}, nonce={}, elements=16",
                    market_index, nonce
                );
                Ok((tx, nonce))
            }
            Err(e) => {
                self.nonce_manager.rollback();
                Err(e)
            }
        }
    }

    /// Sign a cancel order transaction.
    ///
    /// Uses Poseidon2 hash over 8 Goldilocks field elements as per Lighter specification.
    ///
    /// # Arguments
    /// * `market_index` - Market ID
    /// * `order_index` - Exchange order ID to cancel
    /// * `expired_at` - Transaction expiry timestamp in milliseconds
    ///
    /// # Returns
    /// Tuple of (SignedTransaction, nonce_used)
    ///
    /// # Errors
    /// Returns error if signing fails. Nonce is automatically rolled back on error.
    pub fn sign_cancel_order(
        &self,
        market_index: u16,
        order_index: i64,
        expired_at: i64,
    ) -> Result<(SignedTransaction, u64), SigningError> {
        let nonce = self.nonce_manager.next();

        // Build 8 Goldilocks elements for CANCEL_ORDER (tx_type=15)
        // Based on lighter-rust SDK specification
        let elements = vec![
            Goldilocks::from_canonical_u64(self.chain_id as u64),           // 1. chain_id
            Goldilocks::from_canonical_u64(Self::TX_TYPE_CANCEL_ORDER as u64), // 2. tx_type (15)
            to_goldi_i64(nonce as i64),                                      // 3. nonce
            to_goldi_i64(expired_at),                                        // 4. expired_at
            to_goldi_i64(self.account_index),                                // 5. account_index
            Goldilocks::from_canonical_u64(self.api_key_index as u64),      // 6. api_key_index
            Goldilocks::from_canonical_u64(market_index as u64),            // 7. market_index
            to_goldi_i64(order_index),                                       // 8. order_index
        ];

        // Hash with Poseidon2 to get 40-byte message
        let hash_fp5 = hash_to_quintic_extension(&elements);
        let message = fp5_to_bytes(&hash_fp5);

        match self.sign(&message) {
            Ok(sig) => {
                let tx = SignedTransaction {
                    signature: hex::encode(sig),
                    signed_hash: hex::encode(&message),
                    nonce,
                };
                debug!(
                    "Signed cancel order: market={}, order={}, nonce={}, elements=8",
                    market_index, order_index, nonce
                );
                Ok((tx, nonce))
            }
            Err(e) => {
                self.nonce_manager.rollback();
                Err(e)
            }
        }
    }

    /// Sign a cancel all orders transaction.
    ///
    /// Uses Poseidon2 hash over 8 Goldilocks field elements as per Lighter specification.
    ///
    /// # Arguments
    /// * `time_in_force` - Filter by TIF (0 for all)
    /// * `time` - Current timestamp in milliseconds
    /// * `expired_at` - Transaction expiry timestamp in milliseconds
    ///
    /// # Returns
    /// Tuple of (SignedTransaction, nonce_used)
    ///
    /// # Errors
    /// Returns error if signing fails. Nonce is automatically rolled back on error.
    pub fn sign_cancel_all_orders(
        &self,
        time_in_force: u8,
        time: i64,
        expired_at: i64,
    ) -> Result<(SignedTransaction, u64), SigningError> {
        let nonce = self.nonce_manager.next();

        // Build 8 Goldilocks elements for CANCEL_ALL_ORDERS (tx_type=16)
        // Based on lighter-rust SDK specification
        let elements = vec![
            Goldilocks::from_canonical_u64(self.chain_id as u64),               // 1. chain_id
            Goldilocks::from_canonical_u64(Self::TX_TYPE_CANCEL_ALL_ORDERS as u64), // 2. tx_type (16)
            to_goldi_i64(nonce as i64),                                          // 3. nonce
            to_goldi_i64(expired_at),                                            // 4. expired_at
            to_goldi_i64(self.account_index),                                    // 5. account_index
            Goldilocks::from_canonical_u64(self.api_key_index as u64),          // 6. api_key_index
            Goldilocks::from_canonical_u64(time_in_force as u64),               // 7. time_in_force
            to_goldi_i64(time),                                                  // 8. time
        ];

        // Hash with Poseidon2 to get 40-byte message
        let hash_fp5 = hash_to_quintic_extension(&elements);
        let message = fp5_to_bytes(&hash_fp5);

        match self.sign(&message) {
            Ok(sig) => {
                let tx = SignedTransaction {
                    signature: hex::encode(sig),
                    signed_hash: hex::encode(&message),
                    nonce,
                };
                debug!("Signed cancel all orders: nonce={}, elements=8", nonce);
                Ok((tx, nonce))
            }
            Err(e) => {
                self.nonce_manager.rollback();
                Err(e)
            }
        }
    }

    /// Manually rollback the nonce.
    ///
    /// Use this after a transaction fails at the network/API level.
    pub fn rollback_nonce(&self) {
        self.nonce_manager.rollback();
    }

    /// Reset the nonce to a specific value.
    ///
    /// Use this for error recovery when syncing with the server.
    pub fn reset_nonce(&self, server_nonce: u64) {
        self.nonce_manager.reset(server_nonce);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 80 hex chars = 40 bytes
    const TEST_PRIVATE_KEY: &str = "00000000000000000000000000000000000000000000000000000000000000000000000000000001";

    #[test]
    fn test_signer_constants() {
        assert_eq!(LighterSigner::CHAIN_ID_TESTNET, 300);
        assert_eq!(LighterSigner::CHAIN_ID_MAINNET, 304);
        assert_eq!(LighterSigner::ORDER_TYPE_LIMIT, 0);
        assert_eq!(LighterSigner::ORDER_TYPE_MARKET, 1);
        assert_eq!(LighterSigner::TIF_GOOD_TILL_TIME, 0);
        assert_eq!(LighterSigner::TIF_IMMEDIATE_OR_CANCEL, 1);
        assert_eq!(LighterSigner::TIF_FILL_OR_KILL, 2);
        assert_eq!(LighterSigner::TIF_POST_ONLY, 3);
    }

    #[test]
    fn test_signer_new() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        );
        assert!(signer.is_ok());

        let signer = signer.unwrap();
        assert_eq!(signer.chain_id(), 300);
        assert_eq!(signer.api_key_index(), 2);
        assert_eq!(signer.account_index(), 878);
        assert_eq!(signer.current_nonce(), 100);
    }

    #[test]
    fn test_signer_invalid_key_length() {
        let result = LighterSigner::new(
            "0123456789", // Too short
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_create_order() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        )
        .unwrap();

        let expired_at = 1734200000000i64; // Transaction expiry
        let result = signer.sign_create_order(
            1,     // market_index
            12345, // client_order_index
            1000,  // base_amount
            50000, // price
            false, // is_ask (buy)
            LighterSigner::ORDER_TYPE_LIMIT,
            LighterSigner::TIF_GOOD_TILL_TIME,
            false,      // reduce_only
            0,          // trigger_price (0 for non-stop orders)
            0,          // order_expiry (0 for no expiry)
            expired_at, // expired_at
        );

        assert!(result.is_ok());
        let (tx, nonce) = result.unwrap();
        assert_eq!(nonce, 100);
        assert_eq!(tx.nonce, 100);
        assert!(!tx.signature.is_empty());
        assert_eq!(signer.current_nonce(), 101);
    }

    #[test]
    fn test_sign_cancel_order() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            200,
        )
        .unwrap();

        let expired_at = 1734200000000i64; // Transaction expiry
        let result = signer.sign_cancel_order(1, 999, expired_at);

        assert!(result.is_ok());
        let (tx, nonce) = result.unwrap();
        assert_eq!(nonce, 200);
        assert_eq!(tx.nonce, 200);
        assert_eq!(signer.current_nonce(), 201);
    }

    #[test]
    fn test_create_auth_token() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        )
        .unwrap();

        let deadline = 1734200000i64;
        let result = signer.create_auth_token(deadline);

        assert!(result.is_ok());
        let token = result.unwrap();
        assert!(token.starts_with(&format!("{}:878:2:", deadline)));
    }

    #[test]
    fn test_nonce_rollback_on_success() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        )
        .unwrap();

        let expired_at = 1734200000000i64;
        // Sign successfully
        let _ = signer.sign_create_order(1, 1, 100, 1000, false, 0, 0, false, 0, 0, expired_at);
        assert_eq!(signer.current_nonce(), 101);

        // Manual rollback
        signer.rollback_nonce();
        assert_eq!(signer.current_nonce(), 100);
    }

    #[test]
    fn test_real_schnorr_signing() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        )
        .unwrap();

        // Create a test message
        let message = [42u8; 40];

        // Sign the message
        let signature = signer.sign(&message).unwrap();

        // Verify signature length
        assert_eq!(signature.len(), 80);

        // Signature should not be all zeros (placeholder)
        assert_ne!(signature, [0u8; 80]);

        // Verify that different messages produce different signatures
        let message2 = [43u8; 40];
        let signature2 = signer.sign(&message2).unwrap();
        assert_ne!(signature, signature2);
    }

    #[test]
    fn test_signature_verification() {
        use goldilocks_crypto::ScalarField;

        // Generate a private key as ScalarField
        let private_key_scalar = ScalarField::new([1, 0, 0, 0, 0]);
        let private_key_bytes = private_key_scalar.to_bytes_le();

        // Create signer with this private key
        let private_key_hex = hex::encode(private_key_bytes);
        let signer = LighterSigner::new(
            &private_key_hex,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        )
        .unwrap();

        // Sign a message
        let message = [7u8; 40];
        let signature = signer.sign(&message).unwrap();

        // NOTE: The goldilocks-crypto verify_signature function currently expects
        // the private key as the third parameter (not the public key).
        // This is a quirk of the library implementation.
        // See: https://github.com/crates/goldilocks-crypto/blob/main/src/schnorr.rs#L818
        // In production, you would use the actual public key derived from the private key.
        let is_valid = LighterSigner::verify(&signature, &message, &private_key_bytes).unwrap();
        assert!(is_valid, "Signature should be valid");

        // Verify that wrong message fails
        let wrong_message = [8u8; 40];
        let is_valid_wrong = LighterSigner::verify(&signature, &wrong_message, &private_key_bytes)
            .unwrap_or(false);
        assert!(!is_valid_wrong, "Signature should be invalid for wrong message");
    }

    #[test]
    fn test_deterministic_signing_with_same_nonce() {
        let signer = LighterSigner::new(
            TEST_PRIVATE_KEY,
            LighterSigner::CHAIN_ID_TESTNET,
            2,
            878,
            100,
        )
        .unwrap();

        // Sign the same message twice with the same nonce (before incrementing)
        let message = [99u8; 40];
        let sig1 = signer.sign(&message).unwrap();
        let sig2 = signer.sign(&message).unwrap();

        // Should be identical since nonce is the same
        assert_eq!(sig1, sig2, "Signatures should be deterministic with same nonce");
    }
}
