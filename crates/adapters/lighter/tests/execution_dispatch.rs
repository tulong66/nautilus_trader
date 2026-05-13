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
        dispatch::{DispatchOutcome, OrderDispatchStatus, dispatch_private_message},
        fixtures::{OrderFixtureStatus, execution_fixture_set},
        reconciliation::{ExecutionReconciler, ReconciledOrderStatus, ReconciliationAction},
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
                status: OrderDispatchStatus::from(order.status),
                venue_status: order.status.as_lighter_status().to_string(),
                timestamp_ms: order.timestamp_ms,
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
            status: OrderDispatchStatus::CancelRejected,
            venue_status: "cancel_rejected".to_string(),
            timestamp_ms: order.timestamp_ms,
        }
    );
}

#[test]
fn dispatches_account_update_fixture_to_account_outcome() {
    let fixtures = execution_fixture_set();
    let message = InboundMessage::AccountUpdate {
        address: fixtures.account.address.clone(),
        balances: vec![(
            fixtures.account.asset.clone(),
            fixtures.account.balance.clone(),
        )],
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

#[test]
fn reconciles_send_order_update_fill_cancel_and_cancel_reject_replay() {
    let fixtures = execution_fixture_set();
    let mut reconciler = ExecutionReconciler::default();

    let accepted = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::Accepted)
        .expect("accepted fixture");
    let partial = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::PartiallyFilled)
        .expect("partial fixture");
    let filled = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::Filled)
        .expect("filled fixture");
    let canceled = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::Canceled)
        .expect("canceled fixture");
    let cancel_rejected = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::CancelRejected)
        .expect("cancel rejected fixture");

    assert_eq!(
        reconciler.record_send(
            accepted.client_order_id,
            accepted.market_index,
            accepted.timestamp_ms - 1,
        ),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&accepted.to_ws_message())),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.order_status(accepted.client_order_id),
        Some(ReconciledOrderStatus::Accepted)
    );

    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&partial.to_ws_message())),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.order_status(partial.client_order_id),
        Some(ReconciledOrderStatus::PartiallyFilled)
    );
    assert_eq!(
        reconciler.record_fill(
            &fixtures.fill.trade_id,
            filled.order_id,
            Some(filled.client_order_id),
            filled.market_index,
            fixtures.fill.timestamp_ms,
        ),
        ReconciliationAction::Accepted
    );
    let fill_message = InboundMessage::OrderUpdate {
        order_id: filled.order_id.to_string(),
        client_order_id: Some(filled.client_order_id.to_string()),
        market_index: filled.market_index,
        status: filled.status.as_lighter_status().to_string(),
        side: filled.side.to_string(),
        order_type: filled.order_type.to_string(),
        price: filled.price.to_string(),
        quantity: filled.quantity.to_string(),
        filled_quantity: filled.filled_quantity.to_string(),
        timestamp: fixtures.fill.timestamp_ms + 1,
    };
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&fill_message)),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.order_status(filled.client_order_id),
        Some(ReconciledOrderStatus::Filled)
    );

    assert_eq!(
        reconciler.record_cancel_request(
            canceled.client_order_id,
            canceled.market_index,
            canceled.timestamp_ms - 1,
        ),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&canceled.to_ws_message())),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.order_status(canceled.client_order_id),
        Some(ReconciledOrderStatus::Canceled)
    );

    assert_eq!(
        reconciler.record_cancel_request(
            cancel_rejected.client_order_id,
            cancel_rejected.market_index,
            cancel_rejected.timestamp_ms - 1,
        ),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&cancel_rejected.to_ws_message())),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.order_status(cancel_rejected.client_order_id),
        Some(ReconciledOrderStatus::CancelRejected)
    );

    let account_message = InboundMessage::AccountUpdate {
        address: fixtures.account.address.clone(),
        balances: vec![(
            fixtures.account.asset.clone(),
            fixtures.account.balance.clone(),
        )],
        timestamp: fixtures.account.timestamp_ms,
    };
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&account_message)),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.account_timestamp(&fixtures.account.address),
        Some(fixtures.account.timestamp_ms)
    );
}

#[test]
fn reconciliation_deduplicates_duplicate_messages_and_rejects_stale_order_updates() {
    let fixtures = execution_fixture_set();
    let mut reconciler = ExecutionReconciler::default();
    let partial = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::PartiallyFilled)
        .expect("partial fixture");
    let filled = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::Filled)
        .expect("filled fixture");

    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&filled.to_ws_message())),
        ReconciliationAction::Accepted
    );
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&filled.to_ws_message())),
        ReconciliationAction::Duplicate
    );
    let stale_partial = InboundMessage::OrderUpdate {
        order_id: filled.order_id.to_string(),
        client_order_id: Some(filled.client_order_id.to_string()),
        market_index: filled.market_index,
        status: partial.status.as_lighter_status().to_string(),
        side: filled.side.to_string(),
        order_type: filled.order_type.to_string(),
        price: filled.price.to_string(),
        quantity: filled.quantity.to_string(),
        filled_quantity: partial.filled_quantity.to_string(),
        timestamp: filled.timestamp_ms - 1,
    };
    assert_eq!(
        reconciler.apply_dispatch(&dispatch_private_message(&stale_partial)),
        ReconciliationAction::Stale
    );
    assert_eq!(
        reconciler.order_status(filled.client_order_id),
        Some(ReconciledOrderStatus::Filled)
    );
    assert_eq!(reconciler.order_count(), 1);
}
