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

//! Common types for the Lighter adapter.

use serde::{Deserialize, Serialize};

use super::enums::LighterMarketStatus;

/// Market information from Lighter.
///
/// Contains metadata about a trading market including the critical
/// `price_decimals` field which varies per market.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketInfo {
    /// Market ID (numeric).
    pub market_id: u32,
    /// Market symbol (e.g., "ETH_USDC").
    pub symbol: String,
    /// Base asset symbol (e.g., "ETH").
    pub base_symbol: String,
    /// Quote asset symbol (e.g., "USDC").
    pub quote_symbol: String,
    /// Price decimals (varies per market!).
    ///
    /// This is crucial for price conversion:
    /// - ETH: 2 decimals ($4127.39 -> 412739)
    /// - BTC: 1 decimal ($114357.8 -> 1143578)
    /// - DOGE: 6 decimals ($0.202095 -> 202095)
    pub price_decimals: u8,
    /// Size decimals (typically 8).
    pub size_decimals: u8,
    /// Minimum order size in base units.
    pub min_base_amount: u64,
    /// Tick size in quote units.
    pub tick_size: u64,
    /// Market status.
    pub status: LighterMarketStatus,
}

impl MarketInfo {
    /// Get the quantity multiplier for this market.
    ///
    /// Formula: 10^(6 - price_decimals)
    #[must_use]
    pub fn quantity_multiplier(&self) -> u64 {
        10u64.pow(6 - self.price_decimals as u32)
    }
}

/// Signed transaction result from the signer.
#[derive(Clone, Debug)]
pub struct SignedTransaction {
    /// Transaction type (e.g., "CreateOrder", "CancelOrder").
    pub tx_type: String,
    /// Transaction info as JSON string.
    pub tx_info: String,
    /// Transaction hash/signature.
    pub tx_hash: String,
}

/// Order book level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// Price in quote units (needs conversion with price_decimals).
    pub price: u64,
    /// Size in base units.
    pub size: u64,
}

/// Trade record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trade {
    /// Trade ID.
    pub id: u64,
    /// Market ID.
    pub market_id: u32,
    /// Price in quote units.
    pub price: u64,
    /// Size in base units.
    pub size: u64,
    /// Whether this was a taker sell (ask).
    pub is_taker_ask: bool,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Account information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Account index.
    pub account_index: u32,
    /// Total equity in quote currency.
    pub equity: i64,
    /// Available margin.
    pub available_margin: i64,
    /// Position margin used.
    pub position_margin: i64,
    /// Order margin reserved.
    pub order_margin: i64,
    /// Unrealized PnL.
    pub unrealized_pnl: i64,
}

/// Order information from the exchange.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderInfo {
    /// Exchange order index.
    pub order_index: u64,
    /// Client order index.
    pub client_order_index: u64,
    /// Market ID.
    pub market_id: u32,
    /// Order side (is_ask).
    pub is_ask: bool,
    /// Order price.
    pub price: u64,
    /// Original order size.
    pub original_size: u64,
    /// Remaining size.
    pub remaining_size: u64,
    /// Filled size.
    pub filled_size: u64,
    /// Order type.
    pub order_type: u8,
    /// Time in force.
    pub time_in_force: u8,
    /// Whether reduce-only.
    pub reduce_only: bool,
    /// Creation timestamp.
    pub created_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_info_quantity_multiplier() {
        let eth_market = MarketInfo {
            market_id: 1,
            symbol: "ETH_USDC".to_string(),
            base_symbol: "ETH".to_string(),
            quote_symbol: "USDC".to_string(),
            price_decimals: 2,
            size_decimals: 8,
            min_base_amount: 1000,
            tick_size: 1,
            status: LighterMarketStatus::Active,
        };
        assert_eq!(eth_market.quantity_multiplier(), 10000); // 10^(6-2) = 10^4

        let doge_market = MarketInfo {
            market_id: 5,
            symbol: "DOGE_USDC".to_string(),
            base_symbol: "DOGE".to_string(),
            quote_symbol: "USDC".to_string(),
            price_decimals: 6,
            size_decimals: 8,
            min_base_amount: 1000000,
            tick_size: 1,
            status: LighterMarketStatus::Active,
        };
        assert_eq!(doge_market.quantity_multiplier(), 1); // 10^(6-6) = 10^0
    }
}
