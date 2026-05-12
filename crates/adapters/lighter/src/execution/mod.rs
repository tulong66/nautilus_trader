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

//! Execution client implementation for the Lighter DEX adapter.
//!
//! This module provides order execution functionality for Lighter DEX:
//! - Order submission with L2 cryptographic signing
//! - Order cancellation
//! - Order modification
//! - Account state management
//! - Position tracking
//!
//! # Architecture
//!
//! The execution client follows NautilusTrader's standard pattern:
//! 1. **Synchronous validation** - Immediate checks and event generation
//! 2. **Async submission** - Non-blocking HTTP calls with signed transactions
//! 3. **WebSocket updates** - Real-time order and position updates
//!
//! # Signing
//!
//! All orders must be signed using Lighter's cryptographic stack:
//! - Elliptic Curve: ECgFp5 (degree-5 extension over Goldilocks)
//! - Hash Function: Poseidon2 (ZK-friendly)
//! - Signature Scheme: Schnorr signatures
//!
//! See [`crate::signing`] for implementation details.

pub mod client;
pub mod fixtures;

pub use client::LighterExecutionClient;
