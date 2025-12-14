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

//! API endpoint constants for the Lighter DEX REST API.

// ================================================================================================
// Public (Unauthenticated) Endpoints
// ================================================================================================

/// Get all available markets/order books.
///
/// **HTTP Method**: GET
///
/// **Response**: List of order book information including market ID, symbols, etc.
pub const ORDER_BOOKS: &str = "/api/v1/order_books";

/// Get detailed information for a specific order book.
///
/// **HTTP Method**: GET
///
/// **Query Parameters**: `order_book_id`
///
/// **Response**: Detailed order book configuration and status.
pub const ORDER_BOOK_DETAILS: &str = "/api/v1/order_book_details";

/// Get order book depth (bids and asks).
///
/// **HTTP Method**: GET
///
/// **Query Parameters**: `order_book_id`, optional `depth`
///
/// **Response**: Current order book state with price levels.
pub const ORDER_BOOK_ORDERS: &str = "/api/v1/order_book_orders";

/// Get recent trades for a market.
///
/// **HTTP Method**: GET
///
/// **Query Parameters**: `order_book_id`, optional `limit`
///
/// **Response**: List of recent trades with price, size, timestamp.
pub const RECENT_TRADES: &str = "/api/v1/recent_trades";

/// Get candlestick/OHLCV data.
///
/// **HTTP Method**: GET
///
/// **Query Parameters**: `order_book_id`, `interval`, `start_time`, optional `end_time`
///
/// **Response**: OHLCV candlestick bars.
pub const CANDLESTICKS: &str = "/api/v1/candlesticks";

/// Get 24-hour ticker data.
///
/// **HTTP Method**: GET
///
/// **Query Parameters**: optional `order_book_id` (if omitted, returns all tickers)
///
/// **Response**: 24h price statistics including high, low, volume, price change.
pub const TICKER: &str = "/api/v1/ticker";

// ================================================================================================
// Private (Authenticated) Endpoints
// ================================================================================================

/// Get account information including balances and positions.
///
/// **HTTP Method**: GET
///
/// **Headers**: Requires `X-Lighter-Auth`
///
/// **Query Parameters**: `blockchain_id` (wallet address)
///
/// **Response**: Account balances, open positions, margin info.
pub const ACCOUNT: &str = "/api/v1/account";

/// Get active orders for an account.
///
/// **HTTP Method**: GET
///
/// **Headers**: Requires `X-Lighter-Auth`
///
/// **Query Parameters**: `blockchain_id`, optional `order_book_id`
///
/// **Response**: List of open orders.
pub const ACCOUNT_ACTIVE_ORDERS: &str = "/api/v1/account_active_orders";

/// Get order history for an account.
///
/// **HTTP Method**: GET
///
/// **Headers**: Requires `X-Lighter-Auth`
///
/// **Query Parameters**: `blockchain_id`, optional filters
///
/// **Response**: Historical order data.
pub const ORDER_HISTORY: &str = "/api/v1/order_history";

/// Get trade history for an account.
///
/// **HTTP Method**: GET
///
/// **Headers**: Requires `X-Lighter-Auth`
///
/// **Query Parameters**: `blockchain_id`, optional filters
///
/// **Response**: Historical trade/fill data.
pub const TRADE_HISTORY: &str = "/api/v1/trade_history";

// ================================================================================================
// Transaction Endpoints
// ================================================================================================

/// Get the next nonce for transaction signing.
///
/// **HTTP Method**: GET
///
/// **Query Parameters**: `blockchain_id` (wallet address)
///
/// **Response**: Next nonce value to use for signing.
pub const NEXT_NONCE: &str = "/api/v1/next_nonce";

/// Submit a signed transaction.
///
/// **HTTP Method**: POST
///
/// **Headers**: `Content-Type: application/json`
///
/// **Body**: Signed transaction payload including signature.
///
/// **Response**: Transaction ID or error.
pub const SEND_TX: &str = "/api/v1/send_tx";

/// Submit multiple signed transactions in a batch.
///
/// **HTTP Method**: POST
///
/// **Headers**: `Content-Type: application/json`
///
/// **Body**: Array of signed transaction payloads.
///
/// **Response**: Array of transaction IDs or errors.
pub const SEND_TX_BATCH: &str = "/api/v1/send_tx_batch";

// ================================================================================================
// Order Management Endpoints
// ================================================================================================

/// Create a new order (via signed transaction).
///
/// This is typically done via `SEND_TX` endpoint with an order creation transaction.
pub const CREATE_ORDER: &str = SEND_TX;

/// Cancel an order (via signed transaction).
///
/// This is typically done via `SEND_TX` endpoint with an order cancellation transaction.
pub const CANCEL_ORDER: &str = SEND_TX;

/// Cancel all orders (via signed transaction).
///
/// This is typically done via `SEND_TX` endpoint with a cancel-all transaction.
pub const CANCEL_ALL_ORDERS: &str = SEND_TX;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_paths() {
        // Public endpoints
        assert_eq!(ORDER_BOOKS, "/api/v1/order_books");
        assert_eq!(ORDER_BOOK_DETAILS, "/api/v1/order_book_details");
        assert_eq!(RECENT_TRADES, "/api/v1/recent_trades");
        assert_eq!(CANDLESTICKS, "/api/v1/candlesticks");

        // Private endpoints
        assert_eq!(ACCOUNT, "/api/v1/account");
        assert_eq!(ACCOUNT_ACTIVE_ORDERS, "/api/v1/account_active_orders");

        // Transaction endpoints
        assert_eq!(NEXT_NONCE, "/api/v1/next_nonce");
        assert_eq!(SEND_TX, "/api/v1/send_tx");
        assert_eq!(SEND_TX_BATCH, "/api/v1/send_tx_batch");
    }

    #[test]
    fn test_order_endpoints_use_send_tx() {
        // Verify that order management uses the send_tx endpoint
        assert_eq!(CREATE_ORDER, SEND_TX);
        assert_eq!(CANCEL_ORDER, SEND_TX);
        assert_eq!(CANCEL_ALL_ORDERS, SEND_TX);
    }
}
