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

//! Data type conversion utilities for the Lighter adapter.
//!
//! Provides functions to convert between Lighter API types and NautilusTrader
//! data types, handling the critical price decimal conversions.

use std::str::FromStr;

use nautilus_core::UnixNanos;
use nautilus_model::{
    data::{
        Bar, BarType, BookOrder, OrderBookDelta, QuoteTick, TradeTick,
        bar::BarSpecification,
    },
    enums::{AggregationSource, AggressorSide, BarAggregation, BookAction, OrderSide, PriceType},
    identifiers::{InstrumentId, Symbol, TradeId},
    instruments::CryptoFuture,
    types::{Currency, Price, Quantity},
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use ustr::Ustr;

use crate::{
    common::{MarketInfo, OrderBookLevel, Trade, consts::LIGHTER_VENUE},
    error::LighterError,
};

/// Default size decimals used by Lighter (8 decimals for base amounts).
pub(crate) const DEFAULT_SIZE_DECIMALS: u8 = 8;

/// Converts a Lighter `MarketInfo` to a NautilusTrader `CryptoFuture` instrument.
///
/// # Arguments
///
/// * `market` - The market information from Lighter API
///
/// # Errors
///
/// Returns an error if:
/// - Instrument creation fails
/// - Price/quantity conversion fails
///
/// # Example
///
/// ```ignore
/// let market = MarketInfo {
///     market_id: 1,
///     symbol: "ETH_USDC".to_string(),
///     base_symbol: "ETH".to_string(),
///     quote_symbol: "USDC".to_string(),
///     price_decimals: 2,
///     size_decimals: 8,
///     min_base_amount: 1000000,
///     tick_size: 1,
///     status: LighterMarketStatus::Active,
/// };
/// let instrument = parse_instrument(&market)?;
/// ```
pub fn parse_instrument(market: &MarketInfo) -> Result<CryptoFuture, LighterError> {
    // Create instrument ID (e.g., "ETH_USDC.LIGHTER")
    let instrument_id = InstrumentId::new(
        Symbol::new(Ustr::from(&market.symbol)),
        *LIGHTER_VENUE,
    );
    let raw_symbol = Symbol::new(Ustr::from(&market.symbol));

    // Create currencies for base and quote
    let underlying = Currency::new(
        Ustr::from(&market.base_symbol),
        market.size_decimals,
        0, // ISO code not applicable for crypto
        Ustr::from(&market.base_symbol),
        nautilus_model::enums::CurrencyType::Crypto,
    );

    // USDC typically has 6 decimals
    let quote_decimals = if market.quote_symbol == "USDC" { 6 } else { market.price_decimals };
    let quote_currency = Currency::new(
        Ustr::from(&market.quote_symbol),
        quote_decimals,
        0,
        Ustr::from(&market.quote_symbol),
        nautilus_model::enums::CurrencyType::Crypto,
    );

    // Settlement is same as quote for Lighter perpetuals
    let settlement_currency = quote_currency;

    // Price and size precision
    let price_precision = market.price_decimals;
    let size_precision = market.size_decimals;

    // Price increment (tick_size is stored as f64 bits)
    let tick_size_f64 = f64::from_bits(market.tick_size);
    let price_increment = Price::new(tick_size_f64, price_precision);

    // Size increment (minimum step is 1 at size_decimals precision)
    let size_increment = Quantity::new(10f64.powi(-(size_precision as i32)), size_precision);

    // Minimum quantity
    let min_base_f64 = f64::from_bits(market.min_base_amount);
    let min_quantity = Some(Quantity::new(min_base_f64, size_precision));

    // Create the instrument
    let ts_now = UnixNanos::default();

    CryptoFuture::new_checked(
        instrument_id,
        raw_symbol,
        underlying,
        quote_currency,
        settlement_currency,
        false, // is_inverse
        UnixNanos::default(), // activation_ns (perpetual)
        UnixNanos::from(u64::MAX), // expiration_ns (perpetual/never)
        price_precision,
        size_precision,
        price_increment,
        size_increment,
        None, // multiplier (default 1)
        None, // lot_size (default 1)
        None, // max_quantity
        min_quantity,
        None, // max_notional
        None, // min_notional
        None, // max_price
        None, // min_price
        None, // margin_init
        None, // margin_maint
        Some(Decimal::new(2, 4)), // maker_fee: 0.02% = 0.0002
        Some(Decimal::new(5, 4)), // taker_fee: 0.05% = 0.0005
        None, // params
        ts_now,
        ts_now,
    )
    .map_err(|e| LighterError::Parse(format!("Failed to create CryptoFuture: {e}")))
}

/// Converts Lighter order book levels to NautilusTrader `OrderBookDelta`s.
///
/// # Arguments
///
/// * `bids` - Bid levels from Lighter
/// * `asks` - Ask levels from Lighter
/// * `instrument_id` - The instrument ID
/// * `price_decimals` - Number of decimals for price (varies per market!)
/// * `sequence` - Sequence number for ordering
///
/// # Errors
///
/// Returns an error if price/quantity conversion fails.
///
/// # Price Conversion
///
/// Lighter stores prices as integers that must be divided by 10^price_decimals:
/// - ETH (2 decimals): 412739 -> $4127.39
/// - BTC (1 decimal): 1143578 -> $114357.8
/// - DOGE (6 decimals): 202095 -> $0.202095
pub fn parse_order_book_deltas(
    bids: &[OrderBookLevel],
    asks: &[OrderBookLevel],
    instrument_id: InstrumentId,
    price_decimals: u8,
    size_decimals: u8,
    sequence: u64,
) -> Result<Vec<OrderBookDelta>, LighterError> {
    let mut deltas = Vec::with_capacity(bids.len() + asks.len());
    let ts_event = UnixNanos::default();
    let ts_init = UnixNanos::default();

    // Process bids
    for level in bids {
        let price = convert_price(level.price, price_decimals)?;
        let size = convert_quantity(level.size, size_decimals)?;

        // Determine action based on size (0 = delete, else add/update)
        let action = if level.size == 0 {
            BookAction::Delete
        } else {
            BookAction::Add
        };

        let order = BookOrder::new(
            OrderSide::Buy,
            price,
            size,
            0, // order_id - not tracked at level
        );

        deltas.push(OrderBookDelta::new(
            instrument_id,
            action,
            order,
            0, // flags
            sequence,
            ts_event,
            ts_init,
        ));
    }

    // Process asks
    for level in asks {
        let price = convert_price(level.price, price_decimals)?;
        let size = convert_quantity(level.size, size_decimals)?;

        let action = if level.size == 0 {
            BookAction::Delete
        } else {
            BookAction::Add
        };

        let order = BookOrder::new(
            OrderSide::Sell,
            price,
            size,
            0,
        );

        deltas.push(OrderBookDelta::new(
            instrument_id,
            action,
            order,
            0,
            sequence,
            ts_event,
            ts_init,
        ));
    }

    Ok(deltas)
}

/// Converts a Lighter `Trade` to a NautilusTrader `TradeTick`.
///
/// # Arguments
///
/// * `trade` - The trade record from Lighter API
/// * `instrument_id` - The instrument ID
/// * `price_decimals` - Number of decimals for price conversion
///
/// # Errors
///
/// Returns an error if:
/// - Price/quantity conversion fails
/// - Timestamp conversion fails
///
/// # Aggressor Side
///
/// Lighter provides `is_taker_ask`:
/// - `true` means taker was selling (aggressor = SELL)
/// - `false` means taker was buying (aggressor = BUY)
pub fn parse_trade_tick(
    trade: &Trade,
    instrument_id: InstrumentId,
    price_decimals: u8,
    size_decimals: u8,
) -> Result<TradeTick, LighterError> {
    let price = convert_price(trade.price, price_decimals)?;
    let size = convert_quantity(trade.size, size_decimals)?;

    // Map is_taker_ask to AggressorSide
    // true = taker was selling = aggressor SELLER
    // false = taker was buying = aggressor BUYER
    let aggressor_side = if trade.is_taker_ask {
        AggressorSide::Seller
    } else {
        AggressorSide::Buyer
    };

    // Convert timestamp from milliseconds to nanoseconds
    let ts_event = UnixNanos::from(trade.timestamp_ms as u64 * 1_000_000);
    let ts_init = ts_event;

    // Create trade ID from trade index
    let trade_id = TradeId::new(Ustr::from(&trade.id.to_string()));

    Ok(TradeTick::new(
        instrument_id,
        price,
        size,
        aggressor_side,
        trade_id,
        ts_event,
        ts_init,
    ))
}

/// Converts a Lighter ticker update to a NautilusTrader `QuoteTick`.
///
/// # Arguments
///
/// * `bid_price` - Best bid price (raw integer)
/// * `bid_size` - Best bid size (raw integer)
/// * `ask_price` - Best ask price (raw integer)
/// * `ask_size` - Best ask size (raw integer)
/// * `instrument_id` - The instrument ID
/// * `price_decimals` - Number of decimals for price conversion
/// * `size_decimals` - Number of decimals for quote size conversion
/// * `timestamp_ns` - Timestamp in nanoseconds
///
/// # Errors
///
/// Returns an error if price/quantity conversion fails.
pub fn parse_quote_tick(
    bid_price: u64,
    bid_size: u64,
    ask_price: u64,
    ask_size: u64,
    instrument_id: InstrumentId,
    price_decimals: u8,
    size_decimals: u8,
    timestamp_ns: u64,
) -> Result<QuoteTick, LighterError> {
    let bid = convert_price(bid_price, price_decimals)?;
    let ask = convert_price(ask_price, price_decimals)?;
    let bid_qty = convert_quantity(bid_size, size_decimals)?;
    let ask_qty = convert_quantity(ask_size, size_decimals)?;

    let ts_event = UnixNanos::from(timestamp_ns);
    let ts_init = ts_event;

    Ok(QuoteTick::new(
        instrument_id,
        bid,
        ask,
        bid_qty,
        ask_qty,
        ts_event,
        ts_init,
    ))
}

pub fn decimal_places(value: &str, field_name: &str) -> Result<u8, LighterError> {
    let decimal = Decimal::from_str(value).map_err(|e| {
        LighterError::Parse(format!("Failed to parse {field_name} '{value}': {e}"))
    })?;

    if decimal <= Decimal::ZERO {
        return Err(LighterError::Parse(format!(
            "{field_name} '{value}' must be positive"
        )));
    }

    u8::try_from(decimal.normalize().scale()).map_err(|e| {
        LighterError::Parse(format!("Failed to derive decimal places for {field_name} '{value}': {e}"))
    })
}

pub fn parse_decimal_to_raw(value: &str, decimals: u8, field_name: &str) -> Result<u64, LighterError> {
    let decimal = Decimal::from_str(value).map_err(|e| {
        LighterError::Parse(format!("Failed to parse {field_name} '{value}': {e}"))
    })?;
    let scale = Decimal::from(10u64.pow(decimals as u32));
    let raw = decimal * scale;

    if raw.fract() != Decimal::ZERO {
        return Err(LighterError::Parse(format!(
            "{field_name} '{value}' exceeds {decimals} decimal places"
        )));
    }

    raw.to_u64().ok_or_else(|| {
        LighterError::Parse(format!("Failed to convert {field_name} '{value}' to raw integer"))
    })
}

/// Converts a Lighter bar/candle to a NautilusTrader `Bar`.
///
/// # Arguments
///
/// * `open` - Opening price (raw)
/// * `high` - High price (raw)
/// * `low` - Low price (raw)
/// * `close` - Close price (raw)
/// * `volume` - Volume (raw)
/// * `bar_type` - The bar type for this bar
/// * `price_decimals` - Number of decimals for price conversion
/// * `size_decimals` - Number of decimals for volume conversion
/// * `timestamp_ns` - Bar timestamp in nanoseconds
///
/// # Errors
///
/// Returns an error if price/volume conversion fails.
pub fn parse_bar(
    open: u64,
    high: u64,
    low: u64,
    close: u64,
    volume: u64,
    bar_type: BarType,
    price_decimals: u8,
    size_decimals: u8,
    timestamp_ns: u64,
) -> Result<Bar, LighterError> {
    let open_price = convert_price(open, price_decimals)?;
    let high_price = convert_price(high, price_decimals)?;
    let low_price = convert_price(low, price_decimals)?;
    let close_price = convert_price(close, price_decimals)?;
    let vol = convert_quantity(volume, size_decimals)?;

    let ts_event = UnixNanos::from(timestamp_ns);
    let ts_init = ts_event;

    Ok(Bar::new(
        bar_type,
        open_price,
        high_price,
        low_price,
        close_price,
        vol,
        ts_event,
        ts_init,
    ))
}

/// Creates a BarType for Lighter candle data.
///
/// # Arguments
///
/// * `instrument_id` - The instrument ID
/// * `interval` - Interval string ("1m", "1h", "1d")
///
/// # Errors
///
/// Returns an error if interval is not supported.
pub fn create_bar_type(instrument_id: InstrumentId, interval: &str) -> Result<BarType, LighterError> {
    let (step, aggregation) = match interval {
        "1m" => (1, BarAggregation::Minute),
        "5m" => (5, BarAggregation::Minute),
        "15m" => (15, BarAggregation::Minute),
        "1h" => (1, BarAggregation::Hour),
        "4h" => (4, BarAggregation::Hour),
        "1d" => (1, BarAggregation::Day),
        _ => return Err(LighterError::Parse(format!("Unsupported interval: {interval}"))),
    };

    let spec = BarSpecification::new(
        step,
        aggregation,
        PriceType::Last,
    );
    Ok(BarType::new(instrument_id, spec, AggregationSource::External))
}

/// Converts a raw Lighter price to a NautilusTrader `Price`.
///
/// # Arguments
///
/// * `raw_price` - Raw price as stored by Lighter
/// * `price_decimals` - Number of decimals for this market
///
/// # Example
///
/// ```ignore
/// // ETH with 2 decimals
/// let price = convert_price(412739, 2)?; // $4127.39
///
/// // BTC with 1 decimal
/// let price = convert_price(1143578, 1)?; // $114357.8
///
/// // DOGE with 6 decimals
/// let price = convert_price(202095, 6)?; // $0.202095
/// ```
pub fn convert_price(raw_price: u64, price_decimals: u8) -> Result<Price, LighterError> {
    // Formula: actual_price = raw_price / 10^price_decimals
    // Use Price::new with the actual floating point value
    let divisor = 10f64.powi(price_decimals as i32);
    let actual_price = raw_price as f64 / divisor;
    Ok(Price::new(actual_price, price_decimals))
}

/// Converts a raw Lighter size to a NautilusTrader `Quantity`.
///
/// # Arguments
///
/// * `raw_size` - Raw size as stored by Lighter
/// * `size_decimals` - Number of decimals (typically 8)
pub fn convert_quantity(raw_size: u64, size_decimals: u8) -> Result<Quantity, LighterError> {
    // Formula: actual_quantity = raw_size / 10^size_decimals
    // Use Quantity::new with the actual floating point value
    let divisor = 10f64.powi(size_decimals as i32);
    let actual_quantity = raw_size as f64 / divisor;
    Ok(Quantity::new(actual_quantity, size_decimals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::LighterMarketStatus;
    use std::num::NonZeroUsize;

    #[test]
    fn test_convert_price_eth() {
        // ETH with 2 decimals: 412739 -> $4127.39
        let price = convert_price(412739, 2).unwrap();
        assert_eq!(price.as_f64(), 4127.39);
    }

    #[test]
    fn test_convert_price_btc() {
        // BTC with 1 decimal: 1143578 -> $114357.8
        let price = convert_price(1143578, 1).unwrap();
        assert_eq!(price.as_f64(), 114357.8);
    }

    #[test]
    fn test_convert_price_doge() {
        // DOGE with 6 decimals: 202095 -> $0.202095
        let price = convert_price(202095, 6).unwrap();
        assert_eq!(price.as_f64(), 0.202095);
    }

    #[test]
    fn test_convert_quantity() {
        // 1.5 ETH with 8 decimals: 150000000 -> 1.5
        let qty = convert_quantity(150_000_000, 8).unwrap();
        assert_eq!(qty.as_f64(), 1.5);
    }

    #[test]
    fn test_decimal_places() {
        assert_eq!(decimal_places("0.01", "tick_size").unwrap(), 2);
        assert_eq!(decimal_places("0.0001", "step_size").unwrap(), 4);
        assert_eq!(decimal_places("1.0000", "step_size").unwrap(), 0);
    }

    #[test]
    fn test_parse_decimal_to_raw_price() {
        let raw = parse_decimal_to_raw("4127.39", 2, "price").unwrap();
        assert_eq!(raw, 412_739);
    }

    #[test]
    fn test_parse_decimal_to_raw_volume() {
        let raw = parse_decimal_to_raw("1234.56", 8, "volume").unwrap();
        assert_eq!(raw, 123_456_000_000);
    }

    #[test]
    fn test_parse_decimal_to_raw_rejects_extra_precision() {
        let result = parse_decimal_to_raw("4127.391", 2, "price");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_bar_type_1m() {
        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let bar_type = create_bar_type(instrument_id, "1m").unwrap();
        assert_eq!(bar_type.spec().step, NonZeroUsize::new(1).unwrap());
        assert_eq!(bar_type.spec().aggregation, BarAggregation::Minute);
    }

    #[test]
    fn test_create_bar_type_1h() {
        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let bar_type = create_bar_type(instrument_id, "1h").unwrap();
        assert_eq!(bar_type.spec().step, NonZeroUsize::new(1).unwrap());
        assert_eq!(bar_type.spec().aggregation, BarAggregation::Hour);
    }

    #[test]
    fn test_create_bar_type_invalid() {
        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let result = create_bar_type(instrument_id, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_trade_tick() {
        let trade = Trade {
            id: 12345,
            market_id: 1,
            price: 412739,
            size: 100_000_000, // 1.0 with 8 decimals
            is_taker_ask: true,
            timestamp_ms: 1700000000000,
        };

        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let tick = parse_trade_tick(&trade, instrument_id, 2, 8).unwrap();

        assert_eq!(tick.price.as_f64(), 4127.39);
        assert_eq!(tick.size.as_f64(), 1.0);
        assert_eq!(tick.aggressor_side, AggressorSide::Seller);
    }

    #[test]
    fn test_parse_quote_tick() {
        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let tick = parse_quote_tick(
            412700, // bid price
            100_000_000, // bid size (1.0)
            412800, // ask price
            200_000_000, // ask size (2.0)
            instrument_id,
            2, // price_decimals
            8, // size_decimals
            1700000000000000000, // timestamp_ns
        ).unwrap();

        assert_eq!(tick.bid_price.as_f64(), 4127.00);
        assert_eq!(tick.ask_price.as_f64(), 4128.00);
        assert_eq!(tick.bid_size.as_f64(), 1.0);
        assert_eq!(tick.ask_size.as_f64(), 2.0);
    }

    #[test]
    fn test_parse_order_book_deltas() {
        let bids = vec![
            OrderBookLevel { price: 412700, size: 100_000_000 },
            OrderBookLevel { price: 412600, size: 200_000_000 },
        ];
        let asks = vec![
            OrderBookLevel { price: 412800, size: 150_000_000 },
        ];

        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let deltas = parse_order_book_deltas(&bids, &asks, instrument_id, 2, 8, 1).unwrap();

        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].order.side, OrderSide::Buy);
        assert_eq!(deltas[0].order.price.as_f64(), 4127.00);
        assert_eq!(deltas[2].order.side, OrderSide::Sell);
        assert_eq!(deltas[2].order.price.as_f64(), 4128.00);
    }

    #[test]
    fn test_parse_instrument() {
        let market = MarketInfo {
            market_id: 1,
            symbol: "ETH_USDC".to_string(),
            base_symbol: "ETH".to_string(),
            quote_symbol: "USDC".to_string(),
            price_decimals: 2,
            size_decimals: 8,
            min_base_amount: 0.001f64.to_bits(), // 0.001 ETH minimum
            tick_size: 0.01f64.to_bits(), // $0.01 tick
            status: LighterMarketStatus::Active,
        };

        let instrument = parse_instrument(&market).unwrap();
        assert_eq!(instrument.id.symbol.as_str(), "ETH_USDC");
        assert_eq!(instrument.price_precision, 2);
        assert_eq!(instrument.size_precision, 8);
    }
}
