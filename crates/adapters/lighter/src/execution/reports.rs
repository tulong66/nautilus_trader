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

use nautilus_core::UnixNanos;
use nautilus_model::{
    enums::{LiquiditySide, OrderSide, OrderStatus, OrderType, PositionSideSpecified, TimeInForce},
    identifiers::{AccountId, ClientId, ClientOrderId, InstrumentId, TradeId, Venue, VenueOrderId},
    reports::{ExecutionMassStatus, FillReport, OrderStatusReport, PositionStatusReport},
    types::{Currency, Money, Price, Quantity},
};
use rust_decimal::Decimal;

use crate::{
    error::LighterError,
    execution::fixtures::{
        ExecutionFixtureSet, FillFixture, OrderFixtureStatus, OrderUpdateFixture, PositionFixture,
    },
};

pub fn build_order_status_report(
    order: &OrderUpdateFixture,
    account_id: AccountId,
    instrument_id: InstrumentId,
) -> Result<OrderStatusReport, LighterError> {
    let ts = timestamp_ms_to_ns(order.timestamp_ms)?;
    let report = OrderStatusReport::new(
        account_id,
        instrument_id,
        Some(ClientOrderId::new(order.client_order_id)),
        VenueOrderId::new(order.order_id),
        order_side_from_fixture(order.side)?,
        OrderType::Limit,
        TimeInForce::Gtc,
        order_status_from_fixture(order.status),
        Quantity::from(order.quantity),
        Quantity::from(order.filled_quantity),
        ts,
        ts,
        ts,
        None,
    )
    .with_price(Price::from(order.price));

    Ok(report)
}

pub fn build_fill_report(
    fill: &FillFixture,
    account_id: AccountId,
    instrument_id: InstrumentId,
) -> Result<FillReport, LighterError> {
    let ts = timestamp_ms_to_ns(fill.timestamp_ms)?;
    Ok(FillReport::new(
        account_id,
        instrument_id,
        VenueOrderId::new(&fill.order_id),
        TradeId::new(&fill.trade_id),
        order_side_from_fixture(&fill.side)?,
        Quantity::from(fill.quantity.as_str()),
        Price::from(fill.price.as_str()),
        Money::new(0.0, Currency::USD()),
        LiquiditySide::NoLiquiditySide,
        Some(ClientOrderId::new(&fill.client_order_id)),
        None,
        ts,
        ts,
        None,
    ))
}

pub fn build_position_status_report(
    position: &PositionFixture,
    account_id: AccountId,
    instrument_id: InstrumentId,
) -> Result<PositionStatusReport, LighterError> {
    let ts = timestamp_ms_to_ns(position.timestamp_ms)?;
    let quantity = Quantity::from(position.size.as_str());
    let side = if quantity.is_zero() {
        PositionSideSpecified::Flat
    } else {
        PositionSideSpecified::Long
    };
    let avg_px_open = position
        .entry_price
        .parse::<Decimal>()
        .map_err(|e| LighterError::Parse(format!("Invalid fixture entry price: {e}")))?;

    Ok(PositionStatusReport::new(
        account_id,
        instrument_id,
        side,
        quantity,
        ts,
        ts,
        None,
        None,
        Some(avg_px_open),
    ))
}

pub fn build_mass_status(
    fixtures: &ExecutionFixtureSet,
    client_id: ClientId,
    account_id: AccountId,
    venue: Venue,
    instrument_id: InstrumentId,
) -> Result<ExecutionMassStatus, LighterError> {
    let mut mass_status = ExecutionMassStatus::new(
        client_id,
        account_id,
        venue,
        UnixNanos::default(),
        None,
    );

    let order_reports = fixtures
        .orders
        .iter()
        .map(|order| build_order_status_report(order, account_id, instrument_id))
        .collect::<Result<Vec<_>, _>>()?;
    let fill_report = build_fill_report(&fixtures.fill, account_id, instrument_id)?;
    let position_report = build_position_status_report(&fixtures.position, account_id, instrument_id)?;

    mass_status.add_order_reports(order_reports);
    mass_status.add_fill_reports(vec![fill_report]);
    mass_status.add_position_reports(vec![position_report]);

    Ok(mass_status)
}

fn timestamp_ms_to_ns(timestamp_ms: i64) -> Result<UnixNanos, LighterError> {
    u64::try_from(timestamp_ms)
        .map_err(|_| LighterError::Parse(format!("Invalid fixture timestamp: {timestamp_ms}")))?
        .checked_mul(1_000_000)
        .map(UnixNanos::from)
        .ok_or_else(|| LighterError::Parse(format!("Fixture timestamp overflow: {timestamp_ms}")))
}

fn order_status_from_fixture(status: OrderFixtureStatus) -> OrderStatus {
    match status {
        OrderFixtureStatus::Accepted | OrderFixtureStatus::CancelRejected => OrderStatus::Accepted,
        OrderFixtureStatus::Rejected => OrderStatus::Rejected,
        OrderFixtureStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
        OrderFixtureStatus::Filled => OrderStatus::Filled,
        OrderFixtureStatus::Canceled => OrderStatus::Canceled,
    }
}

fn order_side_from_fixture(side: &str) -> Result<OrderSide, LighterError> {
    match side {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        _ => Err(LighterError::Parse(format!("Unsupported fixture side: {side}"))),
    }
}
