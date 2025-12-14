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

//! Error types for the Lighter adapter.

use thiserror::Error;

/// Lighter adapter error types.
#[derive(Error, Debug)]
pub enum LighterError {
    /// Configuration error.
    #[error("Config error: {0}")]
    Config(String),

    /// HTTP/REST API error.
    #[error("HTTP error: {0}")]
    Http(String),

    /// WebSocket error.
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Signing/FFI error.
    #[error("Signing error: {0}")]
    Signing(String),

    /// Nonce management error.
    #[error("Nonce error: expected {expected}, got {actual}")]
    Nonce { expected: u64, actual: u64 },

    /// Order rejected by exchange.
    #[error("Order rejected: code={code}, message={message}")]
    OrderRejected { code: i32, message: String },

    /// Market not found.
    #[error("Market not found: {0}")]
    MarketNotFound(u32),

    /// Parse/deserialization error.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Authentication error.
    #[error("Auth error: {0}")]
    Auth(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Timeout error.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for LighterError {
    fn from(e: serde_json::Error) -> Self {
        Self::Parse(e.to_string())
    }
}

impl From<std::io::Error> for LighterError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(e.to_string())
    }
}
