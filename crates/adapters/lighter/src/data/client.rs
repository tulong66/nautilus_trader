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

//! Data client implementation for the Lighter DEX adapter.

use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

use ahash::AHashMap;
use anyhow::Context;
use chrono::{DateTime, Utc};
use nautilus_common::{
    clients::DataClient,
    live::runner::get_data_event_sender,
    messages::DataEvent,
};
use nautilus_core::time::{AtomicTime, get_atomic_clock_realtime};
use nautilus_model::{
    data::{Bar, BarType},
    identifiers::{ClientId, InstrumentId, Venue},
    instruments::InstrumentAny,
};
use serde::Deserialize;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ustr::Ustr;

use crate::{
    common::{MarketInfo, consts::LIGHTER_VENUE},
    config::LighterDataClientConfig,
    data::types::DEFAULT_SIZE_DECIMALS,
    error::LighterError,
    http::LighterRawHttpClient,
    websocket::LighterWebSocketClient,
};

/// Subscription tracking state.
///
/// These fields store subscription metadata for potential future use in:
/// - Resubscription after reconnect (using market_index)
/// - Order book depth filtering (using depth)
/// - Subscription state queries
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for subscription tracking, will be read in reconnect logic
enum SubscriptionState {
    /// Order book subscription with market index.
    OrderBook {
        market_index: u16,
        depth: Option<usize>,
    },
    /// Trade subscription with market index.
    Trades { market_index: u16 },
    /// Ticker subscription with market index.
    Ticker { market_index: u16 },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandleStick {
    timestamp: i64,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

fn bar_type_to_lighter_interval(bar_type: BarType) -> Result<&'static str, LighterError> {
    use nautilus_model::enums::BarAggregation;

    let spec = bar_type.spec();
    match (spec.step.get(), spec.aggregation) {
        (1, BarAggregation::Minute) => Ok("1m"),
        (5, BarAggregation::Minute) => Ok("5m"),
        (15, BarAggregation::Minute) => Ok("15m"),
        (1, BarAggregation::Hour) => Ok("1h"),
        (4, BarAggregation::Hour) => Ok("4h"),
        (1, BarAggregation::Day) => Ok("1d"),
        _ => Err(LighterError::Internal(format!(
            "Unsupported bar specification: {:?}",
            spec
        ))),
    }
}

fn parse_candlestick_bar(
    candle: &CandleStick,
    bar_type: BarType,
    price_decimals: u8,
    size_decimals: u8,
) -> Result<Bar, LighterError> {
    use crate::data::types::{parse_bar, parse_decimal_to_raw};

    let open = parse_decimal_to_raw(&candle.open, price_decimals, "open")?;
    let high = parse_decimal_to_raw(&candle.high, price_decimals, "high")?;
    let low = parse_decimal_to_raw(&candle.low, price_decimals, "low")?;
    let close = parse_decimal_to_raw(&candle.close, price_decimals, "close")?;
    let volume = parse_decimal_to_raw(&candle.volume, size_decimals, "volume")?;
    let timestamp_ns = u64::try_from(candle.timestamp)
        .map_err(|_| LighterError::Parse(format!("Invalid candle timestamp {}", candle.timestamp)))?
        .checked_mul(1_000_000)
        .ok_or_else(|| LighterError::Parse(format!("Candle timestamp overflow: {}", candle.timestamp)))?;

    parse_bar(open, high, low, close, volume, bar_type, price_decimals, size_decimals, timestamp_ns)
}

#[derive(Clone, Copy, Debug)]
struct MarketPrecision {
    price_decimals: u8,
    size_decimals: u8,
}

/// Lighter DEX data client.
///
/// Provides market data access via HTTP and WebSocket:
/// - HTTP: Historical data (bars, trades, markets)
/// - WebSocket: Real-time data (order books, trades, tickers)
///
/// # Architecture
///
/// The client maintains:
/// - HTTP client for historical data requests
/// - WebSocket client for real-time data streaming
/// - Instrument cache for quick lookups
/// - Subscription state tracking
/// - Message routing to NautilusTrader data engine
#[derive(Debug)]
pub struct LighterDataClient {
    /// Client identifier.
    client_id: ClientId,
    /// Configuration (stored for potential reconnection and runtime config access).
    #[allow(dead_code)] // Will be used for runtime configuration access
    config: LighterDataClientConfig,
    /// HTTP client for historical data.
    http_client: LighterRawHttpClient,
    /// WebSocket client for real-time data.
    ws_client: LighterWebSocketClient,
    /// Connection state.
    is_connected: AtomicBool,
    /// Cancellation token for graceful shutdown.
    cancellation_token: CancellationToken,
    /// Background task handles.
    tasks: Vec<JoinHandle<()>>,
    /// Data event sender to NautilusTrader.
    data_sender: tokio::sync::mpsc::UnboundedSender<DataEvent>,
    /// Cached instruments (InstrumentId -> Instrument).
    instruments: Arc<RwLock<AHashMap<InstrumentId, InstrumentAny>>>,
    /// Symbol to InstrumentId mapping (e.g., "ETH_USDC" -> InstrumentId).
    symbol_to_instrument_id: Arc<RwLock<AHashMap<Ustr, InstrumentId>>>,
    /// InstrumentId to market_index mapping
    instrument_to_market_index: Arc<RwLock<AHashMap<InstrumentId, u16>>>,
    /// market_index to InstrumentId reverse mapping (for WebSocket message routing)
    market_index_to_instrument: Arc<RwLock<AHashMap<u16, InstrumentId>>>,
    /// market_index to price/size precision mapping.
    market_precisions: Arc<RwLock<AHashMap<u16, MarketPrecision>>>,
    /// Active subscriptions (InstrumentId -> SubscriptionState).
    subscriptions: Arc<RwLock<AHashMap<InstrumentId, SubscriptionState>>>,
    /// Real-time clock for timestamps (used in data event generation).
    #[allow(dead_code)] // Will be used for timestamp generation in message handling
    clock: &'static AtomicTime,
}

impl LighterDataClient {
    /// Creates a new [`LighterDataClient`] instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - HTTP client initialization fails
    /// - WebSocket client initialization fails
    pub fn new(client_id: ClientId, config: LighterDataClientConfig) -> anyhow::Result<Self> {
        let clock = get_atomic_clock_realtime();
        let data_sender = get_data_event_sender();

        // Create HTTP client
        let public_config = config.to_public_config();
        let environment = public_config.environment;
        let timeout = Some(public_config.http_timeout_secs);
        let http_client = LighterRawHttpClient::new(environment, None, timeout, None, None)
            .context("Failed to create HTTP client")?;

        // Create WebSocket client (public, no auth required for market data)
        let environment = public_config.environment;
        let ws_url = public_config.ws_base_url.clone();
        let heartbeat = Some(public_config.ws_ping_interval_secs);
        let ws_client = LighterWebSocketClient::new_public(environment, ws_url, heartbeat);

        Ok(Self {
            client_id,
            config,
            http_client,
            ws_client,
            is_connected: AtomicBool::new(false),
            cancellation_token: CancellationToken::new(),
            tasks: Vec::new(),
            data_sender,
            instruments: Arc::new(RwLock::new(AHashMap::new())),
            symbol_to_instrument_id: Arc::new(RwLock::new(AHashMap::new())),
            instrument_to_market_index: Arc::new(RwLock::new(AHashMap::new())),
            market_index_to_instrument: Arc::new(RwLock::new(AHashMap::new())),
            market_precisions: Arc::new(RwLock::new(AHashMap::new())),
            subscriptions: Arc::new(RwLock::new(AHashMap::new())),
            clock,
        })
    }

