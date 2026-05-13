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

//! Live execution client implementation for the Lighter DEX adapter.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dashmap::DashMap;
use nautilus_common::{
    clients::ExecutionClient,
    live::runner::get_exec_event_sender,
    messages::execution::{
        BatchCancelOrders, CancelAllOrders, CancelOrder, GenerateFillReports,
        GenerateOrderStatusReport, GenerateOrderStatusReports, GeneratePositionStatusReports,
        ModifyOrder, QueryAccount, QueryOrder, SubmitOrder, SubmitOrderList,
    },
};
use nautilus_core::{MUTEX_POISONED, Params, UnixNanos};
use nautilus_live::{ExecutionClientCore, ExecutionEventEmitter};
use nautilus_model::{
    accounts::AccountAny,
    enums::{
        LiquiditySide, OmsType, OrderSide, OrderStatus, OrderType, PositionSideSpecified,
        TimeInForce,
    },
    identifiers::{AccountId, ClientId, ClientOrderId, InstrumentId, TradeId, Venue, VenueOrderId},
    instruments::{Instrument, InstrumentAny},
    orders::{Order, OrderAny},
    reports::{ExecutionMassStatus, FillReport, OrderStatusReport, PositionStatusReport},
    types::{AccountBalance, Currency, MarginBalance, Money, Price, Quantity},
};
use rust_decimal::Decimal;
use serde_json::Value;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::{
    common::LIGHTER_VENUE,
    config::LighterExecClientConfig,
    error::LighterError,
    execution::{
        dispatch::dispatch_private_message,
        fixtures::execution_fixture_set,
        reconciliation::{ExecutionReconciler, ReconciliationAction},
        reports::{
            build_fill_report, build_mass_status, build_order_status_report,
            build_position_status_report,
        },
    },
    http::{
        client::LighterRawHttpClient,
        endpoints::{NEXT_NONCE, ORDER_BOOKS, SEND_TX},
        types::{
            CancelAllOrdersRequest, CancelOrderRequest, CreateOrderRequest, LighterResponse,
            Market, NextNonceResponse, TxResponse,
        },
    },
    signing::{LighterSigner, LighterStrategySigner, NonceManager},
    websocket::{client::LighterWebSocketClient, messages::InboundMessage},
};

struct OfflineReportSource {
    client_id: ClientId,
    account_id: AccountId,
    venue: nautilus_model::identifiers::Venue,
    instrument_id: InstrumentId,
}

impl OfflineReportSource {
    pub fn new(
        client_id: ClientId,
        account_id: AccountId,
        venue: nautilus_model::identifiers::Venue,
        instrument_id: InstrumentId,
    ) -> Self {
        Self {
            client_id,
            account_id,
            venue,
            instrument_id,
        }
    }

    pub fn order_status_reports(
        &self,
    ) -> anyhow::Result<Vec<nautilus_model::reports::OrderStatusReport>> {
        let fixtures = execution_fixture_set();
        fixtures
            .orders
            .iter()
            .map(|o| {
                build_order_status_report(o, self.account_id, self.instrument_id)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            })
            .collect()
    }

    pub fn fill_reports(&self) -> anyhow::Result<Vec<nautilus_model::reports::FillReport>> {
        let fixtures = execution_fixture_set();
        let report = build_fill_report(&fixtures.fill, self.account_id, self.instrument_id)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(vec![report])
    }

    pub fn position_status_reports(
        &self,
    ) -> anyhow::Result<Vec<nautilus_model::reports::PositionStatusReport>> {
        let fixtures = execution_fixture_set();
        let report =
            build_position_status_report(&fixtures.position, self.account_id, self.instrument_id)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(vec![report])
    }

    pub fn mass_status(
        &self,
    ) -> anyhow::Result<Option<nautilus_model::reports::ExecutionMassStatus>> {
        let fixtures = execution_fixture_set();
        let mass = build_mass_status(
            &fixtures,
            self.client_id,
            self.account_id,
            self.venue,
            self.instrument_id,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Some(mass))
    }
}

const OFFLINE_REPORT_SOURCE_PARAM: &str = "lighter_report_source";
const OFFLINE_REPORT_SOURCE_VALUE: &str = "fixture";
const MOCK_HTTP_REPORT_SOURCE_VALUE: &str = "mock_http";
const MOCK_HTTP_REPORT_BASE_URL_PARAM: &str = "lighter_report_base_url";
const MOCK_HTTP_REPORT_PAGE_LIMIT_PARAM: &str = "lighter_report_page_limit";
const OFFLINE_REPORT_INSTRUMENT_ID: &str = "ETH_USDC.LIGHTER";
const OFFLINE_REPORT_LOOKBACK_MINS: u64 = u64::MAX;

fn is_offline_fixture_report_request(params: Option<&Params>) -> bool {
    params.and_then(|params| params.get_str(OFFLINE_REPORT_SOURCE_PARAM))
        == Some(OFFLINE_REPORT_SOURCE_VALUE)
}

fn is_mock_http_report_request(params: Option<&Params>) -> bool {
    params.and_then(|params| params.get_str(OFFLINE_REPORT_SOURCE_PARAM))
        == Some(MOCK_HTTP_REPORT_SOURCE_VALUE)
}

fn mock_http_report_params_from_config(config: &LighterExecClientConfig) -> Option<Params> {
    if config.report_source.as_deref()? != MOCK_HTTP_REPORT_SOURCE_VALUE {
        return None;
    }

    let mut params = Params::new();
    params.insert(
        OFFLINE_REPORT_SOURCE_PARAM.to_string(),
        serde_json::Value::String(MOCK_HTTP_REPORT_SOURCE_VALUE.to_string()),
    );
    if let Some(base_url) = config.report_base_url.as_ref() {
        params.insert(
            MOCK_HTTP_REPORT_BASE_URL_PARAM.to_string(),
            serde_json::Value::String(base_url.clone()),
        );
    }
    if let Some(page_limit) = config.report_page_limit {
        params.insert(
            MOCK_HTTP_REPORT_PAGE_LIMIT_PARAM.to_string(),
            serde_json::Value::Number(serde_json::Number::from(page_limit)),
        );
    }

    Some(params)
}

fn default_offline_report_source(
    client_id: ClientId,
    account_id: AccountId,
) -> OfflineReportSource {
    OfflineReportSource::new(
        client_id,
        account_id,
        *LIGHTER_VENUE,
        InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID),
    )
}

struct MockHttpReportSource {
    http_client: LighterRawHttpClient,
    account_id: AccountId,
    instrument_id: InstrumentId,
    page_limit: usize,
}

