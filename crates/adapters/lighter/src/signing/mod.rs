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

//! Lighter DEX signing module.
//!
//! This module provides cryptographic signing functionality for the Lighter DEX.
//!
//! # Cryptographic Stack
//!
//! Lighter uses a **non-EVM** cryptographic stack optimized for ZK proofs:
//!
//! | Component | Implementation |
//! |-----------|---------------|
//! | Elliptic Curve | ECgFp5 (degree-5 extension over Goldilocks) |
//! | Hash Function | Poseidon2 (ZK-friendly) |
//! | Signature Scheme | Schnorr |
//! | Field | Goldilocks (p = 2^64 - 2^32 + 1) |
//!
//! # Key Sizes
//!
//! - Private Key: 40 bytes (5 Goldilocks field elements)
//! - Public Key: 40 bytes (ECgFp5 point encoded as Fp5Element)
//! - Signature: 80 bytes (r: 40 bytes, s: 40 bytes)
//!
//! # Implementation
//!
//! This module uses pure Rust implementation via `goldilocks-crypto` and `poseidon-hash`
//! crates, avoiding the need for FFI bindings to the lighter-go library.
//!
//! # Example
//!
//! ```ignore
//! use nautilus_lighter::signing::{LighterSigner, NonceManager};
//!
//! // Create a signer from private key
//! let signer = LighterSigner::new(&private_key_bytes)?;
//!
//! // Create a nonce manager
//! let nonce_manager = NonceManager::new(initial_nonce);
//!
//! // Sign a message
//! let signature = signer.sign(&message)?;
//!
//! // Create auth token
//! let token = signer.create_auth_token(deadline, account_index, api_key_index)?;
//! ```

pub mod nonce;
pub mod signer;

pub use nonce::NonceManager;
pub use signer::{LighterSigner, SignedTransaction, SigningError};
