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

//! Lighter DEX integration adapter for NautilusTrader.
//!
//! This module provides connectivity to the Lighter DEX:
//! - Market data streaming via WebSocket
//! - Order execution via REST API with L2 signing
//! - Account information and position tracking
//!
//! # Architecture
//!
//! The adapter follows NautilusTrader's standard adapter pattern:
//! - `config`: Configuration for data and execution clients
//! - `common`: Shared types, enums, and utilities
//! - `http`: REST API client implementation
//! - `websocket`: WebSocket client for real-time data
//! - `signing`: Pure Rust transaction signing
//! - `data`: DataClient implementation
//! - `execution`: ExecutionClient implementation
//!
//! # Signing
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
//! ## Key Sizes
//!
//! - Private Key: 40 bytes (5 Goldilocks field elements)
//! - Public Key: 40 bytes (ECgFp5 point encoded as Fp5Element)
//! - Signature: 80 bytes (r: 40 bytes, s: 40 bytes)
//!
//! The signing module uses pure Rust implementation via `goldilocks-crypto` and
//! `poseidon-hash` crates, avoiding the need for FFI bindings.

pub mod common;
pub mod config;
pub mod data;
pub mod error;
pub mod execution;
pub mod http;
pub mod signing;
pub mod websocket;

#[cfg(feature = "python")]
pub mod python;

pub use config::{LighterDataClientConfig, LighterExecClientConfig};
pub use error::LighterError;
pub use execution::LighterExecutionClient;