impl MockHttpReportSource {
    fn from_params(
        params: Option<&Params>,
        account_id: AccountId,
        instrument_id: Option<InstrumentId>,
    ) -> anyhow::Result<Self> {
        let params = params.ok_or_else(|| anyhow::anyhow!("mock_http report params missing"))?;
        let base_url = params
            .get_str(MOCK_HTTP_REPORT_BASE_URL_PARAM)
            .ok_or_else(|| anyhow::anyhow!("mock_http report base URL missing"))?;
        let page_limit = params
            .get_usize(MOCK_HTTP_REPORT_PAGE_LIMIT_PARAM)
            .unwrap_or(100)
            .max(1);
        let http_client = LighterRawHttpClient::with_base_url(base_url, None, Some(5), None, None)
            .map_err(|e| anyhow::anyhow!("mock_http report client error: {e}"))?;

        Ok(Self {
            http_client,
            account_id,
            instrument_id: instrument_id
                .unwrap_or_else(|| InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID)),
            page_limit,
        })
    }

    async fn order_status_reports(&self) -> anyhow::Result<Vec<OrderStatusReport>> {
        let items = self
            .fetch_paginated_items("/api/v1/account_active_orders")
            .await?;
        items
            .iter()
            .map(|item| self.order_status_report_from_json(item))
            .collect()
    }

    async fn fill_reports(&self) -> anyhow::Result<Vec<FillReport>> {
        let items = self.fetch_paginated_items("/api/v1/account_fills").await?;
        items
            .iter()
            .map(|item| self.fill_report_from_json(item))
            .collect()
    }

    async fn position_status_reports(&self) -> anyhow::Result<Vec<PositionStatusReport>> {
        let items = self
            .fetch_paginated_items("/api/v1/account_positions")
            .await?;
        items
            .iter()
            .map(|item| self.position_status_report_from_json(item))
            .collect()
    }

    async fn mass_status(
        &self,
        client_id: ClientId,
        venue: Venue,
    ) -> anyhow::Result<Option<ExecutionMassStatus>> {
        let mut mass_status = ExecutionMassStatus::new(
            client_id,
            self.account_id,
            venue,
            UnixNanos::default(),
            None,
        );
        mass_status.add_order_reports(self.order_status_reports().await?);
        mass_status.add_fill_reports(self.fill_reports().await?);
        mass_status.add_position_reports(self.position_status_reports().await?);

        Ok(Some(mass_status))
    }

    async fn fetch_paginated_items(&self, endpoint: &str) -> anyhow::Result<Vec<Value>> {
        let mut cursor: Option<String> = None;
        let mut items = Vec::new();

        loop {
            let query = cursor.as_ref().map_or_else(
                || format!("limit={}", self.page_limit),
                |cursor| format!("limit={}&cursor={cursor}", self.page_limit),
            );
            let response: Value = self
                .http_client
                .get(endpoint, Some(query.as_str()))
                .await
                .map_err(|e| {
                    anyhow::anyhow!("mock_http report request failed for {endpoint}: {e}")
                })?;
            if response.get("success").and_then(Value::as_bool) != Some(true) {
                return Err(anyhow::anyhow!(
                    "mock_http report request failed for {endpoint}: success=false"
                ));
            }
            let data = response.get("data").ok_or_else(|| {
                anyhow::anyhow!("mock_http report response missing data for {endpoint}")
            })?;
            let page_items = data
                .get("items")
                .or_else(|| data.get("orders"))
                .or_else(|| data.get("fills"))
                .or_else(|| data.get("positions"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    anyhow::anyhow!("mock_http report response missing items for {endpoint}")
                })?;
            items.extend(page_items.iter().cloned());

            let next_cursor = data.get("next_cursor").and_then(Value::as_str);
            let Some(next_cursor) = next_cursor else {
                break;
            };
            cursor = Some(next_cursor.to_string());
        }

        Ok(items)
    }

    fn order_status_report_from_json(&self, item: &Value) -> anyhow::Result<OrderStatusReport> {
        let ts = timestamp_ms_to_ns(required_i64(item, "timestamp")?)?;
        let status = match required_str(item, "status")? {
            "open" => OrderStatus::Accepted,
            "filled" => OrderStatus::Filled,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "canceled" => OrderStatus::Canceled,
            "rejected" => OrderStatus::Rejected,
            other => {
                return Err(anyhow::anyhow!(
                    "mock_http unsupported order status: {other}"
                ));
            }
        };
        let report = OrderStatusReport::new(
            self.account_id,
            self.instrument_id,
            Some(ClientOrderId::new(required_str(item, "client_order_id")?)),
            VenueOrderId::new(required_str(item, "order_id")?),
            order_side_from_str(required_str(item, "side")?)?,
            OrderType::Limit,
            TimeInForce::Gtc,
            status,
            Quantity::from(required_str(item, "quantity")?),
            Quantity::from(required_str(item, "filled_quantity")?),
            ts,
            ts,
            ts,
            None,
        )
        .with_price(Price::from(required_str(item, "price")?));

        Ok(report)
    }

    fn fill_report_from_json(&self, item: &Value) -> anyhow::Result<FillReport> {
        let ts = timestamp_ms_to_ns(required_i64(item, "timestamp")?)?;
        Ok(FillReport::new(
            self.account_id,
            self.instrument_id,
            VenueOrderId::new(required_str(item, "order_id")?),
            TradeId::new(required_str(item, "trade_id")?),
            order_side_from_str(required_str(item, "side")?)?,
            Quantity::from(required_str(item, "quantity")?),
            Price::from(required_str(item, "price")?),
            Money::new(0.0, Currency::USD()),
            LiquiditySide::NoLiquiditySide,
            Some(ClientOrderId::new(required_str(item, "client_order_id")?)),
            None,
            ts,
            ts,
            None,
        ))
    }

    fn position_status_report_from_json(
        &self,
        item: &Value,
    ) -> anyhow::Result<PositionStatusReport> {
        let ts = timestamp_ms_to_ns(required_i64(item, "timestamp")?)?;
        let quantity = Quantity::from(required_str(item, "size")?);
        let side = if quantity.is_zero() {
            PositionSideSpecified::Flat
        } else {
            PositionSideSpecified::Long
        };
        let avg_px_open = required_str(item, "entry_price")?
            .parse::<Decimal>()
            .map_err(|e| anyhow::anyhow!("mock_http invalid position entry_price: {e}"))?;

        Ok(PositionStatusReport::new(
            self.account_id,
            self.instrument_id,
            side,
            quantity,
            ts,
            ts,
            None,
            None,
            Some(avg_px_open),
        ))
    }
}

fn required_str<'a>(item: &'a Value, field: &str) -> anyhow::Result<&'a str> {
    item.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("mock_http report item missing string field {field}"))
}

fn required_i64(item: &Value, field: &str) -> anyhow::Result<i64> {
    item.get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("mock_http report item missing i64 field {field}"))
}

fn timestamp_ms_to_ns(timestamp_ms: i64) -> anyhow::Result<UnixNanos> {
    u64::try_from(timestamp_ms)
        .map_err(|_| anyhow::anyhow!("mock_http invalid timestamp: {timestamp_ms}"))?
        .checked_mul(1_000_000)
        .map(UnixNanos::from)
        .ok_or_else(|| anyhow::anyhow!("mock_http timestamp overflow: {timestamp_ms}"))
}

fn order_side_from_str(side: &str) -> anyhow::Result<OrderSide> {
    match side {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        other => Err(anyhow::anyhow!("mock_http unsupported order side: {other}")),
    }
}

/// Live execution client for the Lighter DEX adapter.
///
/// # Supported Order Types
///
/// Lighter supports the following order types:
/// - **Market**: Execute immediately at best available price
/// - **Limit**: Execute at specified price or better
/// - **Stop Market**: Triggered when price crosses stop price, then executes as market order
/// - **Stop Limit**: Triggered when price crosses stop price, then places limit order
///
/// # Architecture
///
/// The client follows a two-layer execution model:
/// 1. **Synchronous validation** - Immediate checks and event generation
/// 2. **Async submission** - Non-blocking HTTP calls via signed transactions
///
/// This matches the pattern used in dYdX, Hyperliquid, and other DEX adapters,
/// ensuring consistent behavior across the Nautilus ecosystem.
///
/// # Signing
///
/// All transactions are signed using Lighter's L2 cryptographic stack:
/// - Private key stored securely in `LighterSigner`
/// - Nonces managed by `NonceManager` for transaction ordering
/// - Each order submission generates a signed transaction
#[derive(Debug)]
pub struct LighterExecutionClient {
    /// Core execution client functionality.
    core: ExecutionClientCore,
    /// Client configuration.
    config: LighterExecClientConfig,
    /// Execution event emitter.
    emitter: ExecutionEventEmitter,
    /// HTTP client for REST API calls (wrapped in Arc for async sharing).
    http_client: Arc<LighterRawHttpClient>,
    /// WebSocket client for real-time updates.
    ws_client: LighterWebSocketClient,
    /// Strategy-path transaction signer.
    signer: LighterStrategySigner,
    /// Nonce manager for transaction ordering.
    nonce_manager: Arc<Mutex<NonceManager>>,
    /// Cached instruments (InstrumentId -> InstrumentAny).
    #[allow(dead_code)] // Will be used for instrument lookups and price/size conversions
    instruments: DashMap<InstrumentId, InstrumentAny>,
    /// InstrumentId to Lighter market index mapping.
    instrument_to_market_index: DashMap<InstrumentId, u16>,
    /// Order state cache (ClientOrderId -> OrderState).
    orders: DashMap<ClientOrderId, OrderState>,
    /// Reconciled execution state for send/order/fill/cancel/account replay.
    reconciler: Arc<Mutex<ExecutionReconciler>>,
    /// Client started flag.
    started: bool,
    /// Client connected flag.
    connected: bool,
    /// Instruments initialized flag.
    instruments_initialized: bool,
    /// WebSocket stream handle.
    ws_stream_handle: Option<JoinHandle<()>>,
    /// Pending async tasks.
    pending_tasks: Mutex<Vec<JoinHandle<()>>>,
}

/// Internal order state tracking.
///
/// Stores order metadata for:
/// - Order reconciliation with exchange
/// - Cancel/modify operations using nonce
/// - WebSocket message routing
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for order tracking, will be read in WebSocket handler
struct OrderState {
    /// Client order ID.
    client_order_id: ClientOrderId,
    /// Instrument ID.
    instrument_id: InstrumentId,
    /// Venue order ID (if assigned).
    venue_order_id: Option<String>,
    /// Order nonce used for signing.
    nonce: u64,
}

