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

//! WebSocket message types for the Lighter DEX adapter.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::LighterError;

/// Subscription types supported by the Lighter WebSocket API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionType {
    /// Subscribe to orderbook updates for a specific market.
    Orderbook { market_index: u16 },
    /// Subscribe to trade updates for a specific market.
    Trades { market_index: u16 },
    /// Subscribe to ticker updates for a specific market.
    Ticker { market_index: u16 },
    /// Subscribe to account updates (requires authentication).
    Account,
    /// Subscribe to order updates (requires authentication).
    Orders,
}

impl SubscriptionType {
    /// Convert subscription type to channel name for Lighter API.
    ///
    /// Channel format follows Lighter API specification:
    /// - `order_book/{MARKET_INDEX}` for orderbook updates
    /// - `trade/{MARKET_INDEX}` for trade updates
    /// - `market_stats/{MARKET_INDEX}` for ticker/market stats
    /// - `account_all/{ACCOUNT_ID}` for account updates (requires auth)
    /// - `account_all_orders/{ACCOUNT_ID}` for order updates (requires auth)
    #[must_use]
    pub fn to_channel(&self) -> String {
        match self {
            Self::Orderbook { market_index } => format!("order_book/{}", market_index),
            Self::Trades { market_index } => format!("trade/{}", market_index),
            Self::Ticker { market_index } => format!("market_stats/{}", market_index),
            Self::Account => "account_all".to_string(),
            Self::Orders => "account_all_orders".to_string(),
        }
    }

    /// Convert subscription type to channel name with account ID for private channels.
    #[must_use]
    pub fn to_channel_with_account(&self, account_index: u64) -> String {
        match self {
            Self::Orderbook { market_index } => format!("order_book/{}", market_index),
            Self::Trades { market_index } => format!("trade/{}", market_index),
            Self::Ticker { market_index } => format!("market_stats/{}", market_index),
            Self::Account => format!("account_all/{}", account_index),
            Self::Orders => format!("account_all_orders/{}", account_index),
        }
    }

    /// Returns whether this subscription type requires authentication.
    #[must_use]
    pub const fn requires_auth(&self) -> bool {
        matches!(self, Self::Account | Self::Orders)
    }
}

/// Lighter WebSocket channel names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionChannel {
    /// Market statistics channel.
    MarketStats,
    /// Account all orders channel.
    AccountAllOrders,
    /// Account all updates channel.
    AccountAll,
    /// User statistics channel.
    UserStats,
}

impl SubscriptionChannel {
    /// Convert to string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::MarketStats => "market_stats",
            Self::AccountAllOrders => "account_all_orders",
            Self::AccountAll => "account_all",
            Self::UserStats => "user_stats",
        }
    }
}

/// Inbound messages received from the Lighter WebSocket.
#[derive(Debug, Clone)]
pub enum InboundMessage {
    /// Orderbook snapshot.
    OrderbookSnapshot {
        /// Market index.
        market_index: u16,
        /// Bids as (price, size) tuples.
        bids: Vec<(String, String)>,
        /// Asks as (price, size) tuples.
        asks: Vec<(String, String)>,
        /// Timestamp.
        timestamp: i64,
    },
    /// Orderbook update (delta).
    OrderbookUpdate {
        /// Market index.
        market_index: u16,
        /// Bid updates as (price, size) tuples.
        bids: Vec<(String, String)>,
        /// Ask updates as (price, size) tuples.
        asks: Vec<(String, String)>,
        /// Timestamp.
        timestamp: i64,
    },
    /// Trade execution.
    Trade {
        /// Market index.
        market_index: u16,
        /// Trade ID.
        trade_id: String,
        /// Price.
        price: String,
        /// Size.
        size: String,
        /// Side (true for buy, false for sell).
        is_buy: bool,
        /// Timestamp.
        timestamp: i64,
    },
    /// Ticker update.
    Ticker {
        /// Market index.
        market_index: u16,
        /// Last price.
        last_price: Option<String>,
        /// Best bid price.
        bid_price: Option<String>,
        /// Best bid size.
        bid_size: Option<String>,
        /// Best ask price.
        ask_price: Option<String>,
        /// Best ask size.
        ask_size: Option<String>,
        /// 24h volume.
        volume_24h: Option<String>,
        /// 24h high.
        high_24h: Option<String>,
        /// 24h low.
        low_24h: Option<String>,
        /// Timestamp.
        timestamp: i64,
    },
    /// Order update from private channel.
    OrderUpdate {
        /// Order ID.
        order_id: String,
        /// Client order ID.
        client_order_id: Option<String>,
        /// Market index.
        market_index: u16,
        /// Order status.
        status: String,
        /// Order side.
        side: String,
        /// Order type.
        order_type: String,
        /// Price.
        price: String,
        /// Quantity.
        quantity: String,
        /// Filled quantity.
        filled_quantity: String,
        /// Timestamp.
        timestamp: i64,
    },
    /// Account balance update from private channel.
    AccountUpdate {
        /// Account address.
        address: String,
        /// Balances by asset.
        balances: Vec<(String, String)>,
        /// Timestamp.
        timestamp: i64,
    },
    /// Pong response to ping.
    Pong,
    /// Error message from venue.
    Error {
        /// Error code.
        code: i32,
        /// Error message.
        message: String,
    },
    /// Subscription confirmation.
    SubscriptionSuccess {
        /// Channel that was subscribed to.
        channel: String,
    },
    /// Unsubscription confirmation.
    UnsubscriptionSuccess {
        /// Channel that was unsubscribed from.
        channel: String,
    },
    /// Authentication success.
    AuthSuccess,
    /// Raw unclassified message.
    Raw(Value),
}

