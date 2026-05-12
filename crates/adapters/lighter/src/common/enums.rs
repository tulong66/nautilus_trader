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

//! Enumerations for the Lighter adapter.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Lighter environment (network).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(
        eq,
        eq_int,
        module = "nautilus_trader.core.nautilus_pyo3.lighter",
        from_py_object,
        rename_all = "SCREAMING_SNAKE_CASE",
    )
)]
#[cfg_attr(
    feature = "python",
    pyo3_stub_gen::derive::gen_stub_pyclass_enum(module = "nautilus_trader.lighter")
)]
pub enum LighterEnvironment {
    /// Lighter mainnet.
    #[default]
    Mainnet,
    /// Lighter testnet.
    Testnet,
}

impl LighterEnvironment {
    /// Get the chain ID for this environment.
    #[must_use]
    pub const fn chain_id(&self) -> u32 {
        match self {
            Self::Mainnet => 304,
            Self::Testnet => 300,
        }
    }

    /// Get the HTTP base URL for this environment.
    #[must_use]
    pub const fn http_url(&self) -> &'static str {
        match self {
            Self::Mainnet => "https://mainnet.zklighter.elliot.ai",
            Self::Testnet => "https://testnet.zklighter.elliot.ai",
        }
    }

    /// Get the WebSocket base URL for this environment.
    #[must_use]
    pub const fn ws_url(&self) -> &'static str {
        match self {
            Self::Mainnet => "wss://mainnet.zklighter.elliot.ai/stream",
            Self::Testnet => "wss://testnet.zklighter.elliot.ai/stream",
        }
    }
}

/// Lighter order type.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[repr(u8)]
pub enum LighterOrderType {
    /// Limit order.
    #[default]
    Limit = 0,
    /// Market order.
    Market = 1,
}

impl LighterOrderType {
    /// Convert to u8 for API calls.
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Lighter time-in-force.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[repr(u8)]
pub enum LighterTimeInForce {
    /// Good till time (default 28 days).
    #[default]
    GoodTillTime = 0,
    /// Immediate or cancel.
    ImmediateOrCancel = 1,
    /// Fill or kill.
    FillOrKill = 2,
    /// Post only (maker only).
    PostOnly = 3,
}

impl LighterTimeInForce {
    /// Convert to u8 for API calls.
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Lighter order side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
pub enum LighterOrderSide {
    /// Buy order.
    Buy,
    /// Sell order.
    Sell,
}

impl LighterOrderSide {
    /// Convert to is_ask boolean for Lighter API.
    #[must_use]
    pub const fn is_ask(&self) -> bool {
        matches!(self, Self::Sell)
    }
}

/// Lighter order status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
pub enum LighterOrderStatus {
    /// Order is open/active.
    Open,
    /// Order is partially filled.
    PartiallyFilled,
    /// Order is fully filled.
    Filled,
    /// Order was canceled.
    Canceled,
    /// Order expired.
    Expired,
    /// Order was rejected.
    Rejected,
}

/// Lighter market status.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
pub enum LighterMarketStatus {
    /// Market is active and trading.
    #[default]
    Active,
    /// Market is paused.
    Paused,
    /// Market is closed.
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_chain_ids() {
        assert_eq!(LighterEnvironment::Mainnet.chain_id(), 304);
        assert_eq!(LighterEnvironment::Testnet.chain_id(), 300);
    }

    #[test]
    fn test_environment_urls() {
        assert!(LighterEnvironment::Mainnet.http_url().contains("mainnet"));
        assert!(LighterEnvironment::Testnet.http_url().contains("testnet"));
    }

    #[test]
    fn test_order_type_values() {
        assert_eq!(LighterOrderType::Limit.as_u8(), 0);
        assert_eq!(LighterOrderType::Market.as_u8(), 1);
    }

    #[test]
    fn test_time_in_force_values() {
        assert_eq!(LighterTimeInForce::GoodTillTime.as_u8(), 0);
        assert_eq!(LighterTimeInForce::ImmediateOrCancel.as_u8(), 1);
        assert_eq!(LighterTimeInForce::FillOrKill.as_u8(), 2);
        assert_eq!(LighterTimeInForce::PostOnly.as_u8(), 3);
    }

    #[test]
    fn test_order_side_is_ask() {
        assert!(!LighterOrderSide::Buy.is_ask());
        assert!(LighterOrderSide::Sell.is_ask());
    }
}