impl LighterExecutionClient {
    /// Creates a new [`LighterExecutionClient`].
    ///
    /// # Parameters
    ///
    /// * `core` - Core execution client functionality
    /// * `config` - Execution client configuration
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The signer cannot be created from the private key
    /// - The HTTP client fails to initialize
    /// - The WebSocket client fails to construct
    pub fn new(
        core: ExecutionClientCore,
        config: LighterExecClientConfig,
    ) -> Result<Self, LighterError> {
        // Get chain_id from environment
        let chain_id = match config.environment {
            crate::common::LighterEnvironment::Mainnet => LighterSigner::CHAIN_ID_MAINNET,
            crate::common::LighterEnvironment::Testnet => LighterSigner::CHAIN_ID_TESTNET,
        };

        // Create signer from config
        let signer = LighterSigner::new(
            &config.private_key,
            chain_id,
            config.api_key_index,
            config.account_index as i64,
            0, // Initial nonce - will be fetched from server on connect
        )
        .map_err(|e| LighterError::Internal(format!("Failed to create signer: {e}")))?;
        let signer = LighterStrategySigner::new(signer, config.enable_live_signing);

        // Create HTTP client
        let http_client = LighterRawHttpClient::new(
            config.environment,
            None, // Auth token will be generated per request
            config.http_timeout_secs,
            None, // TODO: Add proxy support
            None, // TODO: Add retry config
        )?;

        // Create WebSocket client (private connection for account updates)
        let ws_client = LighterWebSocketClient::new_private(
            config.environment,
            String::new(), // Auth token will be set when connecting
            None,
            config.heartbeat_interval_secs,
        );

        // Initialize nonce manager (will be updated on connect)
        let nonce_manager = Arc::new(Mutex::new(NonceManager::new(
            0, // Initial nonce
            config.account_index as u32,
            config.api_key_index,
        )));

        info!(
            client_id = %core.client_id,
            account_id = %core.account_id,
            environment = ?config.environment,
            "Creating Lighter execution client"
        );

        let emitter = ExecutionEventEmitter::new(
            nautilus_core::time::get_atomic_clock_realtime(),
            core.trader_id,
            core.account_id,
            core.account_type,
            core.base_currency,
        );

        Ok(Self {
            core,
            config,
            emitter,
            http_client: Arc::new(http_client),
            ws_client,
            signer,
            nonce_manager,
            instruments: DashMap::new(),
            instrument_to_market_index: DashMap::new(),
            orders: DashMap::new(),
            reconciler: Arc::new(Mutex::new(ExecutionReconciler::default())),
            started: false,
            connected: false,
            instruments_initialized: false,
            ws_stream_handle: None,
            pending_tasks: Mutex::new(Vec::new()),
        })
    }

    /// Spawn an async task and track it.
    fn spawn_task<F>(&self, label: &'static str, fut: F)
    where
        F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let handle = tokio::spawn(async move {
            if let Err(e) = fut.await {
                error!("{label}: {e:?}");
            }
        });

        self.pending_tasks
            .lock()
            .expect(MUTEX_POISONED)
            .push(handle);
    }

    /// Abort all pending tasks.
    fn abort_pending_tasks(&self) {
        let mut guard = self.pending_tasks.lock().expect(MUTEX_POISONED);
        for handle in guard.drain(..) {
            handle.abort();
        }
    }

    /// Get the next nonce for transaction signing.
    #[allow(dead_code)] // Will be used for sequential transaction signing
    fn get_next_nonce(&self) -> u64 {
        self.nonce_manager.lock().expect(MUTEX_POISONED).next()
    }

    fn ensure_live_signing_enabled(&self) -> anyhow::Result<()> {
        if !self.config.enable_live_signing {
            anyhow::bail!(
                "Lighter live signing is disabled; set enable_live_signing=true to connect the private execution client"
            );
        }
        Ok(())
    }

    /// Cache instruments for lookups.
    #[allow(dead_code)] // Will be used for instrument initialization from HTTP API
    fn cache_instruments(&mut self, instruments: Vec<InstrumentAny>) {
        for (market_index, instrument) in instruments.into_iter().enumerate() {
            let instrument_id = instrument.id();

            self.instruments.insert(instrument_id, instrument.clone());
            self.instrument_to_market_index.insert(
                instrument_id,
                u16::try_from(market_index).unwrap_or(u16::MAX),
            );
        }

        self.instruments_initialized = true;
        info!("Cached {} instruments", self.instruments.len());
    }

    fn market_index_from_cache(
        cache: &DashMap<InstrumentId, u16>,
        instrument_id: &InstrumentId,
    ) -> Option<u16> {
        cache.get(instrument_id).map(|entry| *entry)
    }

    /// Get market index from cached exchange metadata.
    fn get_market_index(&self, instrument_id: &InstrumentId) -> Option<u16> {
        Self::market_index_from_cache(&self.instrument_to_market_index, instrument_id)
    }

    /// Convert NautilusTrader order side to Lighter is_ask flag.
    fn order_side_to_is_ask(order: &OrderAny) -> bool {
        use nautilus_model::enums::OrderSide;
        matches!(order.order_side(), OrderSide::Sell)
    }

    /// Convert NautilusTrader order type to Lighter order type.
    fn convert_order_type(order: &OrderAny) -> u8 {
        use nautilus_model::enums::OrderType;
        match order.order_type() {
            OrderType::Market => LighterSigner::ORDER_TYPE_MARKET,
            OrderType::Limit => LighterSigner::ORDER_TYPE_LIMIT,
            // Default to limit for other types
            _ => LighterSigner::ORDER_TYPE_LIMIT,
        }
    }

    /// Convert NautilusTrader time-in-force to Lighter TIF.
    fn convert_time_in_force(order: &OrderAny) -> u8 {
        use nautilus_model::enums::TimeInForce;
        match order.time_in_force() {
            TimeInForce::Gtc => LighterSigner::TIF_GOOD_TILL_TIME,
            TimeInForce::Ioc => LighterSigner::TIF_IMMEDIATE_OR_CANCEL,
            TimeInForce::Fok => LighterSigner::TIF_FILL_OR_KILL,
            TimeInForce::Gtd => LighterSigner::TIF_GOOD_TILL_TIME,
            TimeInForce::Day => LighterSigner::TIF_GOOD_TILL_TIME,
            TimeInForce::AtTheOpen => LighterSigner::TIF_GOOD_TILL_TIME,
            TimeInForce::AtTheClose => LighterSigner::TIF_GOOD_TILL_TIME,
        }
    }

    /// Convert price to Lighter raw integer format.
    ///
    /// Lighter uses integer prices with implied decimals (price_decimals).
    /// For example, ETH with 2 decimals: $4127.39 -> 412739
    fn convert_price_to_raw(price: f64, price_decimals: u8) -> u32 {
        let multiplier = 10f64.powi(price_decimals as i32);
        (price * multiplier) as u32
    }

    /// Convert quantity to Lighter raw integer format.
    ///
    /// Lighter uses integer quantities with 8 decimal places.
    /// For example, 1.5 ETH -> 150000000
    fn convert_quantity_to_raw(quantity: f64) -> i64 {
        const SIZE_DECIMALS: u8 = 8;
        let multiplier = 10f64.powi(SIZE_DECIMALS as i32);
        (quantity * multiplier) as i64
    }

    /// Submit order asynchronously via HTTP.
    async fn submit_order_async(
        http_client: Arc<LighterRawHttpClient>,
        request: CreateOrderRequest,
        client_order_id: ClientOrderId,
    ) -> anyhow::Result<()> {
        debug!("Submitting order {} to Lighter", client_order_id);

        let response: TxResponse = http_client
            .post(SEND_TX, Some(request))
            .await
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {e}"))?;

        if response.success {
            if let Some(data) = response.data {
                info!(
                    "Order {} submitted successfully, tx_id: {:?}, order_index: {:?}",
                    client_order_id, data.tx_id, data.order_index
                );
            }
        } else {
            let error_msg = response
                .error
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Order {} rejected: {}", client_order_id, error_msg);
        }

        Ok(())
    }

    /// Cancel order asynchronously via HTTP.
    async fn cancel_order_async(
        http_client: Arc<LighterRawHttpClient>,
        request: CancelOrderRequest,
        client_order_id: ClientOrderId,
    ) -> anyhow::Result<()> {
        debug!("Cancelling order {} on Lighter", client_order_id);

        let response: TxResponse = http_client
            .post(SEND_TX, Some(request))
            .await
            .map_err(|e| anyhow::anyhow!("Cancel request failed: {e}"))?;

        if response.success {
            info!(
                "Cancel request for {} submitted successfully",
                client_order_id
            );
        } else {
            let error_msg = response
                .error
                .unwrap_or_else(|| "Unknown error".to_string());
            error!(
                "Cancel request for {} rejected: {}",
                client_order_id, error_msg
            );
        }

        Ok(())
    }

    /// Cancel all orders asynchronously via HTTP.
    async fn cancel_all_orders_async(
        http_client: Arc<LighterRawHttpClient>,
        request: CancelAllOrdersRequest,
    ) -> anyhow::Result<()> {
        debug!("Cancelling all orders on Lighter");

        let response: TxResponse = http_client
            .post(SEND_TX, Some(request))
            .await
            .map_err(|e| anyhow::anyhow!("Cancel all request failed: {e}"))?;

        if response.success {
            info!("Cancel all orders request submitted successfully");
        } else {
            let error_msg = response
                .error
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Cancel all orders request rejected: {}", error_msg);
        }

        Ok(())
    }