/// Outbound messages sent to the Lighter WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundMessage {
    /// Subscribe to a single channel.
    Subscribe {
        /// Channel name to subscribe to.
        channel: String,
        /// Optional authentication token for private channels.
        #[serde(skip_serializing_if = "Option::is_none")]
        auth: Option<String>,
    },
    /// Unsubscribe from a single channel.
    Unsubscribe {
        /// Channel name to unsubscribe from.
        channel: String,
    },
    /// Ping message for heartbeat.
    Ping,
    /// Authentication message.
    Auth {
        /// Authentication token.
        token: String,
    },
}

impl OutboundMessage {
    /// Create a subscribe message for a public channel.
    #[must_use]
    pub fn subscribe(channel: impl Into<String>) -> Self {
        Self::Subscribe {
            channel: channel.into(),
            auth: None,
        }
    }

    /// Create a subscribe message for a private channel with authentication.
    #[must_use]
    pub fn subscribe_auth(channel: impl Into<String>, auth_token: impl Into<String>) -> Self {
        Self::Subscribe {
            channel: channel.into(),
            auth: Some(auth_token.into()),
        }
    }

    /// Create an unsubscribe message.
    #[must_use]
    pub fn unsubscribe(channel: impl Into<String>) -> Self {
        Self::Unsubscribe {
            channel: channel.into(),
        }
    }

    /// Serialize this message to JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl InboundMessage {
    /// Parse a JSON value into an InboundMessage.
    ///
    /// # Errors
    ///
    /// Returns an error if the message format is invalid or required fields are missing.
    ///
    /// # Lighter API Message Format
    ///
    /// Lighter uses a specific message format:
    /// - `type`: `"action/channel_type"` (e.g., `"subscribed/order_book"`, `"update/trade"`)
    /// - `channel`: `"channel_type:market_index"` (e.g., `"order_book:1"`, `"trade:1"`)
    /// - Data is nested in objects matching the channel type (e.g., `"order_book": {...}`)
    pub fn parse(value: &Value) -> Result<Self, LighterError> {
        // Get message type
        let msg_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing 'type' field".to_string()))?;

        // Handle Lighter's "action/channel" format (e.g., "update/order_book", "subscribed/trade")
        if msg_type.contains('/') {
            let parts: Vec<&str> = msg_type.splitn(2, '/').collect();
            if parts.len() == 2 {
                let action = parts[0];
                let channel_type = parts[1];

                return match action {
                    "subscribed" => Self::parse_lighter_subscription_success(value, channel_type),
                    "unsubscribed" => Self::parse_lighter_unsubscription_success(value, channel_type),
                    "update" | "snapshot" => {
                        Self::parse_lighter_data_message(value, action, channel_type)
                    }
                    _ => Ok(Self::Raw(value.clone())),
                };
            }
        }

        // Fallback to standard message types
        match msg_type {
            "orderbook_snapshot" => Self::parse_orderbook_snapshot(value),
            "orderbook_update" | "orderbook" => Self::parse_orderbook_update(value),
            "trade" => Self::parse_trade(value),
            "ticker" => Self::parse_ticker(value),
            "order_update" | "order" => Self::parse_order_update(value),
            "account_update" | "account" => Self::parse_account_update(value),
            "pong" => Ok(Self::Pong),
            "subscribed" => Self::parse_subscription_success(value),
            "unsubscribed" => Self::parse_unsubscription_success(value),
            "auth_success" | "authenticated" => Ok(Self::AuthSuccess),
            "error" => Self::parse_error(value),
            _ => {
                // Unknown message type, return as raw
                Ok(Self::Raw(value.clone()))
            }
        }
    }

