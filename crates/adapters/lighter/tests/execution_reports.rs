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
    common::LIGHTER_VENUE,
    execution::{
        fixtures::{OrderFixtureStatus, execution_fixture_set},
        reports::{
            build_fill_report, build_mass_status, build_order_status_report,
            build_position_status_report,
        },
    },
};
use nautilus_model::{
    enums::{LiquiditySide, OrderSide, OrderStatus, PositionSideSpecified},
    identifiers::{AccountId, ClientId, InstrumentId},
};

fn fixture_account_id() -> AccountId {
    AccountId::new("FIXTURE-001")
}

fn fixture_client_id() -> ClientId {
    ClientId::new("LIGHTER")
}

fn fixture_instrument_id() -> InstrumentId {
    InstrumentId::from("ETH_USDC.LIGHTER")
}

#[test]
fn builds_order_status_report_from_filled_order_fixture() {
    let fixtures = execution_fixture_set();
    let order = fixtures
        .orders
        .iter()
        .find(|order| order.status == OrderFixtureStatus::Filled)
        .expect("filled order fixture");

    let report = build_order_status_report(order, fixture_account_id(), fixture_instrument_id())
        .expect("order status report");

    assert_eq!(report.account_id, fixture_account_id());
    assert_eq!(report.instrument_id, fixture_instrument_id());
    assert_eq!(report.client_order_id.unwrap().as_str(), order.client_order_id);
    assert_eq!(report.venue_order_id.as_str(), order.order_id);
    assert_eq!(report.order_side, OrderSide::Buy);
    assert_eq!(report.order_status, OrderStatus::Filled);
    assert_eq!(report.quantity.to_string(), "0.2500");
    assert_eq!(report.filled_qty.to_string(), "0.2500");
}

#[test]
fn maps_order_fixture_statuses_to_nautilus_order_statuses() {
    let fixtures = execution_fixture_set();

    let statuses = fixtures
        .orders
        .iter()
        .map(|order| {
            let report = build_order_status_report(order, fixture_account_id(), fixture_instrument_id())
                .expect("order status report");
            (order.status, report.order_status)
        })
        .collect::<Vec<_>>();

    assert!(statuses.contains(&(OrderFixtureStatus::Accepted, OrderStatus::Accepted)));
    assert!(statuses.contains(&(OrderFixtureStatus::Rejected, OrderStatus::Rejected)));
    assert!(statuses.contains(&(OrderFixtureStatus::PartiallyFilled, OrderStatus::PartiallyFilled)));
    assert!(statuses.contains(&(OrderFixtureStatus::Filled, OrderStatus::Filled)));
    assert!(statuses.contains(&(OrderFixtureStatus::Canceled, OrderStatus::Canceled)));
    assert!(statuses.contains(&(OrderFixtureStatus::CancelRejected, OrderStatus::Accepted)));
}

#[test]
fn builds_fill_and_position_reports_from_fixtures() {
    let fixtures = execution_fixture_set();

    let fill = build_fill_report(&fixtures.fill, fixture_account_id(), fixture_instrument_id())
        .expect("fill report");
    assert_eq!(fill.account_id, fixture_account_id());
    assert_eq!(fill.instrument_id, fixture_instrument_id());
    assert_eq!(fill.venue_order_id.as_str(), fixtures.fill.order_id);
    assert_eq!(fill.trade_id.as_str(), fixtures.fill.trade_id);
    assert_eq!(fill.client_order_id.unwrap().as_str(), fixtures.fill.client_order_id);
    assert_eq!(fill.order_side, OrderSide::Buy);
    assert_eq!(fill.last_qty.to_string(), "0.2500");
    assert_eq!(fill.last_px.to_string(), "101.25");
    assert_eq!(fill.liquidity_side, LiquiditySide::NoLiquiditySide);

    let position = build_position_status_report(
        &fixtures.position,
        fixture_account_id(),
        fixture_instrument_id(),
    )
    .expect("position status report");
    assert_eq!(position.account_id, fixture_account_id());
    assert_eq!(position.instrument_id, fixture_instrument_id());
    assert_eq!(position.position_side, PositionSideSpecified::Long);
    assert_eq!(position.quantity.to_string(), "0.2500");
    assert_eq!(position.avg_px_open.unwrap().to_string(), "101.25");
}

#[test]
fn builds_mass_status_from_fixture_set() {
    let fixtures = execution_fixture_set();
    let mass_status = build_mass_status(
        &fixtures,
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    )
    .expect("mass status");

    assert_eq!(mass_status.client_id, fixture_client_id());
    assert_eq!(mass_status.account_id, fixture_account_id());
    assert_eq!(mass_status.venue, *LIGHTER_VENUE);
    assert_eq!(mass_status.order_reports().len(), 6);
    assert_eq!(mass_status.fill_reports().len(), 1);
    assert_eq!(mass_status.position_reports().len(), 1);
}
