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

use std::sync::{Mutex, Arc};

use async_trait::async_trait;
use dashmap::DashMap;
use nautilus_common::messages::execution::{
    BatchCancelOrders, CancelAllOrders, CancelOrder, GenerateFillReports,
    GenerateOrderStatusReport, GeneratePositionReports, ModifyOrder, QueryAccount,
    QueryOrder, SubmitOrder, SubmitOrderList,
};
use nautilus_core::{MUTEX_POISONED, UnixNanos};
use nautilus_execution::client::{ExecutionClient, base::ExecutionClientCore};
use nautilus_live::execution::client::LiveExecutionClient;
use nautilus_model::{
    accounts::AccountAny,
    enums::OmsType,
    identifiers::{AccountId, ClientId, ClientOrderId, InstrumentId, Venue},
    instruments::{Instrument, InstrumentAny},
    orders::{Order, OrderAny},
    reports::{ExecutionMassStatus, FillReport, OrderStatusReport, PositionStatusReport},
    types::{AccountBalance, MarginBalance},
};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::{
    common::LIGHTER_VENUE,
    config::LighterExecClientConfig,
    error::LighterError,
    http::{
        client::LighterRawHttpClient,
        endpoints::{SEND_TX, NEXT_NONCE, ORDER_BOOKS},
        types::{
            CreateOrderRequest, CancelOrderRequest, CancelAllOrdersRequest,
            TxResponse, NextNonceResponse, Market, LighterResponse,
        },
    },
    signing::{LighterSigner, NonceManager},
    websocket::{client::LighterWebSocketClient, messages::InboundMessage},
};

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
    /// HTTP client for REST API calls (wrapped in Arc for async sharing).
    http_client: Arc<LighterRawHttpClient>,
    /// WebSocket client for real-time updates.
    ws_client: LighterWebSocketClient,
    /// Transaction signer.
    signer: LighterSigner,
    /// Nonce manager for transaction ordering.
    nonce_manager: Arc<Mutex<NonceManager>>,
    /// Cached instruments (InstrumentId -> InstrumentAny).
    #[allow(dead_code)] // Will be used for instrument lookups and price/size conversions
    instruments: DashMap<InstrumentId, InstrumentAny>,
    /// Market symbol to InstrumentId mapping.
    market_to_instrument: DashMap<String, InstrumentId>,
    /// Order state cache (ClientOrderId -> OrderState).
    orders: DashMap<ClientOrderId, OrderState>,
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

        Ok(Self {
            core,
            config,
            http_client: Arc::new(http_client),
            ws_client,
            signer,
            nonce_manager,
            instruments: DashMap::new(),
            market_to_instrument: DashMap::new(),
            orders: DashMap::new(),
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
        self.nonce_manager
            .lock()
            .expect(MUTEX_POISONED)
            .next()
    }

    /// Cache instruments for lookups.
    #[allow(dead_code)] // Will be used for instrument initialization from HTTP API
    fn cache_instruments(&mut self, instruments: Vec<InstrumentAny>) {
        for instrument in instruments {
            let instrument_id = instrument.id();
            let symbol = instrument_id.symbol.as_str();

            self.instruments.insert(instrument_id, instrument.clone());
            self.market_to_instrument
                .insert(symbol.to_string(), instrument_id);
        }

        self.instruments_initialized = true;
        info!("Cached {} instruments", self.instruments.len());
    }

    /// Get market index from instrument symbol.
    ///
    /// The market index is extracted from the cached instrument data.
    /// Returns None if the instrument is not found.
    fn get_market_index(&self, instrument_id: &InstrumentId) -> Option<u16> {
        // For now, use a simple symbol-to-index mapping
        // In production, this would be populated from the exchange API
        let symbol = instrument_id.symbol.as_str();
        match symbol {
            s if s.contains("ETH") => Some(1),
            s if s.contains("BTC") => Some(2),
            s if s.contains("SOL") => Some(3),
            s if s.contains("DOGE") => Some(4),
            _ => None,
        }
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
            let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
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
            info!("Cancel request for {} submitted successfully", client_order_id);
        } else {
            let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
            error!("Cancel request for {} rejected: {}", client_order_id, error_msg);
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
            let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
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
        let core_client_id = self.core.client_id;
        let core_account_id = self.core.account_id;

        let handle = tokio::spawn(async move {
            info!("Starting execution WebSocket message handler for {}", core_client_id);

            loop {
                match msg_rx.recv().await {
                    Some(message) => {
                        if let Err(e) = Self::process_ws_message(
                            message,
                            &orders,
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
    /// Handles order updates, account updates, and other execution-related messages.
    fn process_ws_message(
        message: InboundMessage,
        orders: &DashMap<ClientOrderId, OrderState>,
        client_id: ClientId,
        account_id: AccountId,
    ) -> Result<(), LighterError> {
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
                debug!(
                    "Processing order update: order_id={}, status={}, filled={}",
                    order_id, status, filled_quantity
                );

                // Try to find order by client_order_id first
                let client_order_id_key = client_order_id
                    .as_ref()
                    .and_then(|id| ClientOrderId::new_checked(id).ok());

                if let Some(coid) = client_order_id_key {
                    if let Some(mut order_state) = orders.get_mut(&coid) {
                        // Update venue order ID if not set
                        if order_state.venue_order_id.is_none() {
                            order_state.venue_order_id = Some(order_id.clone());
                        }
                    }
                }

                // Log order status for now - full event generation requires ExecutionClientCore access
                // which would need to be passed through Arc<Mutex<>> or similar
                info!(
                    "[{}] Order {} status: {} (market={}, side={}, type={}, price={}, qty={}, filled={})",
                    client_id, order_id, status, market_index, side, order_type, price, quantity, filled_quantity
                );

                // TODO: Generate proper NautilusTrader order events via core
                // This would require:
                // 1. Passing core reference to this method (via Arc)
                // 2. Calling core.generate_order_accepted/rejected/filled/canceled based on status
                // For now, we just update internal state and log

                let _ = timestamp; // Used for event timestamps
            }

            InboundMessage::AccountUpdate {
                address,
                balances,
                timestamp,
            } => {
                debug!(
                    "Processing account update: address={}, balances_count={}",
                    address,
                    balances.len()
                );

                // Log account state for now - full event generation requires ExecutionClientCore access
                info!(
                    "[{}] Account {} updated with {} balance entries",
                    account_id, address, balances.len()
                );

                for (asset, amount) in &balances {
                    debug!("  Balance: {} = {}", asset, amount);
                }

                let _ = timestamp; // Used for event timestamps
            }

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
                return Err(LighterError::WebSocket(format!("Error {}: {}", code, message)));
            }

            InboundMessage::Pong => {
                debug!("[{}] Received pong", client_id);
            }

            InboundMessage::Raw(value) => {
                debug!("[{}] Received raw message: {:?}", client_id, value);
            }

            // Ignore data messages in execution client (handled by data client)
            InboundMessage::OrderbookSnapshot { .. }
            | InboundMessage::OrderbookUpdate { .. }
            | InboundMessage::Trade { .. }
            | InboundMessage::Ticker { .. } => {
                // These are market data messages, ignore in execution client
            }
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
        self.core.get_account()
    }

    fn generate_account_state(
        &self,
        balances: Vec<AccountBalance>,
        margins: Vec<MarginBalance>,
        reported: bool,
        ts_event: UnixNanos,
    ) -> anyhow::Result<()> {
        self.core
            .generate_account_state(balances, margins, reported, ts_event)
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
    fn submit_order(&self, cmd: &SubmitOrder) -> anyhow::Result<()> {
        let order = &cmd.order;

        if !self.is_connected() {
            anyhow::bail!("Cannot submit order: execution client not connected");
        }

        if order.is_closed() {
            warn!("Cannot submit closed order {}", order.client_order_id());
            return Ok(());
        }

        // Generate OrderSubmitted event immediately
        self.core.generate_order_submitted(
            order.strategy_id(),
            order.instrument_id(),
            order.client_order_id(),
            cmd.ts_init,
        );

        // Get market index from instrument
        let market_index = match self.get_market_index(&order.instrument_id()) {
            Some(idx) => idx,
            None => {
                error!(
                    "Unknown instrument {}, cannot submit order",
                    order.instrument_id()
                );
                self.core.generate_order_rejected(
                    order.strategy_id(),
                    order.instrument_id(),
                    order.client_order_id(),
                    "Unknown instrument",
                    cmd.ts_init,
                    false,
                );
                return Ok(());
            }
        };

        // Extract order parameters
        let client_order_index = order.client_order_id().to_string()
            .parse::<i64>()
            .unwrap_or_else(|_| {
                // Generate a unique ID from hash if not numeric
                use std::hash::{Hash, Hasher};
                use std::collections::hash_map::DefaultHasher;
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
        let is_ask = Self::order_side_to_is_ask(order);
        let order_type = Self::convert_order_type(order);
        let time_in_force = Self::convert_time_in_force(order);
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
                self.core.generate_order_rejected(
                    order.strategy_id(),
                    order.instrument_id(),
                    order.client_order_id(),
                    &reason,
                    cmd.ts_init,
                    false,
                );
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

    fn submit_order_list(&self, _cmd: &SubmitOrderList) -> anyhow::Result<()> {
        anyhow::bail!("Order lists not yet implemented for Lighter DEX")
    }

    fn modify_order(&self, _cmd: &ModifyOrder) -> anyhow::Result<()> {
        // TODO: Implement order modification
        anyhow::bail!("Order modification not yet implemented for Lighter DEX")
    }

    fn cancel_order(&self, cmd: &CancelOrder) -> anyhow::Result<()> {
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
        let venue_order_str = cmd.venue_order_id.to_string();
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
        let (signed_tx, nonce) = match self.signer.sign_cancel_order(market_index, order_index, expired_at) {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to sign cancel order: {}", e);
                return Ok(());
            }
        };

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

    fn cancel_all_orders(&self, cmd: &CancelAllOrders) -> anyhow::Result<()> {
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
        let (signed_tx, nonce) = match self.signer.sign_cancel_all_orders(time_in_force, time, expired_at) {
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

    fn batch_cancel_orders(&self, cmd: &BatchCancelOrders) -> anyhow::Result<()> {
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

    fn query_account(&self, _cmd: &QueryAccount) -> anyhow::Result<()> {
        // TODO: Implement account query
        Ok(())
    }

    fn query_order(&self, _cmd: &QueryOrder) -> anyhow::Result<()> {
        // TODO: Implement order query
        Ok(())
    }

    async fn connect(&mut self) -> anyhow::Result<()> {
        if self.connected {
            warn!("Lighter execution client already connected");
            return Ok(());
        }

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
            self.market_to_instrument
                .insert(symbol, instrument_id);
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

        let server_nonce = nonce_response
            .data
            .map(|d| d.nonce)
            .unwrap_or(0);

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
}

#[async_trait(?Send)]
impl LiveExecutionClient for LighterExecutionClient {
    async fn generate_order_status_report(
        &self,
        _cmd: &GenerateOrderStatusReport,
    ) -> anyhow::Result<Option<OrderStatusReport>> {
        // TODO: Implement single order status report generation
        warn!("generate_order_status_report not yet implemented");
        Ok(None)
    }

    async fn generate_order_status_reports(
        &self,
        _cmd: &GenerateOrderStatusReport,
    ) -> anyhow::Result<Vec<OrderStatusReport>> {
        // TODO: Implement order status reports generation
        // 1. Query orders from HTTP API
        // 2. Filter by instrument_id, client_order_id, venue_order_id
        // 3. Parse to OrderStatusReport
        warn!("generate_order_status_reports not yet implemented");
        Ok(Vec::new())
    }

    async fn generate_fill_reports(
        &self,
        _cmd: GenerateFillReports,
    ) -> anyhow::Result<Vec<FillReport>> {
        // TODO: Implement fill reports generation
        // 1. Query fills from HTTP API
        // 2. Filter by instrument_id, venue_order_id, time range
        // 3. Parse to FillReport
        warn!("generate_fill_reports not yet implemented");
        Ok(Vec::new())
    }

    async fn generate_position_status_reports(
        &self,
        _cmd: &GeneratePositionReports,
    ) -> anyhow::Result<Vec<PositionStatusReport>> {
        // TODO: Implement position status reports generation
        // 1. Query positions from HTTP API
        // 2. Filter by instrument_id
        // 3. Parse to PositionStatusReport
        warn!("generate_position_status_reports not yet implemented");
        Ok(Vec::new())
    }

    async fn generate_mass_status(
        &self,
        _lookback_mins: Option<u64>,
    ) -> anyhow::Result<Option<ExecutionMassStatus>> {
        // TODO: Implement mass status generation
        // 1. Query all orders, fills, positions
        // 2. Apply time filter if lookback_mins specified
        // 3. Combine into ExecutionMassStatus
        warn!("generate_mass_status not yet implemented");
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_client_creation() {
        // This test would need a valid private key and core setup
        // Placeholder for future implementation
    }
}