    /// Parse Lighter subscription success message.
    fn parse_lighter_subscription_success(
        value: &Value,
        channel_type: &str,
    ) -> Result<Self, LighterError> {
        // Channel format: "order_book:1" or just the type if no market index
        let channel = value
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or(channel_type)
            .to_string();

        Ok(Self::SubscriptionSuccess { channel })
    }

    /// Parse Lighter unsubscription success message.
    fn parse_lighter_unsubscription_success(
        value: &Value,
        channel_type: &str,
    ) -> Result<Self, LighterError> {
        let channel = value
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or(channel_type)
            .to_string();

        Ok(Self::UnsubscriptionSuccess { channel })
    }

    /// Parse Lighter data message (update/snapshot).
    fn parse_lighter_data_message(
        value: &Value,
        action: &str,
        channel_type: &str,
    ) -> Result<Self, LighterError> {
        match channel_type {
            "order_book" => Self::parse_lighter_orderbook(value, action),
            "trade" => Self::parse_lighter_trades(value),
            "market_stats" => Self::parse_lighter_market_stats(value),
            "account_all" | "account" => Self::parse_lighter_account(value),
            "account_all_orders" | "orders" => Self::parse_lighter_orders(value),
            _ => Ok(Self::Raw(value.clone())),
        }
    }

    /// Extract market index from Lighter channel format.
    ///
    /// Channel format: "order_book:1" -> returns 1
    fn extract_market_index_from_channel(value: &Value) -> Option<u16> {
        value
            .get("channel")
            .and_then(|v| v.as_str())
            .and_then(|ch| ch.split(':').last())
            .and_then(|idx| idx.parse::<u16>().ok())
    }

    /// Parse Lighter orderbook message.
    fn parse_lighter_orderbook(value: &Value, action: &str) -> Result<Self, LighterError> {
        let market_index = Self::extract_market_index_from_channel(value)
            .ok_or_else(|| LighterError::Parse("Missing market index in channel".to_string()))?;

        // Orderbook data is nested in "order_book" object
        let ob_data = value
            .get("order_book")
            .ok_or_else(|| LighterError::Parse("Missing order_book data".to_string()))?;

        let bids = Self::parse_lighter_price_levels(ob_data.get("bids"));
        let asks = Self::parse_lighter_price_levels(ob_data.get("asks"));

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        if action == "snapshot" {
            Ok(Self::OrderbookSnapshot {
                market_index,
                bids,
                asks,
                timestamp,
            })
        } else {
            Ok(Self::OrderbookUpdate {
                market_index,
                bids,
                asks,
                timestamp,
            })
        }
    }

    /// Parse Lighter price levels from orderbook data.
    ///
    /// Lighter format: `[[price, size, order_count], ...]` or `[{price, size}, ...]`
    fn parse_lighter_price_levels(value: Option<&Value>) -> Vec<(String, String)> {
        let Some(arr) = value.and_then(|v| v.as_array()) else {
            return Vec::new();
        };

        let mut levels = Vec::new();
        for level in arr {
            // Try array format first: [price, size, ...]
            if let Some(level_arr) = level.as_array() {
                if level_arr.len() >= 2 {
                    let price = Self::value_to_string(&level_arr[0]);
                    let size = Self::value_to_string(&level_arr[1]);
                    levels.push((price, size));
                }
            }
            // Try object format: {price: "...", size: "..."}
            else if let Some(obj) = level.as_object() {
                let price = obj
                    .get("price")
                    .map(Self::value_to_string)
                    .unwrap_or_default();
                let size = obj
                    .get("size")
                    .or_else(|| obj.get("quantity"))
                    .map(Self::value_to_string)
                    .unwrap_or_default();
                if !price.is_empty() {
                    levels.push((price, size));
                }
            }
        }
        levels
    }

