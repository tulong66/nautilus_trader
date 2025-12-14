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

//! Data transfer objects for deserializing Lighter DEX HTTP API payloads.

use serde::{Deserialize, Serialize};
use ustr::Ustr;

/// Market status enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketStatus {
    /// Market is active and trading.
    Active,
    /// Market is inactive but not delisted.
    Inactive,
    /// Market has been delisted.
    Delisted,
}

/// Trade side enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeSide {
    /// Buy side trade.
    Buy,
    /// Sell side trade.
    Sell,
}

/// Order type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    /// Limit order.
    Limit,
    /// Market order.
    Market,
}

/// Order status enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    /// Newly created order.
    New,
    /// Order is partially filled.
    #[serde(rename = "partially_filled")]
    PartiallyFilled,
    /// Order is completely filled.
    Filled,
    /// Order has been canceled.
    Canceled,
    /// Order was rejected by the exchange.
    Rejected,
    /// Order has expired.
    Expired,
}

/// Time in force enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeInForce {
    /// Good till time.
    #[serde(rename = "good_till_time")]
    GoodTillTime,
    /// Immediate or cancel.
    #[serde(rename = "immediate_or_cancel")]
    ImmediateOrCancel,
    /// Fill or kill.
    #[serde(rename = "fill_or_kill")]
    FillOrKill,
    /// Post only.
    #[serde(rename = "post_only")]
    PostOnly,
}

/// Market information from the Lighter DEX.
///
/// # References
/// - Lighter API documentation: <https://docs.lighter.xyz>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    /// Market index identifier.
    pub market_index: u16,
    /// Market symbol (e.g., "BTC/USDC").
    pub symbol: Ustr,
    /// Base asset symbol.
    pub base_asset: Ustr,
    /// Quote asset symbol.
    pub quote_asset: Ustr,
    /// Minimum price increment (tick size).
    pub tick_size: String,
    /// Minimum quantity increment (step size).
    pub step_size: String,
    /// Minimum order size.
    pub min_order_size: String,
    /// Maximum order size.
    pub max_order_size: String,
    /// Current market status.
    pub status: MarketStatus,
}

/// Single level in the orderbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderbookLevel {
    /// Price level.
    pub price: String,
    /// Quantity at this price level.
    pub quantity: String,
}

/// Orderbook snapshot response.
///
/// # References
/// - Lighter API documentation: <https://docs.lighter.xyz>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderbookResponse {
    /// Market index for this orderbook.
    pub market_index: u16,
    /// Timestamp of the snapshot (milliseconds).
    pub timestamp: i64,
    /// Bid levels (buy orders).
    pub bids: Vec<OrderbookLevel>,
    /// Ask levels (sell orders).
    pub asks: Vec<OrderbookLevel>,
}

/// Trade execution response.
///
/// # References
/// - Lighter API documentation: <https://docs.lighter.xyz>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeResponse {
    /// Market index for this trade.
    pub market_index: u16,
    /// Unique trade identifier.
    pub trade_id: i64,
    /// Execution price.
    pub price: String,
    /// Executed quantity.
    pub quantity: String,
    /// Side of the trade (buy/sell).
    pub side: TradeSide,
    /// Timestamp of the trade (milliseconds).
    pub timestamp: i64,
}

/// Market ticker information.
///
/// # References
/// - Lighter API documentation: <https://docs.lighter.xyz>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickerResponse {
    /// Market index for this ticker.
    pub market_index: u16,
    /// Last traded price.
    pub last_price: String,
    /// Best bid price.
    pub best_bid: String,
    /// Best ask price.
    pub best_ask: String,
    /// 24-hour trading volume.
    pub volume_24h: String,
    /// 24-hour high price.
    pub high_24h: String,
    /// 24-hour low price.
    pub low_24h: String,
    /// 24-hour price change.
    pub price_change_24h: String,
    /// Timestamp of the ticker (milliseconds).
    pub timestamp: i64,
}

/// Asset balance information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    /// Asset symbol.
    pub asset: Ustr,
    /// Free (available) balance.
    pub free: String,
    /// Locked (reserved) balance.
    pub locked: String,
}

/// Position information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Market index for this position.
    pub market_index: u16,
    /// Position size (positive for long, negative for short).
    pub size: String,
    /// Entry price of the position.
    pub entry_price: String,
    /// Unrealized profit and loss.
    pub unrealized_pnl: String,
    /// Margin allocated to this position.
    pub margin: String,
}

/// Account information response.
///
/// # References
/// - Lighter API documentation: <https://docs.lighter.xyz>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    /// Account index identifier.
    pub account_index: i64,
    /// Asset balances.
    pub balances: Vec<Balance>,
    /// Open positions.
    pub positions: Vec<Position>,
}

