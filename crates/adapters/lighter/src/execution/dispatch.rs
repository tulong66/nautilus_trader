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

use crate::{
    execution::fixtures::OrderFixtureStatus,
    websocket::messages::InboundMessage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    Order {
        order_id: String,
        client_order_id: Option<String>,
        market_index: u16,
        status: OrderFixtureStatus,
        venue_status: String,
    },
    Account {
        address: String,
        balances: Vec<(String, String)>,
        timestamp_ms: i64,
    },
    Ignored,
}

#[must_use]
pub fn dispatch_private_message(message: &InboundMessage) -> DispatchOutcome {
    match message {
        InboundMessage::OrderUpdate {
            order_id,
            client_order_id,
            market_index,
            status,
            ..
        } => DispatchOutcome::Order {
            order_id: order_id.clone(),
            client_order_id: client_order_id.clone(),
            market_index: *market_index,
            status: order_status_from_lighter(status),
            venue_status: status.clone(),
        },
        InboundMessage::AccountUpdate {
            address,
            balances,
            timestamp,
        } => DispatchOutcome::Account {
            address: address.clone(),
            balances: balances.clone(),
            timestamp_ms: *timestamp,
        },
        _ => DispatchOutcome::Ignored,
    }
}

fn order_status_from_lighter(status: &str) -> OrderFixtureStatus {
    match status {
        "open" => OrderFixtureStatus::Accepted,
        "rejected" => OrderFixtureStatus::Rejected,
        "partially_filled" => OrderFixtureStatus::PartiallyFilled,
        "filled" => OrderFixtureStatus::Filled,
        "canceled" | "cancelled" => OrderFixtureStatus::Canceled,
        "cancel_rejected" => OrderFixtureStatus::CancelRejected,
        _ => OrderFixtureStatus::Rejected,
    }
}