    /// Convert JSON value to string (handles both string and number types).
    fn value_to_string(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }

    /// Parse Lighter trades message.
    fn parse_lighter_trades(value: &Value) -> Result<Self, LighterError> {
        let market_index = Self::extract_market_index_from_channel(value)
            .ok_or_else(|| LighterError::Parse("Missing market index in channel".to_string()))?;

        // Trades can be in "trades" array or a single "trade" object
        let trades = value
            .get("trades")
            .and_then(|v| v.as_array())
            .or_else(|| value.get("trade").and_then(|v| v.as_array()));

        if let Some(trades_arr) = trades {
            // Return the first trade if multiple (or handle batch)
            if let Some(trade) = trades_arr.first() {
                return Self::parse_single_trade(trade, market_index);
            }
        }

        // Try parsing as single trade object
        if let Some(trade_obj) = value.get("trade").filter(|v| v.is_object()) {
            return Self::parse_single_trade(trade_obj, market_index);
        }

        // Fallback: try parsing from root
        Self::parse_single_trade(value, market_index)
    }

    /// Parse a single trade from Lighter format.
    fn parse_single_trade(trade: &Value, market_index: u16) -> Result<InboundMessage, LighterError> {
        let trade_id = trade
            .get("trade_id")
            .or_else(|| trade.get("id"))
            .or_else(|| trade.get("tx_hash"))
            .map(Self::value_to_string)
            .unwrap_or_default();

        let price = trade
            .get("price")
            .map(Self::value_to_string)
            .ok_or_else(|| LighterError::Parse("Missing trade price".to_string()))?;

        let size = trade
            .get("size")
            .or_else(|| trade.get("quantity"))
            .or_else(|| trade.get("base_amount"))
            .map(Self::value_to_string)
            .ok_or_else(|| LighterError::Parse("Missing trade size".to_string()))?;

        let is_buy = trade
            .get("side")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase() == "buy" || s == "bid" || s == "0")
            .or_else(|| trade.get("is_ask").and_then(|v| v.as_bool()).map(|b| !b))
            .unwrap_or(true);