/// Order information response.
///
/// # References
/// - Lighter API documentation: <https://docs.lighter.xyz>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    /// Order index identifier.
    pub order_index: i64,
    /// Client-provided order identifier.
    pub client_order_index: i64,
    /// Market index for this order.
    pub market_index: u16,
    /// Order side (buy/sell).
    pub side: TradeSide,
    /// Order type (limit/market).
    pub order_type: OrderType,
    /// Order price.
    pub price: String,
    /// Order quantity.
    pub quantity: String,
    /// Filled quantity.
    pub filled_quantity: String,
    /// Order status.
    pub status: OrderStatus,
    /// Time in force policy.
    pub time_in_force: TimeInForce,
    /// Order creation timestamp (milliseconds).
    pub created_at: i64,
    /// Last update timestamp (milliseconds).
    pub updated_at: i64,
}

/// Generic Lighter API response wrapper.
///
/// The Lighter API typically wraps responses with a success flag and optional error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LighterResponse<T> {
    /// Indicates if the request was successful.
    pub success: bool,
    /// Error message if success is false.
    pub error: Option<String>,
    /// Response data payload.
    pub data: Option<T>,
}

/// Generic list wrapper for collections.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LighterList<T> {
    /// Collection of items.
    pub items: Vec<T>,
}

/// Convenience type alias for market list response.
pub type MarketsResponse = LighterResponse<LighterList<Market>>;

/// Convenience type alias for trades list response.
pub type TradesResponse = LighterResponse<LighterList<TradeResponse>>;

/// Convenience type alias for orders list response.
pub type OrdersResponse = LighterResponse<LighterList<OrderResponse>>;

// ================================================================================================
// Transaction Request Types
// ================================================================================================

/// Create order transaction request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderRequest {
    /// Market index for the order.
    pub market_index: u16,
    /// Client-provided order index.
    pub client_order_index: i64,
    /// Base amount in raw integer format.
    pub base_amount: i64,
    /// Price in raw integer format.
    pub price: u32,
    /// True for sell (ask), false for buy (bid).
    pub is_ask: bool,
    /// Order type (0=limit, 1=market).
    pub order_type: u8,
    /// Time in force (0=GoodTillTime, 1=IOC, 2=FOK, 3=PostOnly).
    pub time_in_force: u8,
    /// Whether this is a reduce-only order.
    pub reduce_only: bool,
    /// Transaction nonce.
    pub nonce: u64,
    /// Hex-encoded signature.
    pub signature: String,
}

/// Cancel order transaction request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelOrderRequest {
    /// Market index for the order.
    pub market_index: u16,
    /// Order index to cancel (venue order ID).
    pub order_index: i64,
    /// Transaction nonce.
    pub nonce: u64,
    /// Hex-encoded signature.
    pub signature: String,
}

/// Cancel all orders transaction request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllOrdersRequest {
    /// Time in force filter (0 for all).
    pub time_in_force: u8,
    /// Current timestamp in milliseconds.
    pub time: i64,
    /// Transaction nonce.
    pub nonce: u64,
    /// Hex-encoded signature.
    pub signature: String,
}

/// Transaction response from send_tx endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    /// Transaction ID if successful.
    pub tx_id: Option<String>,
    /// Order index if order was created.
    pub order_index: Option<i64>,
}

/// Convenience type alias for transaction response.
pub type TxResponse = LighterResponse<TransactionResponse>;

/// Nonce response from next_nonce endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NonceResponse {
    /// Next nonce value.
    pub nonce: u64,
}

