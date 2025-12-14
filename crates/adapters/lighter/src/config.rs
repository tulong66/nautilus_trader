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

//! Configuration types for the Lighter adapter.
//!
//! Uses Public/Private config separation pattern (from PM-B):
//! - `LighterPublicConfig`: For data-only access (no credentials)
//! - `LighterPrivateConfig`: For trading access (includes credentials)
//! - `LighterDataClientConfig`: NautilusTrader DataClient configuration
//! - `LighterExecClientConfig`: NautilusTrader ExecutionClient configuration

use crate::common::enums::LighterEnvironment;

/// Chain ID constants.
pub mod chain_id {
    /// Lighter testnet chain ID.
    pub const TESTNET: u32 = 300;
    /// Lighter mainnet chain ID.
    pub const MAINNET: u32 = 304;
}

/// Public API configuration (no credentials required).
///
/// Use this for data-only access to Lighter markets.
#[derive(Clone, Debug)]
pub struct LighterPublicConfig {
    /// Environment (Mainnet/Testnet).
    pub environment: LighterEnvironment,
    /// Custom HTTP base URL (optional, uses default if None).
    pub http_base_url: Option<String>,
    /// Custom WebSocket base URL (optional, uses default if None).
    pub ws_base_url: Option<String>,
    /// HTTP request timeout in seconds (default: 30).
    pub http_timeout_secs: u64,
    /// WebSocket ping interval in seconds (default: 60, Lighter timeout is 120s).
    pub ws_ping_interval_secs: u64,
}

impl Default for LighterPublicConfig {
    fn default() -> Self {
        Self {
            environment: LighterEnvironment::default(),
            http_base_url: None,
            ws_base_url: None,
            http_timeout_secs: 30,
            ws_ping_interval_secs: 60,
        }
    }
}

impl LighterPublicConfig {
    /// Create a new public config for the specified environment.
    #[must_use]
    pub fn new(environment: LighterEnvironment) -> Self {
        Self {
            environment,
            ..Default::default()
        }
    }

    /// Get the HTTP base URL for the configured environment.
    #[must_use]
    pub fn http_url(&self) -> &str {
        self.http_base_url
            .as_deref()
            .unwrap_or_else(|| self.environment.http_url())
    }

    /// Get the WebSocket base URL for the configured environment.
    #[must_use]
    pub fn ws_url(&self) -> &str {
        self.ws_base_url
            .as_deref()
            .unwrap_or_else(|| self.environment.ws_url())
    }

    /// Get the chain ID for the configured environment.
    #[must_use]
    pub fn chain_id(&self) -> u32 {
        self.environment.chain_id()
    }
}

/// Private API configuration (credentials required).
///
/// Extends `LighterPublicConfig` with authentication credentials.
#[derive(Clone, Debug)]
pub struct LighterPrivateConfig {
    /// Public configuration (inherited).
    pub public: LighterPublicConfig,
    /// API private key (40 bytes / 80 hex chars).
    pub private_key: String,
    /// API key index (2-254 for programmatic trading).
    pub api_key_index: u8,
    /// Account index on Lighter.
    pub account_index: u32,
}

impl LighterPrivateConfig {
    /// Create a new private config.
    ///
    /// # Arguments
    /// * `environment` - Mainnet or Testnet
    /// * `private_key` - 40-byte API key (80 hex chars, with or without 0x prefix)
    /// * `api_key_index` - API key index (2-254)
    /// * `account_index` - Account index on Lighter
    #[must_use]
    pub fn new(
        environment: LighterEnvironment,
        private_key: String,
        api_key_index: u8,
        account_index: u32,
    ) -> Self {
        Self {
            public: LighterPublicConfig::new(environment),
            private_key,
            api_key_index,
            account_index,
        }
    }

    /// Get the chain ID for the configured environment.
    #[must_use]
    pub fn chain_id(&self) -> u32 {
        self.public.chain_id()
    }
}

/// NautilusTrader Data Client configuration.
#[derive(Clone, Debug)]
pub struct LighterDataClientConfig {
    /// Environment (Mainnet/Testnet).
    pub environment: LighterEnvironment,
    /// Custom HTTP base URL (optional).
    pub base_url_http: Option<String>,
    /// Custom WebSocket base URL (optional).
    pub base_url_ws: Option<String>,
    /// Heartbeat interval in seconds (default: 60).
    pub heartbeat_interval_secs: Option<u64>,
    /// HTTP timeout in seconds (default: 30).
    pub http_timeout_secs: Option<u64>,
    /// Maximum retry attempts (default: 3).
    pub max_retries: Option<u32>,
}

