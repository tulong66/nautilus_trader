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

use std::collections::HashSet;

use nautilus_lighter::{
    execution::fixtures::{
        AccountUpdateFixture, FillFixture, OrderFixtureStatus, PositionFixture,
        execution_fixture_set,
    },
    websocket::messages::InboundMessage,
};

#[test]
fn fixture_set_covers_required_order_outcomes() {
    let fixtures = execution_fixture_set();
    let statuses = fixtures
        .orders
        .iter()
        .map(|order| order.status)
        .collect::<HashSet<_>>();

    assert_eq!(fixtures.orders.len(), 6);
    assert!(statuses.contains(&OrderFixtureStatus::Accepted));
    assert!(statuses.contains(&OrderFixtureStatus::Rejected));
    assert!(statuses.contains(&OrderFixtureStatus::PartiallyFilled));
    assert!(statuses.contains(&OrderFixtureStatus::Filled));
    assert!(statuses.contains(&OrderFixtureStatus::Canceled));
    assert!(statuses.contains(&OrderFixtureStatus::CancelRejected));
}

#[test]
fn order_fixtures_convert_to_private_websocket_messages() {
    let fixtures = execution_fixture_set();
    let accepted = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::Accepted)
        .expect("accepted order fixture");

    let message = accepted.to_ws_message();

    match message {
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
            assert_eq!(order_id, accepted.order_id);
            assert_eq!(client_order_id.as_deref(), Some(accepted.client_order_id));
            assert_eq!(market_index, accepted.market_index);
            assert_eq!(status, "open");
            assert_eq!(side, "buy");
            assert_eq!(order_type, "limit");
            assert_eq!(price, "100.50");
            assert_eq!(quantity, "0.2500");
            assert_eq!(filled_quantity, "0");
            assert_eq!(timestamp, accepted.timestamp_ms);
        }
        other => panic!("expected order update fixture, got {other:?}"),
    };
}

#[test]
fn account_fill_and_position_fixtures_are_present() {
    let fixtures = execution_fixture_set();

    assert_eq!(
        fixtures.account,
        AccountUpdateFixture {
            account_index: 42,
            address: "fixture-account".to_string(),
            asset: "USDC".to_string(),
            balance: "1000.00".to_string(),
            timestamp_ms: 1_700_000_000_100,
        }
    );
    assert_eq!(
        fixtures.fill,
        FillFixture {
            trade_id: "fixture-trade-1".to_string(),
            order_id: "fixture-order-filled".to_string(),
            client_order_id: "fixture-client-filled".to_string(),
            market_index: 1,
            price: "101.25".to_string(),
            quantity: "0.2500".to_string(),
            side: "buy".to_string(),
            timestamp_ms: 1_700_000_000_060,
        }
    );
    assert_eq!(
        fixtures.position,
        PositionFixture {
            market_index: 1,
            symbol: "ETH_USDC".to_string(),
            size: "0.2500".to_string(),
            entry_price: "101.25".to_string(),
            unrealized_pnl: "0.00".to_string(),
            margin: "25.00".to_string(),
            timestamp_ms: 1_700_000_000_120,
        }
    );
}

#[test]
fn fixtures_are_offline_only_and_contain_no_secret_material() {
    let fixtures = execution_fixture_set();

    assert!(fixtures.is_offline_only());
    assert!(fixtures.secret_material().is_empty());
}