/// Convenience type alias for nonce response.
pub type NextNonceResponse = LighterResponse<NonceResponse>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_market_status() {
        let json = r#""active""#;
        let status: MarketStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, MarketStatus::Active);

        let json = r#""inactive""#;
        let status: MarketStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, MarketStatus::Inactive);

        let json = r#""delisted""#;
        let status: MarketStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, MarketStatus::Delisted);
    }

    #[test]
    fn test_deserialize_trade_side() {
        let json = r#""buy""#;
        let side: TradeSide = serde_json::from_str(json).unwrap();
        assert_eq!(side, TradeSide::Buy);

        let json = r#""sell""#;
        let side: TradeSide = serde_json::from_str(json).unwrap();
        assert_eq!(side, TradeSide::Sell);
    }

    #[test]
    fn test_deserialize_order_status() {
        let json = r#""new""#;
        let status: OrderStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, OrderStatus::New);

        let json = r#""partially_filled""#;
        let status: OrderStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, OrderStatus::PartiallyFilled);

        let json = r#""filled""#;
        let status: OrderStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, OrderStatus::Filled);
    }

    #[test]
    fn test_deserialize_market() {
        let json = r#"{
            "marketIndex": 1,
            "symbol": "BTC/USDC",
            "baseAsset": "BTC",
            "quoteAsset": "USDC",
            "tickSize": "0.01",
            "stepSize": "0.001",
            "minOrderSize": "0.001",
            "maxOrderSize": "100",
            "status": "active"
        }"#;

        let market: Market = serde_json::from_str(json).unwrap();
        assert_eq!(market.market_index, 1);
        assert_eq!(market.symbol.as_str(), "BTC/USDC");
        assert_eq!(market.base_asset.as_str(), "BTC");
        assert_eq!(market.quote_asset.as_str(), "USDC");
        assert_eq!(market.tick_size, "0.01");
        assert_eq!(market.status, MarketStatus::Active);
    }

    #[test]
    fn test_deserialize_orderbook_response() {
        let json = r#"{
            "marketIndex": 1,
            "timestamp": 1703001600000,
            "bids": [
                {"price": "43000.00", "quantity": "1.5"},
                {"price": "42999.00", "quantity": "2.0"}
            ],
            "asks": [
                {"price": "43001.00", "quantity": "1.2"},
                {"price": "43002.00", "quantity": "0.8"}
            ]
        }"#;

        let orderbook: OrderbookResponse = serde_json::from_str(json).unwrap();
        assert_eq!(orderbook.market_index, 1);
        assert_eq!(orderbook.timestamp, 1703001600000);
        assert_eq!(orderbook.bids.len(), 2);
        assert_eq!(orderbook.asks.len(), 2);
        assert_eq!(orderbook.bids[0].price, "43000.00");
        assert_eq!(orderbook.bids[0].quantity, "1.5");
    }

    #[test]
    fn test_deserialize_trade_response() {
        let json = r#"{
            "marketIndex": 1,
            "tradeId": 12345,
            "price": "43000.00",
            "quantity": "0.5",
            "side": "buy",
            "timestamp": 1703001600000
        }"#;

        let trade: TradeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(trade.market_index, 1);
        assert_eq!(trade.trade_id, 12345);
        assert_eq!(trade.price, "43000.00");
        assert_eq!(trade.quantity, "0.5");
        assert_eq!(trade.side, TradeSide::Buy);
        assert_eq!(trade.timestamp, 1703001600000);
    }

    #[test]
    fn test_deserialize_ticker_response() {
        let json = r#"{
            "marketIndex": 1,
            "lastPrice": "43000.00",
            "bestBid": "42999.00",
            "bestAsk": "43001.00",
            "volume24h": "1234.56",
            "high24h": "44000.00",
            "low24h": "42000.00",
            "priceChange24h": "1000.00",
            "timestamp": 1703001600000
        }"#;

        let ticker: TickerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.market_index, 1);
        assert_eq!(ticker.last_price, "43000.00");
        assert_eq!(ticker.best_bid, "42999.00");
        assert_eq!(ticker.best_ask, "43001.00");
        assert_eq!(ticker.volume_24h, "1234.56");
    }

    #[test]
    fn test_deserialize_order_response() {
        let json = r#"{
            "orderIndex": 98765,
            "clientOrderIndex": 12345,
            "marketIndex": 1,
            "side": "buy",
            "orderType": "limit",
            "price": "43000.00",
            "quantity": "1.0",
            "filledQuantity": "0.5",
            "status": "partially_filled",
            "timeInForce": "good_till_time",
            "createdAt": 1703001600000,
            "updatedAt": 1703001700000
        }"#;

        let order: OrderResponse = serde_json::from_str(json).unwrap();
        assert_eq!(order.order_index, 98765);
        assert_eq!(order.client_order_index, 12345);
        assert_eq!(order.market_index, 1);
        assert_eq!(order.side, TradeSide::Buy);
        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.time_in_force, TimeInForce::GoodTillTime);
    }

    #[test]
    fn test_deserialize_lighter_response() {
        let json = r#"{
            "success": true,
            "data": {
                "items": [
                    {
                        "marketIndex": 1,
                        "symbol": "BTC/USDC",
                        "baseAsset": "BTC",
                        "quoteAsset": "USDC",
                        "tickSize": "0.01",
                        "stepSize": "0.001",
                        "minOrderSize": "0.001",
                        "maxOrderSize": "100",
                        "status": "active"
                    }
                ]
            }
        }"#;

        let response: MarketsResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        assert!(response.error.is_none());
        assert!(response.data.is_some());
        let data = response.data.unwrap();
        assert_eq!(data.items.len(), 1);
        assert_eq!(data.items[0].market_index, 1);
    }

    #[test]
    fn test_deserialize_lighter_response_error() {
        let json = r#"{
            "success": false,
            "error": "Invalid market index"
        }"#;

        let response: LighterResponse<()> = serde_json::from_str(json).unwrap();
        assert!(!response.success);
        assert_eq!(response.error.as_deref(), Some("Invalid market index"));
        assert!(response.data.is_none());
    }
}