        let timestamp = trade
            .get("timestamp")
            .or_else(|| trade.get("block_timestamp"))
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(InboundMessage::Trade {
            market_index,
            trade_id,
            price,
            size,
            is_buy,
            timestamp,
        })
    }

    /// Parse Lighter market stats (ticker) message.
    fn parse_lighter_market_stats(value: &Value) -> Result<Self, LighterError> {
        let market_index = Self::extract_market_index_from_channel(value)
            .ok_or_else(|| LighterError::Parse("Missing market index in channel".to_string()))?;

        // Stats data is nested in "market_stats" object
        let stats = value.get("market_stats").unwrap_or(value);

        let last_price = stats.get("last_price").map(Self::value_to_string);
        let bid_price = stats
            .get("best_bid")
            .or_else(|| stats.get("bid"))
            .or_else(|| stats.get("bid_price"))
            .map(Self::value_to_string);
        let bid_size = stats
            .get("best_bid_size")
            .or_else(|| stats.get("bid_size"))
            .or_else(|| stats.get("bid_quantity"))
            .map(Self::value_to_string);
        let ask_price = stats
            .get("best_ask")
            .or_else(|| stats.get("ask"))
            .or_else(|| stats.get("ask_price"))
            .map(Self::value_to_string);
        let ask_size = stats
            .get("best_ask_size")
            .or_else(|| stats.get("ask_size"))
            .or_else(|| stats.get("ask_quantity"))
            .map(Self::value_to_string);
        let volume_24h = stats
            .get("volume_24h")
            .or_else(|| stats.get("volume"))
            .map(Self::value_to_string);
        let high_24h = stats
            .get("high_24h")
            .or_else(|| stats.get("high"))
            .map(Self::value_to_string);
        let low_24h = stats
            .get("low_24h")
            .or_else(|| stats.get("low"))
            .map(Self::value_to_string);

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::Ticker {
            market_index,
            last_price,
            bid_price,
            bid_size,
            ask_price,
            ask_size,
            volume_24h,
            high_24h,
            low_24h,
            timestamp,
        })
    }

    /// Parse Lighter account update message.
    fn parse_lighter_account(value: &Value) -> Result<Self, LighterError> {
        let account_data = value.get("account").unwrap_or(value);

        let address = account_data
            .get("address")
            .or_else(|| account_data.get("account_address"))
            .or_else(|| account_data.get("account_index"))
            .map(Self::value_to_string)
            .ok_or_else(|| LighterError::Parse("Missing account address".to_string()))?;

        let balances = if let Some(bal_obj) = account_data
            .get("balances")
            .and_then(|v| v.as_object())
        {
            bal_obj
                .iter()
                .map(|(k, v)| (k.clone(), Self::value_to_string(v)))
                .collect()
        } else if let Some(bal_arr) = account_data.get("balances").and_then(|v| v.as_array()) {
            bal_arr
                .iter()
                .filter_map(|b| {
                    let asset = b.get("asset").map(Self::value_to_string)?;
                    let amount = b
                        .get("amount")
                        .or_else(|| b.get("balance"))
                        .map(Self::value_to_string)?;
                    Some((asset, amount))
                })
                .collect()
        } else {
            Vec::new()
        };

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::AccountUpdate {
            address,
            balances,
            timestamp,
        })
    }

    /// Parse Lighter orders update message.
    fn parse_lighter_orders(value: &Value) -> Result<Self, LighterError> {
        // Orders can be in "orders" array or single "order" object
        let order = value
            .get("order")
            .or_else(|| value.get("orders").and_then(|v| v.as_array()).and_then(|a| a.first()))
            .unwrap_or(value);

        Self::parse_order_update(order)
    }

    fn parse_orderbook_snapshot(value: &Value) -> Result<Self, LighterError> {
        let market_index = value
            .get("market_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LighterError::Parse("Missing market_index".to_string()))?
            as u16;

        let bids = Self::parse_price_levels(value.get("bids"))?;
        let asks = Self::parse_price_levels(value.get("asks"))?;

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::OrderbookSnapshot {
            market_index,
            bids,
            asks,
            timestamp,
        })
    }

    fn parse_orderbook_update(value: &Value) -> Result<Self, LighterError> {
        let market_index = value
            .get("market_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LighterError::Parse("Missing market_index".to_string()))?
            as u16;

        let bids = Self::parse_price_levels(value.get("bids"))?;
        let asks = Self::parse_price_levels(value.get("asks"))?;

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::OrderbookUpdate {
            market_index,
            bids,
            asks,
            timestamp,
        })
    }

    fn parse_price_levels(value: Option<&Value>) -> Result<Vec<(String, String)>, LighterError> {
        let arr = value
            .and_then(|v| v.as_array())
            .ok_or_else(|| LighterError::Parse("Invalid price levels array".to_string()))?;

        let mut levels = Vec::new();
        for level in arr {
            if let Some(level_arr) = level.as_array() {
                if level_arr.len() >= 2 {
                    let price = level_arr[0]
                        .as_str()
                        .ok_or_else(|| LighterError::Parse("Invalid price".to_string()))?
                        .to_string();
                    let size = level_arr[1]
                        .as_str()
                        .ok_or_else(|| LighterError::Parse("Invalid size".to_string()))?
                        .to_string();
                    levels.push((price, size));
                }
            }
        }
        Ok(levels)
    }

    fn parse_trade(value: &Value) -> Result<Self, LighterError> {
        let market_index = value
            .get("market_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LighterError::Parse("Missing market_index".to_string()))?
            as u16;

        let trade_id = value
            .get("trade_id")
            .or_else(|| value.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let price = value
            .get("price")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing price".to_string()))?
            .to_string();

        let size = value
            .get("size")
            .or_else(|| value.get("quantity"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing size".to_string()))?
            .to_string();

        let is_buy = value
            .get("side")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase() == "buy" || s == "bid")
            .ok_or_else(|| LighterError::Parse("Missing side".to_string()))?;

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::Trade {
            market_index,
            trade_id,
            price,
            size,
            is_buy,
            timestamp,
        })
    }

    fn parse_ticker(value: &Value) -> Result<Self, LighterError> {
        let market_index = value
            .get("market_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LighterError::Parse("Missing market_index".to_string()))?
            as u16;

        let last_price = value.get("last_price").and_then(|v| v.as_str()).map(String::from);
        let bid_price = value.get("best_bid").and_then(|v| v.as_str()).map(String::from);
        let bid_size = value.get("best_bid_size").and_then(|v| v.as_str()).map(String::from);
        let ask_price = value.get("best_ask").and_then(|v| v.as_str()).map(String::from);
        let ask_size = value.get("best_ask_size").and_then(|v| v.as_str()).map(String::from);
        let volume_24h = value.get("volume_24h").and_then(|v| v.as_str()).map(String::from);
        let high_24h = value.get("high_24h").and_then(|v| v.as_str()).map(String::from);
        let low_24h = value.get("low_24h").and_then(|v| v.as_str()).map(String::from);

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::Ticker {
            market_index,
            last_price,
            bid_price,
            bid_size,
            ask_price,
            ask_size,
            volume_24h,
            high_24h,
            low_24h,
            timestamp,
        })
    }

    fn parse_order_update(value: &Value) -> Result<Self, LighterError> {
        let order_id = value
            .get("order_id")
            .or_else(|| value.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing order_id".to_string()))?
            .to_string();

        let client_order_id = value
            .get("client_order_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let market_index = value
            .get("market_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LighterError::Parse("Missing market_index".to_string()))?
            as u16;

        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing status".to_string()))?
            .to_string();

        let side = value
            .get("side")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing side".to_string()))?
            .to_string();

        let order_type = value
            .get("order_type")
            .or_else(|| value.get("type"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing order_type".to_string()))?
            .to_string();

        let price = value
            .get("price")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing price".to_string()))?
            .to_string();

        let quantity = value
            .get("quantity")
            .or_else(|| value.get("size"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing quantity".to_string()))?
            .to_string();

        let filled_quantity = value
            .get("filled_quantity")
            .or_else(|| value.get("filled"))
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string();

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::OrderUpdate {
            order_id,
            client_order_id,
            market_index,
            status,
            side,
            order_type,
            price,
            quantity,
            filled_quantity,
            timestamp,
        })
    }

    fn parse_account_update(value: &Value) -> Result<Self, LighterError> {
        let address = value
            .get("address")
            .or_else(|| value.get("account"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing address".to_string()))?
            .to_string();

        let balances = if let Some(bal_obj) = value.get("balances").and_then(|v| v.as_object()) {
            bal_obj
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("0").to_string()))
                .collect()
        } else {
            Vec::new()
        };

        let timestamp = value
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        Ok(Self::AccountUpdate {
            address,
            balances,
            timestamp,
        })
    }

    fn parse_subscription_success(value: &Value) -> Result<Self, LighterError> {
        let channel = value
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing channel".to_string()))?
            .to_string();

        Ok(Self::SubscriptionSuccess { channel })
    }

    fn parse_unsubscription_success(value: &Value) -> Result<Self, LighterError> {
        let channel = value
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing channel".to_string()))?
            .to_string();

        Ok(Self::UnsubscriptionSuccess { channel })
    }

    fn parse_error(value: &Value) -> Result<Self, LighterError> {
        let code = value
            .get("code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        let message = value
            .get("message")
            .or_else(|| value.get("msg"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error")
            .to_string();

        Ok(Self::Error { code, message })
    }
}

/// WebSocket error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketError {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
}

impl WebSocketError {
    /// Create a new WebSocket error.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_type_to_channel() {
        let sub = SubscriptionType::Orderbook { market_index: 42 };
        assert_eq!(sub.to_channel(), "order_book/42");

        let sub = SubscriptionType::Trades { market_index: 100 };
        assert_eq!(sub.to_channel(), "trade/100");

        let sub = SubscriptionType::Ticker { market_index: 1 };
        assert_eq!(sub.to_channel(), "market_stats/1");

        let sub = SubscriptionType::Account;
        assert_eq!(sub.to_channel(), "account_all");

        let sub = SubscriptionType::Orders;
        assert_eq!(sub.to_channel(), "account_all_orders");
    }

    #[test]
    fn test_subscription_type_to_channel_with_account() {
        let sub = SubscriptionType::Account;
        assert_eq!(sub.to_channel_with_account(878), "account_all/878");

        let sub = SubscriptionType::Orders;
        assert_eq!(sub.to_channel_with_account(878), "account_all_orders/878");

        // Public channels don't use account index
        let sub = SubscriptionType::Orderbook { market_index: 1 };
        assert_eq!(sub.to_channel_with_account(878), "order_book/1");
    }

    #[test]
    fn test_subscription_requires_auth() {
        assert!(!SubscriptionType::Orderbook { market_index: 0 }.requires_auth());
        assert!(!SubscriptionType::Trades { market_index: 0 }.requires_auth());
        assert!(!SubscriptionType::Ticker { market_index: 0 }.requires_auth());
        assert!(SubscriptionType::Account.requires_auth());
        assert!(SubscriptionType::Orders.requires_auth());
    }

    #[test]
    fn test_outbound_message_serialization() {
        let msg = OutboundMessage::subscribe("order_book/1");
        let json = msg.to_json().unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("order_book/1"));

        let msg = OutboundMessage::Ping;
        let json = msg.to_json().unwrap();
        assert!(json.contains("ping"));

        let msg = OutboundMessage::Auth {
            token: "test_token".to_string(),
        };
        let json = msg.to_json().unwrap();
        assert!(json.contains("auth"));
        assert!(json.contains("test_token"));
    }

    #[test]
    fn test_parse_lighter_orderbook_update() {
        let json = r#"{
            "type": "update/order_book",
            "channel": "order_book:1",
            "offset": 7013,
            "order_book": {
                "bids": [[4127.50, 10.5], [4127.00, 20.0]],
                "asks": [[4128.00, 5.2], [4128.50, 15.0]]
            },
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::OrderbookUpdate {
                market_index,
                bids,
                asks,
                timestamp,
            } => {
                assert_eq!(market_index, 1);
                assert_eq!(bids.len(), 2);
                // JSON numbers may serialize as "4127.5" or "4127.50" depending on the parser
                assert!(bids[0].0.starts_with("4127"));
                assert_eq!(asks.len(), 2);
                assert!(asks[0].0.starts_with("4128"));
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected OrderbookUpdate"),
        }
    }

    #[test]
    fn test_parse_lighter_subscription_success() {
        let json = r#"{
            "type": "subscribed/order_book",
            "channel": "order_book:1"
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::SubscriptionSuccess { channel } => {
                assert_eq!(channel, "order_book:1");
            }
            _ => panic!("Expected SubscriptionSuccess"),
        }
    }

    #[test]
    fn test_parse_lighter_market_stats() {
        let json = r#"{
            "type": "update/market_stats",
            "channel": "market_stats:1",
            "market_stats": {
                "last_price": "4127.50",
                "best_bid": "4127.00",
                "best_bid_size": "10.5",
                "best_ask": "4128.00",
                "best_ask_size": "5.2",
                "volume_24h": "1000.0",
                "high_24h": "4200.00",
                "low_24h": "4100.00"
            },
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::Ticker {
                market_index,
                last_price,
                bid_price,
                bid_size,
                ask_price,
                ask_size,
                volume_24h,
                high_24h,
                low_24h,
                timestamp,
            } => {
                assert_eq!(market_index, 1);
                assert_eq!(last_price, Some("4127.50".to_string()));
                assert_eq!(bid_price, Some("4127.00".to_string()));
                assert_eq!(bid_size, Some("10.5".to_string()));
                assert_eq!(ask_price, Some("4128.00".to_string()));
                assert_eq!(ask_size, Some("5.2".to_string()));
                assert_eq!(volume_24h, Some("1000.0".to_string()));
                assert_eq!(high_24h, Some("4200.00".to_string()));
                assert_eq!(low_24h, Some("4100.00".to_string()));
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected Ticker"),
        }
    }

    #[test]
    fn test_parse_orderbook_snapshot() {
        let json = r#"{
            "type": "orderbook_snapshot",
            "market_index": 1,
            "bids": [["4127.50", "10.5"], ["4127.00", "20.0"]],
            "asks": [["4128.00", "5.2"], ["4128.50", "15.0"]],
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::OrderbookSnapshot {
                market_index,
                bids,
                asks,
                timestamp,
            } => {
                assert_eq!(market_index, 1);
                assert_eq!(bids.len(), 2);
                assert_eq!(bids[0], ("4127.50".to_string(), "10.5".to_string()));
                assert_eq!(asks.len(), 2);
                assert_eq!(asks[0], ("4128.00".to_string(), "5.2".to_string()));
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected OrderbookSnapshot"),
        }
    }

    #[test]
    fn test_parse_trade() {
        let json = r#"{
            "type": "trade",
            "market_index": 1,
            "trade_id": "12345",
            "price": "4127.50",
            "size": "10.5",
            "side": "buy",
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::Trade {
                market_index,
                trade_id,
                price,
                size,
                is_buy,
                timestamp,
            } => {
                assert_eq!(market_index, 1);
                assert_eq!(trade_id, "12345");
                assert_eq!(price, "4127.50");
                assert_eq!(size, "10.5");
                assert!(is_buy);
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected Trade"),
        }
    }

    #[test]
    fn test_parse_ticker() {
        let json = r#"{
            "type": "ticker",
            "market_index": 1,
            "last_price": "4127.50",
            "volume_24h": "1000.0",
            "high_24h": "4200.00",
            "low_24h": "4100.00",
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::Ticker {
                market_index,
                last_price,
                bid_price,
                bid_size,
                ask_price,
                ask_size,
                volume_24h,
                high_24h,
                low_24h,
                timestamp,
            } => {
                assert_eq!(market_index, 1);
                assert_eq!(last_price, Some("4127.50".to_string()));
                assert_eq!(bid_price, None);
                assert_eq!(bid_size, None);
                assert_eq!(ask_price, None);
                assert_eq!(ask_size, None);
                assert_eq!(volume_24h, Some("1000.0".to_string()));
                assert_eq!(high_24h, Some("4200.00".to_string()));
                assert_eq!(low_24h, Some("4100.00".to_string()));
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected Ticker"),
        }
    }

    #[test]
    fn test_parse_order_update() {
        let json = r#"{
            "type": "order_update",
            "order_id": "order123",
            "client_order_id": "client123",
            "market_index": 1,
            "status": "filled",
            "side": "buy",
            "order_type": "limit",
            "price": "4127.50",
            "quantity": "10.0",
            "filled_quantity": "10.0",
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::OrderUpdate {
                order_id,
                client_order_id,
                market_index,
                status,
                side,
                order_type,
                price,
                quantity,
                filled_quantity,
                timestamp,
            } => {
                assert_eq!(order_id, "order123");
                assert_eq!(client_order_id, Some("client123".to_string()));
                assert_eq!(market_index, 1);
                assert_eq!(status, "filled");
                assert_eq!(side, "buy");
                assert_eq!(order_type, "limit");
                assert_eq!(price, "4127.50");
                assert_eq!(quantity, "10.0");
                assert_eq!(filled_quantity, "10.0");
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected OrderUpdate"),
        }
    }

    #[test]
    fn test_parse_account_update() {
        let json = r#"{
            "type": "account_update",
            "address": "0x1234",
            "balances": {
                "USDC": "10000.0",
                "ETH": "5.0"
            },
            "timestamp": 1734200000000
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::AccountUpdate {
                address,
                balances,
                timestamp,
            } => {
                assert_eq!(address, "0x1234");
                assert_eq!(balances.len(), 2);
                assert!(balances.contains(&("USDC".to_string(), "10000.0".to_string())));
                assert_eq!(timestamp, 1734200000000);
            }
            _ => panic!("Expected AccountUpdate"),
        }
    }

    #[test]
    fn test_parse_subscription_success() {
        let json = r#"{
            "type": "subscribed",
            "channel": "order_book/1"
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::SubscriptionSuccess { channel } => {
                assert_eq!(channel, "order_book/1");
            }
            _ => panic!("Expected SubscriptionSuccess"),
        }
    }

    #[test]
    fn test_parse_error() {
        let json = r#"{
            "type": "error",
            "code": 400,
            "message": "Invalid request"
        }"#;

        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::Error { code, message } => {
                assert_eq!(code, 400);
                assert_eq!(message, "Invalid request");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_parse_pong() {
        let json = r#"{"type": "pong"}"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::Pong => {}
            _ => panic!("Expected Pong"),
        }
    }

    #[test]
    fn test_parse_auth_success() {
        let json = r#"{"type": "auth_success"}"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::AuthSuccess => {}
            _ => panic!("Expected AuthSuccess"),
        }
    }

    #[test]
    fn test_parse_unknown_message() {
        let json = r#"{"type": "unknown_type", "data": "test"}"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let msg = InboundMessage::parse(&value).unwrap();

        match msg {
            InboundMessage::Raw(_) => {}
            _ => panic!("Expected Raw"),
        }
    }
}
