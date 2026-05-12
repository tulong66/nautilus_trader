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

use nautilus_lighter::{
    execution::{
        dispatch::{DispatchOutcome, dispatch_private_message},
        fixtures::{OrderFixtureStatus, execution_fixture_set},
    },
    websocket::messages::InboundMessage,
};

#[test]
fn dispatches_order_update_fixtures_to_order_outcomes() {
    let fixtures = execution_fixture_set();

    for order in &fixtures.orders {
        let outcome = dispatch_private_message(&order.to_ws_message());

        assert_eq!(
            outcome,
            DispatchOutcome::Order {
                order_id: order.order_id.to_string(),
                client_order_id: Some(order.client_order_id.to_string()),
                market_index: order.market_index,
                status: order.status,
                venue_status: order.status.as_lighter_status().to_string(),
            }
        );
    }
}

#[test]
fn preserves_cancel_rejected_as_distinct_order_outcome() {
    let fixtures = execution_fixture_set();
    let order = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::CancelRejected)
        .expect("cancel rejected fixture");

    let outcome = dispatch_private_message(&order.to_ws_message());

    assert_eq!(
        outcome,
        DispatchOutcome::Order {
            order_id: order.order_id.to_string(),
            client_order_id: Some(order.client_order_id.to_string()),
            market_index: order.market_index,
            status: OrderFixtureStatus::CancelRejected,
            venue_status: "cancel_rejected".to_string(),
        }
    );
}

#[test]
fn dispatches_account_update_fixture_to_account_outcome() {
    let fixtures = execution_fixture_set();
    let message = InboundMessage::AccountUpdate {
        address: fixtures.account.address.clone(),
        balances: vec![(fixtures.account.asset.clone(), fixtures.account.balance.clone())],
        timestamp: fixtures.account.timestamp_ms,
    };

    let outcome = dispatch_private_message(&message);

    assert_eq!(
        outcome,
        DispatchOutcome::Account {
            address: fixtures.account.address,
            balances: vec![(fixtures.account.asset, fixtures.account.balance)],
            timestamp_ms: fixtures.account.timestamp_ms,
        }
    );
}

#[test]
fn ignores_non_private_execution_messages() {
    let outcome = dispatch_private_message(&InboundMessage::Pong);

    assert_eq!(outcome, DispatchOutcome::Ignored);
}