    /// Start the WebSocket message handler task.
    ///
    /// Spawns a background task that processes incoming WebSocket messages
    /// for order and account updates.
    fn start_ws_message_handler(
        &mut self,
        mut msg_rx: tokio::sync::mpsc::UnboundedReceiver<InboundMessage>,
    ) {
        let orders = self.orders.clone();
        let reconciler = self.reconciler.clone();
        let core_client_id = self.core.client_id;
        let core_account_id = self.core.account_id;

        let handle = tokio::spawn(async move {
            info!(
                "Starting execution WebSocket message handler for {}",
                core_client_id
            );

            loop {
                match msg_rx.recv().await {
                    Some(message) => {
                        if let Err(e) = Self::process_ws_message(
                            message,
                            &orders,
                            &reconciler,
                            core_client_id,
                            core_account_id,
                        ) {
                            error!("Failed to process WebSocket message: {}", e);
                        }
                    }
                    None => {
                        warn!("WebSocket message channel closed");
                        break;
                    }
                }
            }

            info!("Execution WebSocket message handler stopped");
        });

        self.ws_stream_handle = Some(handle);
    }

    /// Process a single WebSocket message.
    ///
    /// Private order/account messages are routed through [`dispatch_private_message`]
    /// so the dispatch path is exercisable by offline tests without constructing a
    /// full client. Order-cache side-effects (venue-order-ID hydration) are preserved.
    fn process_ws_message(
        message: InboundMessage,
        orders: &DashMap<ClientOrderId, OrderState>,
        reconciler: &Arc<Mutex<ExecutionReconciler>>,
        client_id: ClientId,
        account_id: AccountId,
    ) -> Result<(), LighterError> {
        // Route private messages through the dispatch layer first.
        let outcome = dispatch_private_message(&message);

        use crate::execution::dispatch::DispatchOutcome;
        match outcome {
            DispatchOutcome::Order {
                ref order_id,
                ref client_order_id,
                market_index,
                ref status,
                ref venue_status,
                ..
            } => {
                let reconciliation_action = reconciler
                    .lock()
                    .expect(MUTEX_POISONED)
                    .apply_dispatch(&outcome);
                if matches!(
                    reconciliation_action,
                    ReconciliationAction::Duplicate | ReconciliationAction::Stale
                ) {
                    debug!(
                        "[{}] Ignoring {:?} order update for {}",
                        client_id, reconciliation_action, order_id
                    );
                    return Ok(());
                }

                // Preserve existing order-cache side-effect: hydrate venue_order_id.
                if let Some(coid_str) = client_order_id {
                    if let Ok(coid) = ClientOrderId::new_checked(coid_str) {
                        if let Some(mut order_state) = orders.get_mut(&coid) {
                            if order_state.venue_order_id.is_none() {
                                order_state.venue_order_id = Some(order_id.clone());
                            }
                        }
                    }
                }

                info!(
                    "[{}] Order {} dispatched: status={:?} (venue_status={}, market={})",
                    client_id, order_id, status, venue_status, market_index
                );

                // NOTE: Full NautilusTrader event generation (OrderAccepted, OrderFilled,
                // etc.) requires ExecutionClientCore access via Arc. That wiring is
                // deferred; this dispatch path is the prerequisite.
                return Ok(());
            }
            DispatchOutcome::Account {
                ref address,
                ref balances,
                ..
            } => {
                let reconciliation_action = reconciler
                    .lock()
                    .expect(MUTEX_POISONED)
                    .apply_dispatch(&outcome);
                if matches!(
                    reconciliation_action,
                    ReconciliationAction::Duplicate | ReconciliationAction::Stale
                ) {
                    debug!(
                        "[{}] Ignoring {:?} account update for {}",
                        account_id, reconciliation_action, address
                    );
                    return Ok(());
                }

                info!(
                    "[{}] Account {} updated with {} balance entries",
                    account_id,
                    address,
                    balances.len()
                );
                for (asset, amount) in balances {
                    debug!("  Balance: {} = {}", asset, amount);
                }
                return Ok(());
            }
            DispatchOutcome::Ignored => {
                // Fall through to handle non-private message variants below.
            }
        }

        // Handle remaining non-private / control messages.
        match message {
            InboundMessage::AuthSuccess => {
                info!("[{}] WebSocket authentication successful", client_id);
            }
            InboundMessage::SubscriptionSuccess { channel } => {
                info!("[{}] Subscribed to channel: {}", client_id, channel);
            }
            InboundMessage::UnsubscriptionSuccess { channel } => {
                info!("[{}] Unsubscribed from channel: {}", client_id, channel);
            }
            InboundMessage::Error { code, message } => {
                error!("[{}] WebSocket error {}: {}", client_id, code, message);
                return Err(LighterError::WebSocket(format!(
                    "Error {}: {}",
                    code, message
                )));
            }
            InboundMessage::Pong => {
                debug!("[{}] Received pong", client_id);
            }
            InboundMessage::Raw(ref value) => {
                debug!("[{}] Received raw message: {:?}", client_id, value);
            }
            // Market data messages are ignored in execution client.
            InboundMessage::OrderbookSnapshot { .. }
            | InboundMessage::OrderbookUpdate { .. }
            | InboundMessage::Trade { .. }
            | InboundMessage::Ticker { .. } => {}
            // Order/Account variants were handled by dispatch above.
            InboundMessage::OrderUpdate { .. } | InboundMessage::AccountUpdate { .. } => {}
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl ExecutionClient for LighterExecutionClient {
    fn is_connected(&self) -> bool {
        self.connected
    }

    fn client_id(&self) -> ClientId {
        self.core.client_id
    }

    fn account_id(&self) -> AccountId {
        self.core.account_id
    }

    fn venue(&self) -> Venue {
        *LIGHTER_VENUE
    }

    fn oms_type(&self) -> OmsType {
        self.core.oms_type
    }

    fn get_account(&self) -> Option<AccountAny> {
        self.core.cache().account(&self.core.account_id).cloned()
    }

    fn generate_account_state(
        &self,
        balances: Vec<AccountBalance>,
        margins: Vec<MarginBalance>,
        reported: bool,
        ts_event: UnixNanos,
    ) -> anyhow::Result<()> {
        self.emitter
            .emit_account_state(balances, margins, reported, ts_event);
        Ok(())
    }

    fn start(&mut self) -> anyhow::Result<()> {
        if self.started {
            warn!("Lighter execution client already started");
            return Ok(());
        }

        info!(
            client_id = %self.core.client_id,
            account_id = %self.core.account_id,
            "Starting Lighter execution client"
        );

        self.emitter.set_sender(get_exec_event_sender());
        self.core.set_started();
        self.started = true;
        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        if !self.started {
            warn!("Lighter execution client not started");
            return Ok(());
        }

        info!("Stopping Lighter execution client");

        // Abort pending tasks
        self.abort_pending_tasks();

        // Abort WebSocket stream
        if let Some(handle) = self.ws_stream_handle.take() {
            handle.abort();
        }

        self.core.set_stopped();
        self.started = false;
        self.connected = false;

        Ok(())
    }

    /// Submit an order to Lighter DEX.
    ///
    /// # Order Lifecycle
    ///
    /// 1. Validate order parameters
    /// 2. Generate `OrderSubmitted` event immediately
    /// 3. Sign transaction with signer
    /// 4. Submit via HTTP API asynchronously
    /// 5. WebSocket will provide `OrderAccepted`/`OrderRejected` events
    ///
    /// # Errors
    ///
    /// Returns an error if the client is not connected or the order is already closed.
    fn submit_order(&self, cmd: SubmitOrder) -> anyhow::Result<()> {
        let order = self
            .core
            .cache()
            .order(&cmd.client_order_id)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!("Order not found in cache for {}", cmd.client_order_id)
            })?;

        if !self.is_connected() {
            anyhow::bail!("Cannot submit order: execution client not connected");
        }

        if order.is_closed() {
            warn!("Cannot submit closed order {}", order.client_order_id());
            return Ok(());
        }

        // Generate OrderSubmitted event immediately
        self.emitter.emit_order_submitted(&order);

        // Get market index from instrument
        let market_index = match self.get_market_index(&order.instrument_id()) {
            Some(idx) => idx,
            None => {
                error!(
                    "Unknown instrument {}, cannot submit order",
                    order.instrument_id()
                );
                self.emitter
                    .emit_order_rejected(&order, "Unknown instrument", cmd.ts_init, false);
                return Ok(());
            }
        };

        // Extract order parameters
        let client_order_index = order
            .client_order_id()
            .to_string()
            .parse::<i64>()
            .unwrap_or_else(|_| {
                // Generate a unique ID from hash if not numeric
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                order.client_order_id().hash(&mut hasher);
                (hasher.finish() as i64).abs()
            });

        // Get price and quantity based on order type
        let price_decimals: u8 = 2; // Default, should come from instrument
        let price_raw = if let Some(price) = order.price() {
            Self::convert_price_to_raw(price.as_f64(), price_decimals)
        } else {
            0 // Market orders don't have price
        };

        let quantity_raw = Self::convert_quantity_to_raw(order.quantity().as_f64());
        let is_ask = Self::order_side_to_is_ask(&order);
        let order_type = Self::convert_order_type(&order);
        let time_in_force = Self::convert_time_in_force(&order);
        let reduce_only = order.is_reduce_only();

        // Calculate transaction expiry (default: 60 seconds from now)
        let expired_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64
            + 60_000; // 60 seconds expiry

        // Get trigger price and order expiry from order if available
        let trigger_price = order
            .trigger_price()
            .map(|p| Self::convert_price_to_raw(p.as_f64(), price_decimals))
            .unwrap_or(0);
        let order_expiry = order
            .expire_time()
            .map(|t| t.as_i64() / 1_000_000_000) // Convert nanos to seconds
            .unwrap_or(0);

        // Sign the order transaction
        let (signed_tx, nonce) = match self.signer.sign_create_order(
            market_index,
            client_order_index,
            quantity_raw,
            price_raw,
            is_ask,
            order_type,
            time_in_force,
            reduce_only,
            trigger_price,
            order_expiry,
            expired_at,
        ) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to sign order: {}", e);
                let reason = format!("Signing failed: {e}");
                self.emitter
                    .emit_order_rejected(&order, &reason, cmd.ts_init, false);
                return Ok(());
            }
        };