    /// Connects to the Lighter exchange.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - WebSocket connection fails
    /// - Initial market data fetch fails
    pub async fn connect(&mut self) -> Result<(), LighterError> {
        if self.is_connected.load(Ordering::Relaxed) {
            return Ok(());
        }

        tracing::info!("Connecting to Lighter exchange");

        // Connect WebSocket client
        let msg_rx = self.ws_client.connect().await?;

        // Start message handler task
        self.start_ws_message_handler(msg_rx);

        // Fetch initial market data
        self.fetch_markets().await?;

        self.is_connected.store(true, Ordering::Relaxed);
        tracing::info!("Successfully connected to Lighter exchange");

        Ok(())
    }

    /// Disconnects from the Lighter exchange.
    ///
    /// # Errors
    ///
    /// Returns an error if WebSocket disconnection fails.
    pub async fn disconnect(&mut self) -> Result<(), LighterError> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return Ok(());
        }

        tracing::info!("Disconnecting from Lighter exchange");

        // Cancel all background tasks
        self.cancellation_token.cancel();

        // Disconnect WebSocket
        self.ws_client.close().await?;

        // Wait for tasks to complete
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }

        // Clear subscriptions
        self.subscriptions.write().unwrap().clear();

        self.is_connected.store(false, Ordering::Relaxed);
        tracing::info!("Successfully disconnected from Lighter exchange");

        Ok(())
    }

    /// Subscribes to order book updates for the specified instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - The instrument to subscribe to
    /// * `depth` - Optional order book depth (if None, full book)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client is not connected
    /// - WebSocket subscription fails
    pub async fn subscribe_order_book(
        &mut self,
        instrument_id: InstrumentId,
        depth: Option<usize>,
    ) -> Result<(), LighterError> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return Err(LighterError::Internal(
                "Client not connected".to_string(),
            ));
        }

        tracing::info!(
            "Subscribing to order book for {} (depth: {:?})",
            instrument_id,
            depth
        );

        // Get market index for instrument
        let market_index = self.get_market_index_for_instrument(&instrument_id)?;

        // Subscribe via WebSocket
        use crate::websocket::messages::SubscriptionType;
        self.ws_client
            .subscribe(vec![SubscriptionType::Orderbook { market_index }])
            .await?;

        // Track subscription
        self.subscriptions.write().unwrap().insert(
            instrument_id,
            SubscriptionState::OrderBook {
                market_index,
                depth,
            },
        );

        Ok(())
    }

    /// Unsubscribes from order book updates for the specified instrument.
    ///
    /// # Errors
    ///
    /// Returns an error if WebSocket unsubscription fails.
    pub async fn unsubscribe_order_book(
        &mut self,
        instrument_id: InstrumentId,
    ) -> Result<(), LighterError> {
        tracing::info!("Unsubscribing from order book for {}", instrument_id);

        let market_index = self.get_market_index_for_instrument(&instrument_id)?;

        // Unsubscribe via WebSocket
        use crate::websocket::messages::SubscriptionType;
        self.ws_client
            .unsubscribe(vec![SubscriptionType::Orderbook { market_index }])
            .await?;

        self.subscriptions.write().unwrap().remove(&instrument_id);

        Ok(())
    }

    /// Subscribes to trade updates for the specified instrument.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client is not connected
    /// - WebSocket subscription fails
    pub async fn subscribe_trades(
        &mut self,
        instrument_id: InstrumentId,
    ) -> Result<(), LighterError> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return Err(LighterError::Internal(
                "Client not connected".to_string(),
            ));
        }

        tracing::info!("Subscribing to trades for {}", instrument_id);

        let market_index = self.get_market_index_for_instrument(&instrument_id)?;

        // Subscribe via WebSocket
        use crate::websocket::messages::SubscriptionType;
        self.ws_client
            .subscribe(vec![SubscriptionType::Trades { market_index }])
            .await?;

        self.subscriptions
            .write()
            .unwrap()
            .insert(instrument_id, SubscriptionState::Trades { market_index });

        Ok(())
    }

    /// Unsubscribes from trade updates for the specified instrument.
    ///
    /// # Errors
    ///
    /// Returns an error if WebSocket unsubscription fails.
    pub async fn unsubscribe_trades(
        &mut self,
        instrument_id: InstrumentId,
    ) -> Result<(), LighterError> {
        tracing::info!("Unsubscribing from trades for {}", instrument_id);

        let market_index = self.get_market_index_for_instrument(&instrument_id)?;

        // Unsubscribe via WebSocket
        use crate::websocket::messages::SubscriptionType;
        self.ws_client
            .unsubscribe(vec![SubscriptionType::Trades { market_index }])
            .await?;

        self.subscriptions.write().unwrap().remove(&instrument_id);

        Ok(())
    }

    /// Subscribes to ticker updates for the specified instrument.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client is not connected
    /// - WebSocket subscription fails
    pub async fn subscribe_ticker(
        &mut self,
        instrument_id: InstrumentId,
    ) -> Result<(), LighterError> {
        if !self.is_connected.load(Ordering::Relaxed) {
            return Err(LighterError::Internal(
                "Client not connected".to_string(),
            ));
        }

        tracing::info!("Subscribing to ticker for {}", instrument_id);

        let market_index = self.get_market_index_for_instrument(&instrument_id)?;

        // Subscribe via WebSocket
        use crate::websocket::messages::SubscriptionType;
        self.ws_client
            .subscribe(vec![SubscriptionType::Ticker { market_index }])
            .await?;

        self.subscriptions
            .write()
            .unwrap()
            .insert(instrument_id, SubscriptionState::Ticker { market_index });

        Ok(())
    }

    /// Requests historical bar data for the specified instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - The instrument to request bars for
    /// * `bar_type` - The bar type specification
    /// * `start` - Optional start time
    /// * `end` - Optional end time
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - HTTP request fails
    /// - Data parsing fails
    pub async fn request_bars(
        &self,
        instrument_id: InstrumentId,
        bar_type: BarType,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<Vec<Bar>, LighterError> {
        tracing::info!(
            "Requesting bars for {} (type: {:?}, start: {:?}, end: {:?})",
            instrument_id,
            bar_type,
            start,
            end
        );

        // Get market index
        let market_index = self.get_market_index_for_instrument(&instrument_id)?;

        let interval = bar_type_to_lighter_interval(bar_type)?;

        // Build query parameters
        let mut params = vec![
            format!("order_book_id={}", market_index),
            format!("interval={}", interval),
        ];

        if let Some(start_time) = start {
            params.push(format!("start_time={}", start_time.timestamp_millis()));
        }

        if let Some(end_time) = end {
            params.push(format!("end_time={}", end_time.timestamp_millis()));
        }

        let query_string = params.join("&");

        // Make HTTP request
        use crate::http::endpoints::CANDLESTICKS;
        use crate::http::types::{LighterList, LighterResponse};

        let response: LighterResponse<LighterList<CandleStick>> =
            self.http_client.get(CANDLESTICKS, Some(&query_string)).await?;

        let candles = response
            .data
            .ok_or_else(|| LighterError::Parse("No candle data in response".to_string()))?
            .items;
        let precision = self
            .market_precisions
            .read()
            .unwrap()
            .get(&market_index)
            .copied()
            .unwrap_or(MarketPrecision {
                price_decimals: 2,
                size_decimals: DEFAULT_SIZE_DECIMALS,
            });

        let mut bars = Vec::with_capacity(candles.len());
        for candle in candles {
            bars.push(parse_candlestick_bar(
                &candle,
                bar_type,
                precision.price_decimals,
                precision.size_decimals,
            )?);
        }

        tracing::info!("Fetched {} bars for {}", bars.len(), instrument_id);
        Ok(bars)
    }

    /// Fetches available markets from Lighter.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - HTTP request fails
    /// - Response parsing fails
    pub async fn fetch_markets(&self) -> Result<Vec<MarketInfo>, LighterError> {
        tracing::info!("Fetching markets from Lighter");

        use crate::http::endpoints::ORDER_BOOKS;
        use crate::http::types::MarketsResponse;

        // Request markets from HTTP endpoint
        let response: MarketsResponse = self.http_client.get(ORDER_BOOKS, None).await?;

        // Extract market list
        let markets = response
            .data
            .ok_or_else(|| LighterError::Parse("No market data in response".to_string()))?
            .items;

        // Convert HTTP Market types to MarketInfo and update caches
        let mut market_infos = Vec::new();
        for market in markets {
            // Parse instrument from market data
            let instrument_id = InstrumentId::from(
                format!("{}.{}", market.symbol, *LIGHTER_VENUE).as_str()
            );

            use crate::data::types::decimal_places;
            let price_decimals = decimal_places(&market.tick_size, "tick_size")?;
            let size_decimals = decimal_places(&market.step_size, "step_size")?;

            // Create MarketInfo
            let market_info = MarketInfo {
                market_id: market.market_index as u32,
                symbol: market.symbol.to_string(),
                base_symbol: market.base_asset.to_string(),
                quote_symbol: market.quote_asset.to_string(),
                price_decimals,
                size_decimals,
                min_base_amount: market.min_order_size.parse::<f64>()
                    .unwrap_or(0.0)
                    .to_bits(),
                tick_size: market.tick_size.parse::<f64>()
                    .unwrap_or(0.01)
                    .to_bits(),
                status: match market.status {
                    crate::http::types::MarketStatus::Active =>
                        crate::common::enums::LighterMarketStatus::Active,
                    crate::http::types::MarketStatus::Inactive =>
                        crate::common::enums::LighterMarketStatus::Paused,
                    crate::http::types::MarketStatus::Delisted =>
                        crate::common::enums::LighterMarketStatus::Closed,
                },
            };

            // Parse instrument using types conversion
            use crate::data::types::parse_instrument;
            match parse_instrument(&market_info) {
                Ok(instrument) => {
                    // Update instruments cache
                    self.instruments.write().unwrap().insert(
                        instrument_id,
                        InstrumentAny::CryptoFuture(instrument),
                    );

                    // Update symbol mapping
                    self.symbol_to_instrument_id.write().unwrap().insert(
                        Ustr::from(&market.symbol),
                        instrument_id,
                    );

                    // Update market index mapping (forward)
                    self.instrument_to_market_index.write().unwrap().insert(
                        instrument_id,
                        market.market_index,
                    );

                    // Update market index mapping (reverse - for WebSocket routing)
                    self.market_index_to_instrument.write().unwrap().insert(
                        market.market_index,
                        instrument_id,
                    );

                    // Store price/size precision for this market
                    self.market_precisions.write().unwrap().insert(
                        market.market_index,
                        MarketPrecision {
                            price_decimals: market_info.price_decimals,
                            size_decimals: market_info.size_decimals,
                        },
                    );
                },
                Err(e) => {
                    tracing::warn!("Failed to parse instrument for {}: {}", market.symbol, e);
                }
            }

            market_infos.push(market_info);
        }

        tracing::info!("Fetched {} markets", market_infos.len());
        Ok(market_infos)
    }

    /// Gets the market symbol for the given instrument ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the instrument is not found in cache.
    #[allow(dead_code)]
    fn get_symbol_for_instrument(&self, instrument_id: &InstrumentId) -> Result<String, LighterError> {
        // Extract symbol from instrument_id (format: "SYMBOL.VENUE")
        let symbol = instrument_id.symbol.to_string();

        // Verify it exists in our cache
        let symbol_map = self.symbol_to_instrument_id.read().unwrap();
        let symbol_ustr = Ustr::from(&symbol);

        if symbol_map.contains_key(&symbol_ustr) {
            Ok(symbol)
        } else {
            Err(LighterError::Internal(format!(
                "Symbol for instrument {} not found in cache",
                instrument_id
            )))
        }
    }

    /// Gets the market index for the given instrument ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the instrument is not found in cache.
    fn get_market_index_for_instrument(&self, instrument_id: &InstrumentId) -> Result<u16, LighterError> {
        // Look up market index from our mapping
        let market_indices = self.instrument_to_market_index.read().unwrap();
        market_indices
            .get(instrument_id)
            .copied()
            .ok_or_else(|| {
                LighterError::Internal(format!(
                    "Instrument {} not found in market index cache. Call fetch_markets() first.",
                    instrument_id
                ))
            })
    }

    /// Starts background task for processing WebSocket messages.
    fn start_ws_message_handler(&mut self, mut msg_rx: tokio::sync::mpsc::UnboundedReceiver<crate::websocket::messages::InboundMessage>) {
        let cancellation_token = self.cancellation_token.clone();
        let client_id = self.client_id;

        // Clone Arc references for the spawned task
        let market_index_to_instrument = Arc::clone(&self.market_index_to_instrument);
        let market_precisions = Arc::clone(&self.market_precisions);
        let data_sender = self.data_sender.clone();

        let handle = tokio::spawn(async move {
            tracing::info!("Starting WebSocket message handler for {}", client_id);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::info!("WebSocket message handler shutting down");
                        break;
                    }

                    msg = msg_rx.recv() => {
                        match msg {
                            Some(message) => {
                                // Process the message based on its type
                                if let Err(e) = Self::process_ws_message(
                                    message,
                                    &market_index_to_instrument,
                                    &market_precisions,
                                    &data_sender,
                                ) {
                                    tracing::error!("Failed to process WebSocket message: {}", e);
                                }
                            }
                            None => {
                                tracing::warn!("WebSocket message channel closed");
                                break;
                            }
                        }
                    }
                }
            }

            tracing::info!("WebSocket message handler stopped");
        });

        self.tasks.push(handle);
    }

    /// Process a single WebSocket message and send data events.
    ///
    /// This is a static method to allow calling from the spawned task.
    fn process_ws_message(
        message: crate::websocket::messages::InboundMessage,
        market_index_to_instrument: &Arc<RwLock<AHashMap<u16, InstrumentId>>>,
        market_precisions: &Arc<RwLock<AHashMap<u16, MarketPrecision>>>,
        data_sender: &tokio::sync::mpsc::UnboundedSender<nautilus_common::messages::DataEvent>,
    ) -> Result<(), LighterError> {
        use crate::websocket::messages::InboundMessage;
        use crate::common::OrderBookLevel;
        use crate::data::types::{parse_order_book_deltas, parse_trade_tick};
        use crate::common::Trade;
        use nautilus_common::messages::DataEvent;

        match message {
            InboundMessage::OrderbookSnapshot { market_index, bids, asks, timestamp } |
            InboundMessage::OrderbookUpdate { market_index, bids, asks, timestamp } => {
                tracing::debug!(
                    "Processing orderbook update for market {} with {} bids and {} asks",
                    market_index,
                    bids.len(),
                    asks.len()
                );

                // Look up instrument ID
                let instrument_id = match market_index_to_instrument.read().unwrap().get(&market_index) {
                    Some(id) => *id,
                    None => {
                        tracing::warn!("Unknown market_index {} for orderbook update", market_index);
                        return Ok(());
                    }
                };

                let precision = market_precisions.read().unwrap()
                    .get(&market_index)
                    .copied()
                    .unwrap_or(MarketPrecision { price_decimals: 2, size_decimals: DEFAULT_SIZE_DECIMALS });

                use crate::data::types::parse_decimal_to_raw;
                let bid_levels: Vec<OrderBookLevel> = bids.iter()
                    .filter_map(|(price_str, size_str)| {
                        let price = parse_decimal_to_raw(price_str, precision.price_decimals, "bid price").ok()?;
                        let size = parse_decimal_to_raw(size_str, precision.size_decimals, "bid size").ok()?;
                        Some(OrderBookLevel { price, size })
                    })
                    .collect();

                let ask_levels: Vec<OrderBookLevel> = asks.iter()
                    .filter_map(|(price_str, size_str)| {
                        let price = parse_decimal_to_raw(price_str, precision.price_decimals, "ask price").ok()?;
                        let size = parse_decimal_to_raw(size_str, precision.size_decimals, "ask size").ok()?;
                        Some(OrderBookLevel { price, size })
                    })
                    .collect();

                // Convert to OrderBookDelta objects
                let sequence = timestamp as u64;
                match parse_order_book_deltas(
                    &bid_levels,
                    &ask_levels,
                    instrument_id,
                    precision.price_decimals,
                    precision.size_decimals,
                    sequence,
                ) {
                    Ok(deltas) => {
                        for delta in deltas {
                            let event = DataEvent::Data(nautilus_model::data::Data::Delta(delta));
                            if let Err(e) = data_sender.send(event) {
                                tracing::error!("Failed to send orderbook delta: {}", e);
                                return Err(LighterError::Internal(format!(
                                    "Failed to send data event: {}", e
                                )));
                            }
                        }
                        tracing::trace!("Sent {} orderbook deltas for {}", bid_levels.len() + ask_levels.len(), instrument_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse orderbook deltas: {}", e);
                        return Err(e);
                    }
                }
            }

            InboundMessage::Trade { market_index, trade_id, price, size, is_buy, timestamp } => {
                tracing::debug!(
                    "Processing trade for market {}: price={}, size={}, buy={}",
                    market_index,
                    price,
                    size,
                    is_buy
                );

                // Look up instrument ID
                let instrument_id = match market_index_to_instrument.read().unwrap().get(&market_index) {
                    Some(id) => *id,
                    None => {
                        tracing::warn!("Unknown market_index {} for trade", market_index);
                        return Ok(());
                    }
                };

                let precision = market_precisions.read().unwrap()
                    .get(&market_index)
                    .copied()
                    .unwrap_or(MarketPrecision { price_decimals: 2, size_decimals: DEFAULT_SIZE_DECIMALS });

                use crate::data::types::parse_decimal_to_raw;
                let price_u64 = parse_decimal_to_raw(&price, precision.price_decimals, "trade price")?;
                let size_u64 = parse_decimal_to_raw(&size, precision.size_decimals, "trade size")?;
                let trade_id_u64 = trade_id.parse::<u64>().unwrap_or(0);

                // Create Trade struct for conversion
                let trade = Trade {
                    id: trade_id_u64,
                    market_id: market_index as u32,
                    price: price_u64,
                    size: size_u64,
                    is_taker_ask: !is_buy, // is_buy means buyer is taker, so is_taker_ask = !is_buy
                    timestamp_ms: timestamp as u64,
                };

                match parse_trade_tick(
                    &trade,
                    instrument_id,
                    precision.price_decimals,
                    precision.size_decimals,
                ) {
                    Ok(tick) => {
                        let event = DataEvent::Data(nautilus_model::data::Data::Trade(tick));
                        if let Err(e) = data_sender.send(event) {
                            tracing::error!("Failed to send trade tick: {}", e);
                            return Err(LighterError::Internal(format!(
                                "Failed to send data event: {}", e
                            )));
                        }
                        tracing::trace!("Sent trade tick for {}", instrument_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse trade tick: {}", e);
                        return Err(e);
                    }
                }
            }

            InboundMessage::Ticker {
                market_index,
                last_price: _,
                bid_price,
                bid_size,
                ask_price,
                ask_size,
                volume_24h: _,
                high_24h: _,
                low_24h: _,
                timestamp,
            } => {
                let (Some(bid_price), Some(bid_size), Some(ask_price), Some(ask_size)) =
                    (bid_price, bid_size, ask_price, ask_size)
                else {
                    tracing::debug!(
                        "Skipping ticker quote for market {} because best bid/ask is incomplete",
                        market_index
                    );
                    return Ok(());
                };

                let instrument_id = match market_index_to_instrument.read().unwrap().get(&market_index) {
                    Some(id) => *id,
                    None => {
                        tracing::warn!("Unknown market_index {} for ticker", market_index);
                        return Ok(());
                    }
                };

                let precision = market_precisions.read().unwrap()
                    .get(&market_index)
                    .copied()
                    .unwrap_or(MarketPrecision { price_decimals: 2, size_decimals: DEFAULT_SIZE_DECIMALS });

                use crate::data::types::{parse_decimal_to_raw, parse_quote_tick};
                let bid_price_u64 = parse_decimal_to_raw(&bid_price, precision.price_decimals, "bid price")?;
                let bid_size_u64 = parse_decimal_to_raw(&bid_size, precision.size_decimals, "bid size")?;
                let ask_price_u64 = parse_decimal_to_raw(&ask_price, precision.price_decimals, "ask price")?;
                let ask_size_u64 = parse_decimal_to_raw(&ask_size, precision.size_decimals, "ask size")?;

                match parse_quote_tick(
                    bid_price_u64,
                    bid_size_u64,
                    ask_price_u64,
                    ask_size_u64,
                    instrument_id,
                    precision.price_decimals,
                    precision.size_decimals,
                    (timestamp * 1_000_000) as u64, // ms to ns
                ) {
                    Ok(tick) => {
                        let event = DataEvent::Data(nautilus_model::data::Data::Quote(tick));
                        if let Err(e) = data_sender.send(event) {
                            tracing::error!("Failed to send quote tick: {}", e);
                            return Err(LighterError::Internal(format!(
                                "Failed to send data event: {}", e
                            )));
                        }
                        tracing::trace!("Sent quote tick for {}", instrument_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse quote tick: {}", e);
                        return Err(e);
                    }
                }
            }

            InboundMessage::Pong => {
                tracing::trace!("Received pong");
            }

            InboundMessage::Error { code, message } => {
                tracing::error!("WebSocket error {}: {}", code, message);
                return Err(LighterError::WebSocket(format!("Error {}: {}", code, message)));
            }

            InboundMessage::SubscriptionSuccess { channel } => {
                tracing::info!("Successfully subscribed to channel: {}", channel);
            }

            InboundMessage::UnsubscriptionSuccess { channel } => {
                tracing::info!("Successfully unsubscribed from channel: {}", channel);
            }

            InboundMessage::AuthSuccess => {
                tracing::info!("WebSocket authentication successful");
            }

            InboundMessage::Raw(value) => {
                tracing::debug!("Received raw message: {:?}", value);
            }

            _ => {
                tracing::warn!("Unhandled WebSocket message type");
            }
        }

        Ok(())
    }
}

impl DataClient for LighterDataClient {
    fn client_id(&self) -> ClientId {
        self.client_id
    }

    fn venue(&self) -> Option<Venue> {
        Some(*LIGHTER_VENUE)
    }

    fn start(&mut self) -> anyhow::Result<()> {
        // Start background message handler
        tracing::info!("Starting Lighter data client");

        // The WebSocket connection and message processing would be started here
        // For now, we'll just mark it as started - actual WebSocket message
        // processing should be implemented when connect() is called

        // In a full implementation, you would:
        // 1. Spawn a background task to process WebSocket messages
        // 2. Handle incoming messages and route them appropriately
        // 3. Send data events to the NautilusTrader data engine

        tracing::info!("Lighter data client started");
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        // Stop all background tasks
        self.cancellation_token.cancel();
        Ok(())
    }

    fn reset(&mut self) -> anyhow::Result<()> {
        // Reset client state
        self.subscriptions.write().unwrap().clear();
        self.instruments.write().unwrap().clear();
        self.symbol_to_instrument_id.write().unwrap().clear();
        self.instrument_to_market_index.write().unwrap().clear();
        self.market_index_to_instrument.write().unwrap().clear();
        self.market_precisions.write().unwrap().clear();
        Ok(())
    }

    fn dispose(&mut self) -> anyhow::Result<()> {
        // Clean up resources
        let _ = self.stop();
        self.tasks.clear();
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    fn is_disconnected(&self) -> bool {
        !self.is_connected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::LighterEnvironment;

    // Note: These tests are marked as #[ignore] because they require the global
    // data event sender to be initialized by a runner. Run with:
    // cargo test --package nautilus-lighter -- --ignored

    #[test]
    #[ignore = "requires runner initialization"]
    fn test_client_creation() {
        let client_id = ClientId::from("LIGHTER-001");
        let config = LighterDataClientConfig::default();
        let client = LighterDataClient::new(client_id, config);
        assert!(client.is_ok());
    }

    #[test]
    #[ignore = "requires runner initialization"]
    fn test_client_initial_state() {
        let client_id = ClientId::from("LIGHTER-001");
        let config = LighterDataClientConfig::default();
        let client = LighterDataClient::new(client_id, config).unwrap();
        assert!(!client.is_connected());
        assert!(client.is_disconnected());
    }

    // Unit tests that don't require runner initialization
    #[test]
    fn test_subscription_state_display() {
        let state = SubscriptionState::OrderBook {
            market_index: 1,
            depth: Some(10),
        };
        assert!(matches!(state, SubscriptionState::OrderBook { .. }));
    }

    #[test]
    fn test_subscription_state_trades() {
        let state = SubscriptionState::Trades { market_index: 2 };
        assert!(matches!(state, SubscriptionState::Trades { .. }));
    }

    #[test]
    fn test_config_defaults() {
        let config = LighterDataClientConfig::default();
        // Default environment is Mainnet
        assert_eq!(config.environment, LighterEnvironment::Mainnet);
    }

    #[test]
    fn test_bar_type_to_lighter_interval() {
        use crate::data::types::create_bar_type;

        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let bar_type_1m = create_bar_type(instrument_id, "1m").unwrap();
        let bar_type_5m = create_bar_type(instrument_id, "5m").unwrap();
        let bar_type_15m = create_bar_type(instrument_id, "15m").unwrap();
        let bar_type_1h = create_bar_type(instrument_id, "1h").unwrap();
        let bar_type_4h = create_bar_type(instrument_id, "4h").unwrap();
        let bar_type_1d = create_bar_type(instrument_id, "1d").unwrap();

        assert_eq!(bar_type_to_lighter_interval(bar_type_1m).unwrap(), "1m");
        assert_eq!(bar_type_to_lighter_interval(bar_type_5m).unwrap(), "5m");
        assert_eq!(bar_type_to_lighter_interval(bar_type_15m).unwrap(), "15m");
        assert_eq!(bar_type_to_lighter_interval(bar_type_1h).unwrap(), "1h");
        assert_eq!(bar_type_to_lighter_interval(bar_type_4h).unwrap(), "4h");
        assert_eq!(bar_type_to_lighter_interval(bar_type_1d).unwrap(), "1d");
    }

    #[test]
    fn test_decimal_places_from_market_metadata() {
        use crate::data::types::decimal_places;

        assert_eq!(decimal_places("0.01", "tick_size").unwrap(), 2);
        assert_eq!(decimal_places("0.1", "tick_size").unwrap(), 1);
        assert_eq!(decimal_places("0.0001", "step_size").unwrap(), 4);
        assert_eq!(decimal_places("0.001", "step_size").unwrap(), 3);
    }

    #[test]
    fn test_parse_candlestick_bar() {
        use crate::data::types::create_bar_type;

        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let bar_type = create_bar_type(instrument_id, "1h").unwrap();
        let candle = CandleStick {
            timestamp: 1_734_200_000_000,
            open: "4100.00".to_string(),
            high: "4150.00".to_string(),
            low: "4090.00".to_string(),
            close: "4127.50".to_string(),
            volume: "1234.56".to_string(),
        };

        let bar = parse_candlestick_bar(&candle, bar_type, 2, 8).unwrap();

        assert_eq!(bar.open.as_f64(), 4100.00);
        assert_eq!(bar.high.as_f64(), 4150.00);
        assert_eq!(bar.low.as_f64(), 4090.00);
        assert_eq!(bar.close.as_f64(), 4127.50);
        assert_eq!(bar.volume.as_f64(), 1234.56);
        assert_eq!(bar.ts_event.as_u64(), 1_734_200_000_000_000_000);
    }

    fn public_ws_test_state(
    ) -> (
        Arc<RwLock<AHashMap<u16, InstrumentId>>>,
        Arc<RwLock<AHashMap<u16, MarketPrecision>>>,
    ) {
        let instrument_id = InstrumentId::from("ETH_USDC.LIGHTER");
        let market_index_to_instrument = Arc::new(RwLock::new(AHashMap::new()));
        market_index_to_instrument.write().unwrap().insert(1, instrument_id);
        let market_precisions = Arc::new(RwLock::new(AHashMap::new()));
        market_precisions.write().unwrap().insert(
            1,
            MarketPrecision {
                price_decimals: 2,
                size_decimals: DEFAULT_SIZE_DECIMALS,
            },
        );
        (market_index_to_instrument, market_precisions)
    }

    #[test]
    fn test_process_orderbook_snapshot_sends_deltas() {
        let (market_index_to_instrument, market_precisions) = public_ws_test_state();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        LighterDataClient::process_ws_message(
            crate::websocket::messages::InboundMessage::OrderbookSnapshot {
                market_index: 1,
                bids: vec![("4127.00".to_string(), "10.5".to_string())],
                asks: vec![("4128.00".to_string(), "5.2".to_string())],
                timestamp: 1_734_200_000_000,
            },
            &market_index_to_instrument,
            &market_precisions,
            &sender,
        )
        .unwrap();

        let first = receiver.try_recv().unwrap();
        let second = receiver.try_recv().unwrap();
        assert!(matches!(
            first,
            nautilus_common::messages::DataEvent::Data(nautilus_model::data::Data::Delta(_))
        ));
        assert!(matches!(
            second,
            nautilus_common::messages::DataEvent::Data(nautilus_model::data::Data::Delta(_))
        ));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_process_orderbook_update_sends_deltas() {
        let (market_index_to_instrument, market_precisions) = public_ws_test_state();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        LighterDataClient::process_ws_message(
            crate::websocket::messages::InboundMessage::OrderbookUpdate {
                market_index: 1,
                bids: vec![("4127.00".to_string(), "0".to_string())],
                asks: vec![("4128.00".to_string(), "5.2".to_string())],
                timestamp: 1_734_200_000_001,
            },
            &market_index_to_instrument,
            &market_precisions,
            &sender,
        )
        .unwrap();

        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_process_trade_sends_trade_tick() {
        let (market_index_to_instrument, market_precisions) = public_ws_test_state();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        LighterDataClient::process_ws_message(
            crate::websocket::messages::InboundMessage::Trade {
                market_index: 1,
                trade_id: "12345".to_string(),
                price: "4127.50".to_string(),
                size: "10.5".to_string(),
                is_buy: true,
                timestamp: 1_734_200_000_000,
            },
            &market_index_to_instrument,
            &market_precisions,
            &sender,
        )
        .unwrap();

        let event = receiver.try_recv().unwrap();
        match event {
            nautilus_common::messages::DataEvent::Data(nautilus_model::data::Data::Trade(tick)) => {
                assert_eq!(tick.price.as_f64(), 4127.50);
                assert_eq!(tick.size.as_f64(), 10.5);
            }
            _ => panic!("Expected trade tick"),
        }
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_process_ticker_without_bid_ask_skips_quote() {
        let (market_index_to_instrument, market_precisions) = public_ws_test_state();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        LighterDataClient::process_ws_message(
            crate::websocket::messages::InboundMessage::Ticker {
                market_index: 1,
                last_price: Some("4127.50".to_string()),
                bid_price: None,
                bid_size: None,
                ask_price: None,
                ask_size: None,
                volume_24h: Some("1000.0".to_string()),
                high_24h: Some("4200.00".to_string()),
                low_24h: Some("4100.00".to_string()),
                timestamp: 1_734_200_000_000,
            },
            &market_index_to_instrument,
            &market_precisions,
            &sender,
        )
        .unwrap();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_process_ticker_with_bid_ask_sends_quote() {
        let (market_index_to_instrument, market_precisions) = public_ws_test_state();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        LighterDataClient::process_ws_message(
            crate::websocket::messages::InboundMessage::Ticker {
                market_index: 1,
                last_price: Some("4127.50".to_string()),
                bid_price: Some("4127.00".to_string()),
                bid_size: Some("10.5".to_string()),
                ask_price: Some("4128.00".to_string()),
                ask_size: Some("5.2".to_string()),
                volume_24h: Some("1000.0".to_string()),
                high_24h: Some("4200.00".to_string()),
                low_24h: Some("4100.00".to_string()),
                timestamp: 1_734_200_000_000,
            },
            &market_index_to_instrument,
            &market_precisions,
            &sender,
        )
        .unwrap();

        let event = receiver.try_recv().unwrap();
        match event {
            nautilus_common::messages::DataEvent::Data(nautilus_model::data::Data::Quote(tick)) => {
                assert_eq!(tick.bid_price.as_f64(), 4127.00);
                assert_eq!(tick.ask_price.as_f64(), 4128.00);
                assert_eq!(tick.bid_size.as_f64(), 10.5);
                assert_eq!(tick.ask_size.as_f64(), 5.2);
            }
            _ => panic!("Expected quote tick"),
        }
    }
}