impl Default for LighterDataClientConfig {
    fn default() -> Self {
        Self {
            environment: LighterEnvironment::default(),
            base_url_http: None,
            base_url_ws: None,
            heartbeat_interval_secs: Some(60),
            http_timeout_secs: Some(30),
            max_retries: Some(3),
        }
    }
}

impl LighterDataClientConfig {
    /// Convert to public config.
    #[must_use]
    pub fn to_public_config(&self) -> LighterPublicConfig {
        LighterPublicConfig {
            environment: self.environment,
            http_base_url: self.base_url_http.clone(),
            ws_base_url: self.base_url_ws.clone(),
            http_timeout_secs: self.http_timeout_secs.unwrap_or(30),
            ws_ping_interval_secs: self.heartbeat_interval_secs.unwrap_or(60),
        }
    }
}

/// NautilusTrader Execution Client configuration.
#[derive(Clone, Debug)]
pub struct LighterExecClientConfig {
    /// API private key (40 bytes / 80 hex chars).
    pub private_key: String,
    /// Account index on Lighter.
    pub account_index: u32,
    /// API key index (2-254 for programmatic trading).
    pub api_key_index: u8,
    /// Environment (Mainnet/Testnet).
    pub environment: LighterEnvironment,
    /// Order ID prefix for filtering (optional).
    pub order_prefix: Option<String>,
    /// GC interval for stale orders in seconds (default: 300).
    pub gc_interval_secs: Option<u64>,
    /// Custom HTTP base URL (optional).
    pub base_url_http: Option<String>,
    /// Custom WebSocket base URL (optional).
    pub base_url_ws: Option<String>,
    /// Heartbeat interval in seconds (default: 60).
    pub heartbeat_interval_secs: Option<u64>,
    /// HTTP timeout in seconds (default: 30).
    pub http_timeout_secs: Option<u64>,
    /// Maximum retry attempts (default: 3).
    pub max_retries: Option<u32>,
}

impl LighterExecClientConfig {
    /// Create a new execution client config.
    #[must_use]
    pub fn new(
        private_key: String,
        account_index: u32,
        api_key_index: u8,
        environment: LighterEnvironment,
    ) -> Self {
        Self {
            private_key,
            account_index,
            api_key_index,
            environment,
            order_prefix: None,
            gc_interval_secs: Some(300),
            base_url_http: None,
            base_url_ws: None,
            heartbeat_interval_secs: Some(60),
            http_timeout_secs: Some(30),
            max_retries: Some(3),
        }
    }

    /// Get the chain ID for the configured environment.
    #[must_use]
    pub fn chain_id(&self) -> u32 {
        self.environment.chain_id()
    }

    /// Convert to private config.
    #[must_use]
    pub fn to_private_config(&self) -> LighterPrivateConfig {
        LighterPrivateConfig {
            public: LighterPublicConfig {
                environment: self.environment,
                http_base_url: self.base_url_http.clone(),
                ws_base_url: self.base_url_ws.clone(),
                http_timeout_secs: self.http_timeout_secs.unwrap_or(30),
                ws_ping_interval_secs: self.heartbeat_interval_secs.unwrap_or(60),
            },
            private_key: self.private_key.clone(),
            api_key_index: self.api_key_index,
            account_index: self.account_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_config_defaults() {
        let config = LighterPublicConfig::default();
        assert_eq!(config.environment, LighterEnvironment::Mainnet);
        assert_eq!(config.http_timeout_secs, 30);
        assert_eq!(config.ws_ping_interval_secs, 60);
    }

    #[test]
    fn test_public_config_testnet() {
        let config = LighterPublicConfig::new(LighterEnvironment::Testnet);
        assert_eq!(config.chain_id(), chain_id::TESTNET);
    }

    #[test]
    fn test_private_config() {
        let config = LighterPrivateConfig::new(
            LighterEnvironment::Testnet,
            "0x1234567890abcdef".to_string(),
            2,
            878,
        );
        assert_eq!(config.chain_id(), chain_id::TESTNET);
        assert_eq!(config.api_key_index, 2);
        assert_eq!(config.account_index, 878);
    }
}