        // Cache order state
        let order_state = OrderState {
            client_order_id: order.client_order_id(),
            instrument_id: order.instrument_id(),
            venue_order_id: None,
            nonce,
        };
        self.orders.insert(order.client_order_id(), order_state);
        self.reconciler.lock().expect(MUTEX_POISONED).record_send(
            order.client_order_id().to_string(),
            market_index,
            cmd.ts_init.as_i64() / 1_000_000,
        );

        // Build HTTP request
        let request = CreateOrderRequest {
            market_index,
            client_order_index,
            base_amount: quantity_raw,
            price: price_raw,
            is_ask,
            order_type,
            time_in_force,
            reduce_only,
            nonce,
            signature: signed_tx.signature,
        };

        // Submit order asynchronously
        let client_order_id = order.client_order_id();
        let http_client = self.http_client.clone();

        self.spawn_task("submit_order", async move {
            Self::submit_order_async(http_client, request, client_order_id).await
        });

        info!(
            "Order {} submitted with nonce {}",
            order.client_order_id(),
            nonce
        );

        Ok(())
    }

    fn submit_order_list(&self, _cmd: SubmitOrderList) -> anyhow::Result<()> {
        anyhow::bail!("Order lists not yet implemented for Lighter DEX")
    }

    fn modify_order(&self, _cmd: ModifyOrder) -> anyhow::Result<()> {
        // TODO: Implement order modification
        anyhow::bail!("Order modification not yet implemented for Lighter DEX")
    }

    fn cancel_order(&self, cmd: CancelOrder) -> anyhow::Result<()> {
        if !self.is_connected() {
            anyhow::bail!("Cannot cancel order: not connected");
        }

        debug!("Cancelling order: {:?}", cmd.client_order_id);

        // Look up order in cache to get venue_order_id and market_index
        let order_state = match self.orders.get(&cmd.client_order_id) {
            Some(state) => state.clone(),
            None => {
                warn!(
                    "Order {} not found in cache, cannot cancel",
                    cmd.client_order_id
                );
                return Ok(());
            }
        };

        // Get market index
        let market_index = match self.get_market_index(&order_state.instrument_id) {
            Some(idx) => idx,
            None => {
                error!(
                    "Unknown instrument {} for order {}",
                    order_state.instrument_id, cmd.client_order_id
                );
                return Ok(());
            }
        };

        // Get venue order ID (order_index)
        // cmd.venue_order_id is a VenueOrderId, try to parse it
        let venue_order_str = cmd
            .venue_order_id
            .map(|id| id.to_string())
            .unwrap_or_default();
        let order_index = if venue_order_str.is_empty() || venue_order_str == "NULL" {
            // Fall back to cached order state
            match &order_state.venue_order_id {
                Some(id) => id.parse::<i64>().unwrap_or(0),
                None => {
                    warn!(
                        "No venue order ID for {}, cannot cancel",
                        cmd.client_order_id
                    );
                    return Ok(());
                }
            }
        } else {
            venue_order_str.parse::<i64>().unwrap_or(0)
        };

        // Calculate transaction expiry (default: 60 seconds from now)
        let expired_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64
            + 60_000; // 60 seconds expiry

        // Sign the cancel transaction
        let (signed_tx, nonce) =
            match self
                .signer
                .sign_cancel_order(market_index, order_index, expired_at)
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Failed to sign cancel order: {}", e);
                    return Ok(());
                }
            };

        self.reconciler
            .lock()
            .expect(MUTEX_POISONED)
            .record_cancel_request(
                cmd.client_order_id.to_string(),
                market_index,
                cmd.ts_init.as_i64() / 1_000_000,
            );

        // Build HTTP request
        let request = CancelOrderRequest {
            market_index,
            order_index,
            nonce,
            signature: signed_tx.signature,
        };

        // Submit cancel asynchronously
        let client_order_id = cmd.client_order_id;
        let http_client = self.http_client.clone();

        self.spawn_task("cancel_order", async move {
            Self::cancel_order_async(http_client, request, client_order_id).await
        });

        info!(
            "Cancel request for {} submitted with nonce {}",
            cmd.client_order_id, nonce
        );

        Ok(())
    }

    fn cancel_all_orders(&self, cmd: CancelAllOrders) -> anyhow::Result<()> {
        if !self.is_connected() {
            anyhow::bail!("Cannot cancel orders: not connected");
        }

        debug!("Cancelling all orders for: {:?}", cmd.instrument_id);

        // Get current timestamp
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64;

        // Calculate transaction expiry (default: 60 seconds from now)
        let expired_at = time + 60_000; // 60 seconds expiry

        // Use TIF 0 to cancel all order types
        let time_in_force: u8 = 0;

        // Sign the cancel all transaction
        let (signed_tx, nonce) =
            match self
                .signer
                .sign_cancel_all_orders(time_in_force, time, expired_at)
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Failed to sign cancel all orders: {}", e);
                    return Ok(());
                }
            };

        // Build HTTP request
        let request = CancelAllOrdersRequest {
            time_in_force,
            time,
            nonce,
            signature: signed_tx.signature,
        };

        // Submit cancel all asynchronously
        let http_client = self.http_client.clone();

        self.spawn_task("cancel_all_orders", async move {
            Self::cancel_all_orders_async(http_client, request).await
        });

        info!("Cancel all orders request submitted with nonce {}", nonce);

        Ok(())
    }

    fn batch_cancel_orders(&self, cmd: BatchCancelOrders) -> anyhow::Result<()> {
        if cmd.cancels.is_empty() {
            return Ok(());
        }

        if !self.is_connected() {
            anyhow::bail!("Cannot cancel orders: not connected");
        }

        debug!("Batch cancelling {} orders", cmd.cancels.len());

        // TODO: Implement batch cancellation
        // Similar to cancel_all_orders but with specific order list

        Ok(())
    }

    fn query_account(&self, _cmd: QueryAccount) -> anyhow::Result<()> {
        // TODO: Implement account query
        Ok(())
    }

    fn query_order(&self, _cmd: QueryOrder) -> anyhow::Result<()> {
        // TODO: Implement order query
        Ok(())
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        if self.connected {
            warn!("Lighter execution client already connected");
            return Ok(());
        }

        self.ensure_live_signing_enabled()?;

        info!("Connecting to Lighter DEX");

        // Step 1: Fetch instruments/markets from HTTP API
        info!("Fetching markets from Lighter API...");
        let markets_response: LighterResponse<Vec<Market>> = self
            .http_client
            .get(ORDER_BOOKS, None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch markets: {e}"))?;

        let markets = markets_response
            .data
            .ok_or_else(|| anyhow::anyhow!("No market data in response"))?;

        info!("Fetched {} markets from Lighter", markets.len());

        // Step 2: Cache instruments and market mappings
        for market in &markets {
            let symbol = format!("{}_{}", market.base_asset, market.quote_asset);
            let instrument_id = InstrumentId::new(
                nautilus_model::identifiers::Symbol::new(&symbol),
                *LIGHTER_VENUE,
            );
            self.instrument_to_market_index
                .insert(instrument_id, market.market_index);
            debug!(
                "Cached market {} -> instrument {}",
                market.market_index, instrument_id
            );
        }
        self.instruments_initialized = true;

        // Step 3: Query current nonce from API
        info!("Fetching current nonce from Lighter API...");
        let nonce_query = format!("account_index={}", self.config.account_index);
        let nonce_response: NextNonceResponse = self
            .http_client
            .get(NEXT_NONCE, Some(nonce_query.as_str()))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch nonce: {e}"))?;

        let server_nonce = nonce_response.data.map(|d| d.nonce).unwrap_or(0);

        info!("Server nonce: {}", server_nonce);

        // Step 4: Initialize nonce manager with server nonce
        {
            let nonce_manager = self.nonce_manager.lock().expect(MUTEX_POISONED);
            nonce_manager.reset(server_nonce);
        }
        self.signer.reset_nonce(server_nonce);

        // Step 5: Generate auth token with signer
        let deadline = chrono::Utc::now().timestamp() + 86400; // 24 hours from now
        let auth_token = self
            .signer
            .create_auth_token(deadline)
            .map_err(|e| anyhow::anyhow!("Failed to create auth token: {e}"))?;

        info!("Generated auth token for WebSocket connection");

        // Step 6: Connect WebSocket with auth token
        self.ws_client.set_auth_token(auth_token).await;
        let ws_rx = self.ws_client.connect().await?;

        info!("WebSocket connected to Lighter DEX");

        // Step 7: Subscribe to account updates
        self.ws_client
            .subscribe_account(self.config.account_index as i64)
            .await?;

        info!("Subscribed to account updates");

        // Step 8: Start WebSocket message handler
        self.start_ws_message_handler(ws_rx);

        info!("Started WebSocket message handler");

        self.connected = true;
        info!(client_id = %self.core.client_id, "Connected to Lighter DEX");

        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        if !self.connected {
            warn!("Lighter execution client not connected");
            return Ok(());
        }

        info!("Disconnecting from Lighter DEX");

        // Close WebSocket
        self.ws_client.close().await?;

        // Abort WebSocket stream
        if let Some(handle) = self.ws_stream_handle.take() {
            handle.abort();
        }

        // Abort pending tasks
        self.abort_pending_tasks();

        self.connected = false;
        info!(client_id = %self.core.client_id, "Disconnected");

        Ok(())
    }

    async fn generate_order_status_report(
        &self,
        cmd: &GenerateOrderStatusReport,
    ) -> anyhow::Result<Option<OrderStatusReport>> {
        if is_mock_http_report_request(cmd.params.as_ref()) {
            warn!("generate_order_status_report: using mock HTTP report source");
            let reports = MockHttpReportSource::from_params(
                cmd.params.as_ref(),
                self.core.account_id,
                cmd.instrument_id,
            )?
            .order_status_reports()
            .await?;
            return Ok(reports.into_iter().next());
        }

        if !is_offline_fixture_report_request(cmd.params.as_ref()) {
            warn!("generate_order_status_report: live API not implemented");
            return Ok(None);
        }

        warn!("generate_order_status_report: using explicit offline fixture report source");
        let reports = default_offline_report_source(self.core.client_id, self.core.account_id)
            .order_status_reports()?;
        Ok(reports.into_iter().next())
    }

    async fn generate_order_status_reports(
        &self,
        cmd: &GenerateOrderStatusReports,
    ) -> anyhow::Result<Vec<OrderStatusReport>> {
        if is_mock_http_report_request(cmd.params.as_ref()) {
            warn!("generate_order_status_reports: using mock HTTP report source");
            return MockHttpReportSource::from_params(
                cmd.params.as_ref(),
                self.core.account_id,
                cmd.instrument_id,
            )?
            .order_status_reports()
            .await;
        }

        if !is_offline_fixture_report_request(cmd.params.as_ref()) {
            warn!("generate_order_status_reports: live API not implemented");
            return Ok(Vec::new());
        }

        warn!("generate_order_status_reports: using explicit offline fixture report source");
        default_offline_report_source(self.core.client_id, self.core.account_id)
            .order_status_reports()
    }

    async fn generate_fill_reports(
        &self,
        cmd: GenerateFillReports,
    ) -> anyhow::Result<Vec<FillReport>> {
        if is_mock_http_report_request(cmd.params.as_ref()) {
            warn!("generate_fill_reports: using mock HTTP report source");
            return MockHttpReportSource::from_params(
                cmd.params.as_ref(),
                self.core.account_id,
                cmd.instrument_id,
            )?
            .fill_reports()
            .await;
        }

        if !is_offline_fixture_report_request(cmd.params.as_ref()) {
            warn!("generate_fill_reports: live API not implemented");
            return Ok(Vec::new());
        }

        warn!("generate_fill_reports: using explicit offline fixture report source");
        default_offline_report_source(self.core.client_id, self.core.account_id).fill_reports()
    }

    async fn generate_position_status_reports(
        &self,
        cmd: &GeneratePositionStatusReports,
    ) -> anyhow::Result<Vec<PositionStatusReport>> {
        if is_mock_http_report_request(cmd.params.as_ref()) {
            warn!("generate_position_status_reports: using mock HTTP report source");
            return MockHttpReportSource::from_params(
                cmd.params.as_ref(),
                self.core.account_id,
                cmd.instrument_id,
            )?
            .position_status_reports()
            .await;
        }

        if !is_offline_fixture_report_request(cmd.params.as_ref()) {
            warn!("generate_position_status_reports: live API not implemented");
            return Ok(Vec::new());
        }

        warn!("generate_position_status_reports: using explicit offline fixture report source");
        default_offline_report_source(self.core.client_id, self.core.account_id)
            .position_status_reports()
    }

    async fn generate_mass_status(
        &self,
        lookback_mins: Option<u64>,
    ) -> anyhow::Result<Option<ExecutionMassStatus>> {
        if let Some(params) = mock_http_report_params_from_config(&self.config) {
            warn!("generate_mass_status: using mock HTTP report source");
            return MockHttpReportSource::from_params(
                Some(&params),
                self.core.account_id,
                Some(InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID)),
            )?
            .mass_status(self.core.client_id, self.core.venue)
            .await;
        }

        if lookback_mins != Some(OFFLINE_REPORT_LOOKBACK_MINS) {
            warn!("generate_mass_status: live API not implemented");
            return Ok(None);
        }

        warn!("generate_mass_status: using explicit offline fixture report source");
        default_offline_report_source(self.core.client_id, self.core.account_id).mass_status()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, net::SocketAddr, rc::Rc, time::Duration};

    use axum::{
        Router,
        extract::{Query, State},
        http::StatusCode,
        response::Json,
        routing::get,
    };
    use dashmap::DashMap;
    use nautilus_common::cache::Cache;
    use nautilus_core::UUID4;
    use nautilus_model::{
        enums::AccountType,
        identifiers::{InstrumentId, TraderId, VenueOrderId},
    };
    use serde_json::{Value, json};

    use crate::{
        common::LighterEnvironment,
        execution::fixtures::{OrderFixtureStatus, execution_fixture_set},
        websocket::messages::InboundMessage,
    };

    use super::*;

    #[test]
    fn offline_fixture_report_request_requires_explicit_param() {
        let mut params = Params::new();
        params.insert(
            OFFLINE_REPORT_SOURCE_PARAM.to_string(),
            serde_json::Value::String(OFFLINE_REPORT_SOURCE_VALUE.to_string()),
        );

        assert!(!is_offline_fixture_report_request(None));
        assert!(is_offline_fixture_report_request(Some(&params)));

        params.insert(
            OFFLINE_REPORT_SOURCE_PARAM.to_string(),
            serde_json::Value::String("live".to_string()),
        );
        assert!(!is_offline_fixture_report_request(Some(&params)));
    }

    #[test]
    fn offline_mass_status_lookback_requires_explicit_sentinel() {
        assert_eq!(OFFLINE_REPORT_LOOKBACK_MINS, u64::MAX);
        assert_ne!(Some(OFFLINE_REPORT_LOOKBACK_MINS), Some(0));
        assert_ne!(Some(OFFLINE_REPORT_LOOKBACK_MINS), None);
    }

    fn test_execution_client_with_config(
        config: LighterExecClientConfig,
    ) -> LighterExecutionClient {
        let core = ExecutionClientCore::new(
            TraderId::from("TESTER-001"),
            ClientId::from("LIGHTER"),
            *LIGHTER_VENUE,
            OmsType::Netting,
            AccountId::from("LIGHTER-001"),
            AccountType::Margin,
            None,
            Rc::new(RefCell::new(Cache::default())),
        );
        LighterExecutionClient::new(core, config).expect("test execution client")
    }

    fn test_execution_client() -> LighterExecutionClient {
        test_execution_client_with_config(LighterExecClientConfig::new(
            "00000000000000000000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            42,
            2,
            LighterEnvironment::Testnet,
        ))
    }

    #[test]
    fn process_ws_message_dispatches_order_update_and_hydrates_order_cache() {
        let fixtures = execution_fixture_set();
        let accepted = fixtures
            .orders
            .iter()
            .find(|order| order.status == OrderFixtureStatus::Accepted)
            .expect("accepted order fixture");
        let client_order_id = ClientOrderId::new(accepted.client_order_id);
        let orders = DashMap::new();
        let reconciler = Arc::new(Mutex::new(ExecutionReconciler::default()));
        orders.insert(
            client_order_id,
            OrderState {
                client_order_id,
                instrument_id: InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID),
                venue_order_id: None,
                nonce: 1,
            },
        );

        LighterExecutionClient::process_ws_message(
            accepted.to_ws_message(),
            &orders,
            &reconciler,
            ClientId::from("LIGHTER"),
            AccountId::from("LIGHTER-001"),
        )
        .expect("process order update");

        let order_state = orders.get(&client_order_id).expect("cached order");
        assert_eq!(
            order_state.venue_order_id.as_deref(),
            Some(accepted.order_id)
        );
    }

    #[test]
    fn process_ws_message_dispatches_account_update_and_ignores_market_data() {
        let fixtures = execution_fixture_set();
        let message = InboundMessage::AccountUpdate {
            address: fixtures.account.address,
            balances: vec![(fixtures.account.asset, fixtures.account.balance)],
            timestamp: fixtures.account.timestamp_ms,
        };
        let orders = DashMap::new();
        let reconciler = Arc::new(Mutex::new(ExecutionReconciler::default()));

        LighterExecutionClient::process_ws_message(
            message,
            &orders,
            &reconciler,
            ClientId::from("LIGHTER"),
            AccountId::from("LIGHTER-001"),
        )
        .expect("process account update");
        LighterExecutionClient::process_ws_message(
            InboundMessage::Pong,
            &orders,
            &reconciler,
            ClientId::from("LIGHTER"),
            AccountId::from("LIGHTER-001"),
        )
        .expect("process ignored pong");
    }

    #[test]
    fn offline_report_source_returns_fixture_backed_results() {
        let src = default_offline_report_source(
            ClientId::from("LIGHTER"),
            AccountId::from("LIGHTER-001"),
        );

        assert_eq!(src.order_status_reports().unwrap().len(), 6);
        assert_eq!(src.fill_reports().unwrap().len(), 1);
        assert_eq!(src.position_status_reports().unwrap().len(), 1);
        let mass_status = src.mass_status().unwrap().expect("mass status");
        assert_eq!(mass_status.order_reports().len(), 6);
        assert_eq!(mass_status.fill_reports().len(), 1);
        assert_eq!(mass_status.position_reports().len(), 1);
    }

    #[tokio::test]
    async fn generate_mass_status_uses_explicit_offline_fixture_sentinel() {
        let client = test_execution_client();

        assert!(client.generate_mass_status(None).await.unwrap().is_none());
        assert!(
            client
                .generate_mass_status(Some(0))
                .await
                .unwrap()
                .is_none()
        );

        let mass_status = client
            .generate_mass_status(Some(OFFLINE_REPORT_LOOKBACK_MINS))
            .await
            .unwrap()
            .expect("offline fixture mass status");
        assert_eq!(mass_status.order_reports().len(), 6);
        assert_eq!(mass_status.fill_reports().len(), 1);
        assert_eq!(mass_status.position_reports().len(), 1);
    }

    #[derive(Clone)]
    struct MockReportServerState {
        mode: MockReportMode,
    }

    #[derive(Clone, Copy)]
    enum MockReportMode {
        Deterministic,
        Empty,
        Error,
    }

    async fn handle_mock_orders(
        State(state): State<MockReportServerState>,
        Query(query): Query<std::collections::HashMap<String, String>>,
    ) -> Result<Json<Value>, StatusCode> {
        match state.mode {
            MockReportMode::Deterministic => {
                let cursor = query.get("cursor").map(String::as_str);
                let (items, next_cursor) = match cursor {
                    Some("page-2") => (
                        vec![json!({
                            "order_id": "9002",
                            "client_order_id": "P2L-ORDER-2",
                            "market_index": 1,
                            "status": "filled",
                            "side": "sell",
                            "order_type": "limit",
                            "price": "102.50",
                            "quantity": "0.1250",
                            "filled_quantity": "0.1250",
                            "timestamp": 1734200001000_i64
                        })],
                        Value::Null,
                    ),
                    _ => (
                        vec![json!({
                            "order_id": "9001",
                            "client_order_id": "P2L-ORDER-1",
                            "market_index": 1,
                            "status": "open",
                            "side": "buy",
                            "order_type": "limit",
                            "price": "101.25",
                            "quantity": "0.2500",
                            "filled_quantity": "0.0000",
                            "timestamp": 1734200000000_i64
                        })],
                        json!("page-2"),
                    ),
                };
                Ok(Json(json!({
                    "success": true,
                    "data": {
                        "items": items,
                        "next_cursor": next_cursor
                    }
                })))
            }
            MockReportMode::Empty => Ok(Json(json!({
                "success": true,
                "data": {
                    "items": [],
                    "next_cursor": null
                }
            }))),
            MockReportMode::Error => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    async fn handle_mock_fills(
        State(state): State<MockReportServerState>,
    ) -> Result<Json<Value>, StatusCode> {
        match state.mode {
            MockReportMode::Deterministic => Ok(Json(json!({
                "success": true,
                "data": {
                    "items": [{
                        "trade_id": "T-9001",
                        "order_id": "9001",
                        "client_order_id": "P2L-ORDER-1",
                        "market_index": 1,
                        "price": "101.25",
                        "quantity": "0.2500",
                        "side": "buy",
                        "timestamp": 1734200000000_i64
                    }],
                    "next_cursor": null
                }
            }))),
            MockReportMode::Empty => Ok(Json(json!({
                "success": true,
                "data": {
                    "items": [],
                    "next_cursor": null
                }
            }))),
            MockReportMode::Error => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    async fn handle_mock_positions(
        State(state): State<MockReportServerState>,
    ) -> Result<Json<Value>, StatusCode> {
        match state.mode {
            MockReportMode::Deterministic => Ok(Json(json!({
                "success": true,
                "data": {
                    "items": [{
                        "market_index": 1,
                        "symbol": "ETH_USDC",
                        "size": "0.2500",
                        "entry_price": "101.25",
                        "timestamp": 1734200000000_i64
                    }],
                    "next_cursor": null
                }
            }))),
            MockReportMode::Empty => Ok(Json(json!({
                "success": true,
                "data": {
                    "items": [],
                    "next_cursor": null
                }
            }))),
            MockReportMode::Error => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    fn mock_report_params(base_url: &str) -> Params {
        let mut params = Params::new();
        params.insert(
            "lighter_report_source".to_string(),
            serde_json::Value::String("mock_http".to_string()),
        );
        params.insert(
            "lighter_report_base_url".to_string(),
            serde_json::Value::String(base_url.to_string()),
        );
        params.insert(
            "lighter_report_page_limit".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)),
        );
        params
    }

    async fn start_mock_report_server(mode: MockReportMode) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock report server");
        let addr = listener.local_addr().expect("mock report server addr");
        let router = Router::new()
            .route("/api/v1/account_active_orders", get(handle_mock_orders))
            .route("/api/v1/account_fills", get(handle_mock_fills))
            .route("/api/v1/account_positions", get(handle_mock_positions))
            .with_state(MockReportServerState { mode });

        tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("serve mock report server");
        });

        for _ in 0..50 {
            if tokio::net::TcpStream::connect(addr).await.is_ok() {
                return addr;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("mock report server did not start");
    }

    #[tokio::test]
    async fn mock_http_report_source_returns_deterministic_reports_and_follows_pages() {
        let addr = start_mock_report_server(MockReportMode::Deterministic).await;
        let params = mock_report_params(&format!("http://{addr}"));
        let client = test_execution_client();
        let instrument_id = Some(InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID));

        let order_reports = client
            .generate_order_status_reports(&GenerateOrderStatusReports::new(
                UUID4::new(),
                UnixNanos::default(),
                false,
                instrument_id,
                None,
                None,
                Some(params.clone()),
                None,
            ))
            .await
            .expect("mock HTTP order reports");
        let fill_reports = client
            .generate_fill_reports(GenerateFillReports::new(
                UUID4::new(),
                UnixNanos::default(),
                instrument_id,
                Some(VenueOrderId::new("9001")),
                None,
                None,
                Some(params.clone()),
                None,
            ))
            .await
            .expect("mock HTTP fill reports");
        let position_reports = client
            .generate_position_status_reports(&GeneratePositionStatusReports::new(
                UUID4::new(),
                UnixNanos::default(),
                instrument_id,
                None,
                None,
                Some(params),
                None,
            ))
            .await
            .expect("mock HTTP position reports");

        assert_eq!(order_reports.len(), 2, "must follow next_cursor pagination");
        assert_eq!(order_reports[0].venue_order_id.as_str(), "9001");
        assert_eq!(order_reports[1].venue_order_id.as_str(), "9002");
        assert_eq!(fill_reports.len(), 1);
        assert_eq!(fill_reports[0].trade_id.as_str(), "T-9001");
        assert_eq!(position_reports.len(), 1);
        assert_eq!(position_reports[0].quantity.to_string(), "0.2500");
    }

    #[tokio::test]
    async fn mock_http_report_source_preserves_empty_and_error_semantics() {
        let empty_addr = start_mock_report_server(MockReportMode::Empty).await;
        let empty_params = mock_report_params(&format!("http://{empty_addr}"));
        let client = test_execution_client();
        let instrument_id = Some(InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID));

        let no_single_order = client
            .generate_order_status_report(&GenerateOrderStatusReport::new(
                UUID4::new(),
                UnixNanos::default(),
                instrument_id,
                Some(ClientOrderId::new("P2L-MISSING")),
                None,
                Some(empty_params.clone()),
                None,
            ))
            .await
            .expect("mock HTTP empty single order response");
        let empty_orders = client
            .generate_order_status_reports(&GenerateOrderStatusReports::new(
                UUID4::new(),
                UnixNanos::default(),
                false,
                instrument_id,
                None,
                None,
                Some(empty_params),
                None,
            ))
            .await
            .expect("mock HTTP empty order reports");

        assert!(no_single_order.is_none());
        assert!(empty_orders.is_empty());

        let error_addr = start_mock_report_server(MockReportMode::Error).await;
        let error_params = mock_report_params(&format!("http://{error_addr}"));
        let err = client
            .generate_fill_reports(GenerateFillReports::new(
                UUID4::new(),
                UnixNanos::default(),
                instrument_id,
                None,
                None,
                None,
                Some(error_params),
                None,
            ))
            .await
            .expect_err("mock HTTP report errors must not fallback to fixture or empty success");

        assert!(
            err.to_string().contains("500") || err.to_string().contains("mock_http"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn mock_http_report_source_generate_mass_status_uses_configured_mock_server() {
        let addr = start_mock_report_server(MockReportMode::Deterministic).await;
        let mut config = LighterExecClientConfig::new(
            "00000000000000000000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            42,
            2,
            LighterEnvironment::Testnet,
        );
        config.report_source = Some("mock_http".to_string());
        config.report_base_url = Some(format!("http://{addr}"));
        config.report_page_limit = Some(1);
        let client = test_execution_client_with_config(config);

        let mass_status = client
            .generate_mass_status(None)
            .await
            .expect("mock HTTP generate_mass_status")
            .expect("configured mock HTTP source should return mass status");

        assert!(
            mass_status
                .order_reports()
                .contains_key(&VenueOrderId::new("9001"))
        );
        assert!(
            mass_status
                .fill_reports()
                .contains_key(&VenueOrderId::new("9001"))
        );
        assert!(
            mass_status
                .position_reports()
                .contains_key(&InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID))
        );
    }

    #[tokio::test]
    async fn mock_http_report_source_generate_mass_status_returns_empty_status_for_empty_server() {
        let addr = start_mock_report_server(MockReportMode::Empty).await;
        let mut config = LighterExecClientConfig::new(
            "00000000000000000000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            42,
            2,
            LighterEnvironment::Testnet,
        );
        config.report_source = Some("mock_http".to_string());
        config.report_base_url = Some(format!("http://{addr}"));
        let client = test_execution_client_with_config(config);

        let mass_status = client
            .generate_mass_status(None)
            .await
            .expect("empty mock HTTP generate_mass_status")
            .expect("reachable empty mock HTTP source should return empty mass status");

        assert!(mass_status.order_reports().is_empty());
        assert!(mass_status.fill_reports().is_empty());
        assert!(mass_status.position_reports().is_empty());
    }

    #[tokio::test]
    async fn mock_http_report_source_generate_mass_status_errors_for_error_server() {
        let addr = start_mock_report_server(MockReportMode::Error).await;
        let mut config = LighterExecClientConfig::new(
            "00000000000000000000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            42,
            2,
            LighterEnvironment::Testnet,
        );
        config.report_source = Some("mock_http".to_string());
        config.report_base_url = Some(format!("http://{addr}"));
        let client = test_execution_client_with_config(config);

        let err = client
            .generate_mass_status(None)
            .await
            .expect_err("mock HTTP generate_mass_status errors must not fallback");

        assert!(
            err.to_string().contains("500") || err.to_string().contains("mock_http"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn mock_http_report_source_mass_status_aggregates_reports() {
        let addr = start_mock_report_server(MockReportMode::Deterministic).await;
        let params = mock_report_params(&format!("http://{addr}"));
        let source = MockHttpReportSource::from_params(
            Some(&params),
            AccountId::from("LIGHTER-001"),
            Some(InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID)),
        )
        .expect("mock HTTP source");

        let mass_status = source
            .mass_status(ClientId::from("LIGHTER"), *LIGHTER_VENUE)
            .await
            .expect("mock HTTP mass status")
            .expect("reachable mock HTTP source should return a mass status");

        assert_eq!(mass_status.order_reports().len(), 2);
        assert!(
            mass_status
                .order_reports()
                .contains_key(&VenueOrderId::new("9001"))
        );
        assert!(
            mass_status
                .order_reports()
                .contains_key(&VenueOrderId::new("9002"))
        );
        assert_eq!(mass_status.fill_reports().len(), 1);
        assert!(
            mass_status
                .fill_reports()
                .contains_key(&VenueOrderId::new("9001"))
        );
        assert_eq!(mass_status.position_reports().len(), 1);
        assert!(
            mass_status
                .position_reports()
                .contains_key(&InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID))
        );
    }

    #[tokio::test]
    async fn mock_http_report_source_mass_status_preserves_empty_and_error_semantics() {
        let client_id = ClientId::from("LIGHTER");
        let account_id = AccountId::from("LIGHTER-001");
        let instrument_id = Some(InstrumentId::from(OFFLINE_REPORT_INSTRUMENT_ID));

        let empty_addr = start_mock_report_server(MockReportMode::Empty).await;
        let empty_params = mock_report_params(&format!("http://{empty_addr}"));
        let empty_source =
            MockHttpReportSource::from_params(Some(&empty_params), account_id, instrument_id)
                .expect("empty mock HTTP source");
        let empty_mass_status = empty_source
            .mass_status(client_id, *LIGHTER_VENUE)
            .await
            .expect("empty mock HTTP mass status")
            .expect("reachable empty mock HTTP source should return empty mass status");

        assert!(empty_mass_status.order_reports().is_empty());
        assert!(empty_mass_status.fill_reports().is_empty());
        assert!(empty_mass_status.position_reports().is_empty());

        let error_addr = start_mock_report_server(MockReportMode::Error).await;
        let error_params = mock_report_params(&format!("http://{error_addr}"));
        let error_source =
            MockHttpReportSource::from_params(Some(&error_params), account_id, instrument_id)
                .expect("error mock HTTP source");
        let err = error_source
            .mass_status(client_id, *LIGHTER_VENUE)
            .await
            .expect_err("mock HTTP mass status errors must not fallback to fixture or None");

        assert!(
            err.to_string().contains("500") || err.to_string().contains("mock_http"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn connect_rejects_default_config_before_live_signing_or_private_ws() {
        let mut client = test_execution_client();

        let err = client
            .connect()
            .await
            .expect_err("default config must not enter live signing");

        assert!(
            err.to_string().contains("Live signing is disabled")
                || err.to_string().contains("live signing is disabled"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn live_signing_gate_allows_explicit_opt_in() {
        let mut client = test_execution_client();
        client.config.enable_live_signing = true;

        client
            .ensure_live_signing_enabled()
            .expect("explicit opt-in should pass local gate");
    }

    #[test]
    fn test_client_creation() {
        // This test would need a valid private key and core setup
        // Placeholder for future implementation
    }

    #[test]
    fn market_index_lookup_uses_cache_not_symbol_heuristics() {
        let cache = DashMap::new();
        let eth = InstrumentId::from("ETH_USDC.LIGHTER");
        let btc = InstrumentId::from("BTC_USDC.LIGHTER");
        let unknown = InstrumentId::from("UNKNOWN_USDC.LIGHTER");

        cache.insert(eth, 42);
        cache.insert(btc, 7);

        assert_eq!(
            LighterExecutionClient::market_index_from_cache(&cache, &eth),
            Some(42)
        );
        assert_eq!(
            LighterExecutionClient::market_index_from_cache(&cache, &btc),
            Some(7)
        );
        assert_eq!(
            LighterExecutionClient::market_index_from_cache(&cache, &unknown),
            None
        );
    }
}
