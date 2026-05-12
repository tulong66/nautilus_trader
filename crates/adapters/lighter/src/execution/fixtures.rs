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

use crate::websocket::messages::InboundMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderFixtureStatus {
    Accepted,
    Rejected,
    PartiallyFilled,
    Filled,
    Canceled,
    CancelRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderUpdateFixture {
    pub order_id: &'static str,
    pub client_order_id: &'static str,
    pub market_index: u16,
    pub status: OrderFixtureStatus,
    pub side: &'static str,
    pub order_type: &'static str,
    pub price: &'static str,
    pub quantity: &'static str,
    pub filled_quantity: &'static str,
    pub timestamp_ms: i64,
}

impl OrderUpdateFixture {
    #[must_use]
    pub fn to_ws_message(&self) -> InboundMessage {
        InboundMessage::OrderUpdate {
            order_id: self.order_id.to_string(),
            client_order_id: Some(self.client_order_id.to_string()),
            market_index: self.market_index,
            status: self.status.as_lighter_status().to_string(),
            side: self.side.to_string(),
            order_type: self.order_type.to_string(),
            price: self.price.to_string(),
            quantity: self.quantity.to_string(),
            filled_quantity: self.filled_quantity.to_string(),
            timestamp: self.timestamp_ms,
        }
    }
}

impl From<OrderFixtureStatus> for crate::execution::dispatch::OrderDispatchStatus {
    fn from(value: OrderFixtureStatus) -> Self {
        match value {
            OrderFixtureStatus::Accepted => Self::Accepted,
            OrderFixtureStatus::Rejected => Self::Rejected,
            OrderFixtureStatus::PartiallyFilled => Self::PartiallyFilled,
            OrderFixtureStatus::Filled => Self::Filled,
            OrderFixtureStatus::Canceled => Self::Canceled,
            OrderFixtureStatus::CancelRejected => Self::CancelRejected,
        }
    }
}

impl OrderFixtureStatus {
    #[must_use]
    pub const fn as_lighter_status(self) -> &'static str {
        match self {
            Self::Accepted => "open",
            Self::Rejected => "rejected",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Canceled => "canceled",
            Self::CancelRejected => "cancel_rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUpdateFixture {
    pub account_index: u64,
    pub address: String,
    pub asset: String,
    pub balance: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillFixture {
    pub trade_id: String,
    pub order_id: String,
    pub client_order_id: String,
    pub market_index: u16,
    pub price: String,
    pub quantity: String,
    pub side: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionFixture {
    pub market_index: u16,
    pub symbol: String,
    pub size: String,
    pub entry_price: String,
    pub unrealized_pnl: String,
    pub margin: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFixtureSet {
    pub orders: Vec<OrderUpdateFixture>,
    pub account: AccountUpdateFixture,
    pub fill: FillFixture,
    pub position: PositionFixture,
}

impl ExecutionFixtureSet {
    #[must_use]
    pub const fn is_offline_only(&self) -> bool {
        true
    }

    #[must_use]
    pub const fn secret_material(&self) -> &'static [&'static str] {
        &[]
    }
}

#[must_use]
pub fn execution_fixture_set() -> ExecutionFixtureSet {
    ExecutionFixtureSet {
        orders: vec![
            OrderUpdateFixture {
                order_id: "fixture-order-accepted",
                client_order_id: "fixture-client-accepted",
                market_index: 1,
                status: OrderFixtureStatus::Accepted,
                side: "buy",
                order_type: "limit",
                price: "100.50",
                quantity: "0.2500",
                filled_quantity: "0",
                timestamp_ms: 1_700_000_000_000,
            },
            OrderUpdateFixture {
                order_id: "fixture-order-rejected",
                client_order_id: "fixture-client-rejected",
                market_index: 1,
                status: OrderFixtureStatus::Rejected,
                side: "buy",
                order_type: "limit",
                price: "100.50",
                quantity: "0.2500",
                filled_quantity: "0",
                timestamp_ms: 1_700_000_000_010,
            },
            OrderUpdateFixture {
                order_id: "fixture-order-partial",
                client_order_id: "fixture-client-partial",
                market_index: 1,
                status: OrderFixtureStatus::PartiallyFilled,
                side: "buy",
                order_type: "limit",
                price: "100.75",
                quantity: "0.2500",
                filled_quantity: "0.1000",
                timestamp_ms: 1_700_000_000_020,
            },
            OrderUpdateFixture {
                order_id: "fixture-order-filled",
                client_order_id: "fixture-client-filled",
                market_index: 1,
                status: OrderFixtureStatus::Filled,
                side: "buy",
                order_type: "limit",
                price: "101.25",
                quantity: "0.2500",
                filled_quantity: "0.2500",
                timestamp_ms: 1_700_000_000_030,
            },
            OrderUpdateFixture {
                order_id: "fixture-order-canceled",
                client_order_id: "fixture-client-canceled",
                market_index: 1,
                status: OrderFixtureStatus::Canceled,
                side: "sell",
                order_type: "limit",
                price: "102.00",
                quantity: "0.2500",
                filled_quantity: "0",
                timestamp_ms: 1_700_000_000_040,
            },
            OrderUpdateFixture {
                order_id: "fixture-order-cancel-rejected",
                client_order_id: "fixture-client-cancel-rejected",
                market_index: 1,
                status: OrderFixtureStatus::CancelRejected,
                side: "sell",
                order_type: "limit",
                price: "102.00",
                quantity: "0.2500",
                filled_quantity: "0",
                timestamp_ms: 1_700_000_000_050,
            },
        ],
        account: AccountUpdateFixture {
            account_index: 42,
            address: "fixture-account".to_string(),
            asset: "USDC".to_string(),
            balance: "1000.00".to_string(),
            timestamp_ms: 1_700_000_000_100,
        },
        fill: FillFixture {
            trade_id: "fixture-trade-1".to_string(),
            order_id: "fixture-order-filled".to_string(),
            client_order_id: "fixture-client-filled".to_string(),
            market_index: 1,
            price: "101.25".to_string(),
            quantity: "0.2500".to_string(),
            side: "buy".to_string(),
            timestamp_ms: 1_700_000_000_060,
        },
        position: PositionFixture {
            market_index: 1,
            symbol: "ETH_USDC".to_string(),
            size: "0.2500".to_string(),
            entry_price: "101.25".to_string(),
            unrealized_pnl: "0.00".to_string(),
            margin: "25.00".to_string(),
            timestamp_ms: 1_700_000_000_120,
        },
    }
}
