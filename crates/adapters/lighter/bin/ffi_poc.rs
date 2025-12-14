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

//! Signing PoC - Proof of Concept for pure Rust signing.
//!
//! This binary tests the pure Rust signing implementation using
//! goldilocks-crypto and poseidon-hash crates.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin lighter-ffi-poc
//! ```

use nautilus_lighter::signing::LighterSigner;
use std::env;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Lighter Signing PoC (Pure Rust) ===\n");

    // Get configuration from environment or use defaults
    let private_key = env::var("LIGHTER_PRIVATE_KEY")
        .unwrap_or_else(|_| {
            println!("WARNING: Using dummy private key. Set LIGHTER_PRIVATE_KEY for real testing.");
            // 80 hex chars = 40 bytes private key
            "00000000000000000000000000000000000000000000000000000000000000000000000000000001".to_string()
        });
    let chain_id = env::var("LIGHTER_CHAIN_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(LighterSigner::CHAIN_ID_TESTNET);
    let api_key_index: u8 = env::var("LIGHTER_API_KEY_INDEX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let account_index: i64 = env::var("LIGHTER_ACCOUNT_INDEX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(878);
    let initial_nonce: u64 = env::var("LIGHTER_INITIAL_NONCE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    println!("Configuration:");
    println!("  Chain ID: {chain_id} (300=testnet, 304=mainnet)");
    println!("  API Key Index: {api_key_index}");
    println!("  Account Index: {account_index}");
    println!("  Initial Nonce: {initial_nonce}");
    println!();

    // Test 1: Create signer
    println!("Test 1: Creating LighterSigner...");
    match LighterSigner::new(
        &private_key,
        chain_id,
        api_key_index,
        account_index,
        initial_nonce,
    ) {
        Ok(signer) => {
            println!("  ✓ Signer created successfully");
            println!("  Current nonce: {}", signer.current_nonce());

            // Test 2: Create auth token
            println!("\nTest 2: Creating auth token...");
            let deadline = chrono::Utc::now().timestamp() + 3600; // 1 hour from now
            match signer.create_auth_token(deadline) {
                Ok(token) => {
                    println!("  ✓ Auth token created");
                    println!("  Token (truncated): {}...", &token[..token.len().min(50)]);
                }
                Err(e) => {
                    println!("  ✗ Auth token failed: {e}");
                }
            }

            // Test 3: Sign create order
            println!("\nTest 3: Signing create order...");
            let expired_at = chrono::Utc::now().timestamp_millis() + 60_000; // 60 seconds from now
            match signer.sign_create_order(
                1,                                    // market_index (ETH)
                12345,                                // client_order_index
                100_000_000,                          // base_amount (1.0 in 8 decimals)
                412739,                               // price (4127.39 with 2 decimals)
                false,                                // is_ask (buy)
                LighterSigner::ORDER_TYPE_LIMIT,      // order_type
                LighterSigner::TIF_GOOD_TILL_TIME,    // time_in_force
                false,                                // reduce_only
                0,                                    // trigger_price (0 for non-stop orders)
                0,                                    // order_expiry (0 for no expiry)
                expired_at,                           // expired_at (transaction expiry)
            ) {
                Ok((tx, nonce)) => {
                    println!("  ✓ Order signed successfully");
                    println!("  Nonce used: {nonce}");
                    println!("  Signature: {}...", &tx.signature[..32]);
                    println!("  Signed hash: {}", tx.signed_hash);
                }
                Err(e) => {
                    println!("  ✗ Signing failed: {e}");
                }
            }

            // Test 4: Sign cancel order
            println!("\nTest 4: Signing cancel order...");
            match signer.sign_cancel_order(1, 99999, expired_at) {
                Ok((tx, nonce)) => {
                    println!("  ✓ Cancel signed successfully");
                    println!("  Nonce used: {nonce}");
                    println!("  Signature: {}...", &tx.signature[..32]);
                }
                Err(e) => {
                    println!("  ✗ Cancel signing failed: {e}");
                }
            }

            // Test 5: Sign cancel all orders
            println!("\nTest 5: Signing cancel all orders...");
            let time_ms = chrono::Utc::now().timestamp_millis();
            match signer.sign_cancel_all_orders(0, time_ms, expired_at) {
                Ok((tx, nonce)) => {
                    println!("  ✓ Cancel all signed successfully");
                    println!("  Nonce used: {nonce}");
                    println!("  Signature: {}...", &tx.signature[..32]);
                }
                Err(e) => {
                    println!("  ✗ Cancel all signing failed: {e}");
                }
            }

            // Test 6: Nonce rollback
            println!("\nTest 6: Testing nonce rollback...");
            let before = signer.current_nonce();
            signer.rollback_nonce();
            let after = signer.current_nonce();
            println!("  Nonce before rollback: {before}");
            println!("  Nonce after rollback: {after}");
            if after == before - 1 {
                println!("  ✓ Rollback works correctly");
            } else {
                println!("  ✗ Rollback failed");
            }

            println!("\n=== Signing PoC Complete ===");
            println!("Final nonce: {}", signer.current_nonce());
        }
        Err(e) => {
            println!("  ✗ Signer creation failed: {e}");
            println!("\nPossible reasons:");
            println!("  1. Invalid private key format (must be 80 hex chars / 40 bytes)");
            println!("  2. Invalid hex encoding");
        }
    }
}
