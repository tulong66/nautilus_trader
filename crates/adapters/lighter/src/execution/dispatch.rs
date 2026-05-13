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
pub enum OrderDispatchStatus {
    Accepted,
    Rejected,
    PartiallyFilled,
    Filled,
    Canceled,
    CancelRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    Order {
        order_id: String,
        client_order_id: Option<String>,
        market_index: u16,
        status: OrderDispatchStatus,
        venue_status: String,
        timestamp_ms: i64,
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
            timestamp,
            ..
        } => DispatchOutcome::Order {
            order_id: order_id.clone(),
            client_order_id: client_order_id.clone(),
            market_index: *market_index,
            status: order_status_from_lighter(status),
            venue_status: status.clone(),
            timestamp_ms: *timestamp,
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

fn order_status_from_lighter(status: &str) -> OrderDispatchStatus {
    match status {
        "open" => OrderDispatchStatus::Accepted,
        "rejected" => OrderDispatchStatus::Rejected,
        "partially_filled" => OrderDispatchStatus::PartiallyFilled,
        "filled" => OrderDispatchStatus::Filled,
        "canceled" | "cancelled" => OrderDispatchStatus::Canceled,
        "cancel_rejected" => OrderDispatchStatus::CancelRejected,
        _ => OrderDispatchStatus::Rejected,
    }
}
