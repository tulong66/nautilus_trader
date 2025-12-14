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

//! URL builders for the Lighter adapter.

use super::enums::LighterEnvironment;

/// HTTP API endpoints.
pub mod http {
    /// Get all order books (markets).
    pub const ORDER_BOOKS: &str = "/api/v1/order_books";
    /// Get order book details by ID.
    pub const ORDER_BOOK_DETAILS: &str = "/api/v1/order_book_details";
    /// Get order book orders (depth).
    pub const ORDER_BOOK_ORDERS: &str = "/api/v1/order_book_orders";
    /// Get recent trades.
    pub const RECENT_TRADES: &str = "/api/v1/recent_trades";
    /// Get candlesticks.
    pub const CANDLESTICKS: &str = "/api/v1/candlesticks";
    /// Get account info.
    pub const ACCOUNT: &str = "/api/v1/account";
    /// Get account active orders.
    pub const ACCOUNT_ACTIVE_ORDERS: &str = "/api/v1/account_active_orders";
    /// Get next nonce.
    pub const NEXT_NONCE: &str = "/api/v1/next_nonce";
    /// Send transaction.
    pub const SEND_TX: &str = "/api/v1/send_tx";
    /// Send batch transactions.
    pub const SEND_TX_BATCH: &str = "/api/v1/send_tx_batch";
}

/// WebSocket channels.
pub mod ws {
    /// WebSocket stream path (Lighter uses single endpoint for all channels).
    pub const STREAM_PATH: &str = "/stream";
    /// Order book channel prefix.
    pub const ORDER_BOOK: &str = "order_book";
    /// Trade channel prefix.
    pub const TRADE: &str = "trade";
    /// Market stats channel prefix.
    pub const MARKET_STATS: &str = "market_stats";
    /// Account all channel prefix.
    pub const ACCOUNT_ALL: &str = "account_all";
    /// Account all orders channel prefix.
    pub const ACCOUNT_ALL_ORDERS: &str = "account_all_orders";
    /// Account all trades channel prefix.
    pub const ACCOUNT_ALL_TRADES: &str = "account_all_trades";
    /// Account all positions channel prefix.
    pub const ACCOUNT_ALL_POSITIONS: &str = "account_all_positions";
    /// User stats channel prefix.
    pub const USER_STATS: &str = "user_stats";
    /// Height channel.
    pub const HEIGHT: &str = "height";
}

/// Build a full HTTP URL for the given environment and path.
#[must_use]
pub fn build_http_url(env: LighterEnvironment, path: &str) -> String {
    format!("{}{}", env.http_url(), path)
}

/// Build a full WebSocket URL for the given environment and path.
#[must_use]
pub fn build_ws_url(env: LighterEnvironment, path: &str) -> String {
    // WebSocket base URL already includes /stream
    let base = env.ws_url();
    if path.is_empty() || path == ws::STREAM_PATH {
        base.to_string()
    } else if path.starts_with('/') {
        // Replace /stream with the given path
        base.replace("/stream", path)
    } else {
        format!("{}/{}", base, path)
    }
}

/// Build the WebSocket stream URL (Lighter uses single endpoint).
#[must_use]
pub fn build_stream_ws_url(env: LighterEnvironment) -> String {
    env.ws_url().to_string()
}

/// Build the public WebSocket URL (alias for stream URL).
#[must_use]
pub fn build_public_ws_url(env: LighterEnvironment) -> String {
    build_stream_ws_url(env)
}

/// Build the private WebSocket URL (alias for stream URL, auth via subscription).
#[must_use]
pub fn build_private_ws_url(env: LighterEnvironment) -> String {
    build_stream_ws_url(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_http_url() {
        let url = build_http_url(LighterEnvironment::Testnet, http::ORDER_BOOKS);
        assert_eq!(url, "https://testnet.zklighter.elliot.ai/api/v1/order_books");
    }

    #[test]
    fn test_build_public_ws_url() {
        let url = build_public_ws_url(LighterEnvironment::Testnet);
        assert!(url.contains("testnet"));
        assert!(url.contains("/stream"));
    }

    #[test]
    fn test_build_private_ws_url() {
        let url = build_private_ws_url(LighterEnvironment::Mainnet);
        assert!(url.contains("mainnet"));
        assert!(url.contains("/stream"));
    }

    #[test]
    fn test_build_stream_ws_url() {
        let url = build_stream_ws_url(LighterEnvironment::Testnet);
        assert_eq!(url, "wss://testnet.zklighter.elliot.ai/stream");
    }
}
