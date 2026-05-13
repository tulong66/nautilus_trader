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

//! WebSocket client implementation for the Lighter DEX adapter.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use serde_json::Value;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::{
    common::{LighterEnvironment, build_private_ws_url, build_public_ws_url},
    error::LighterError,
    websocket::{
        messages::{InboundMessage, OutboundMessage, SubscriptionType},
        transport::{
            TungsteniteWebSocketConnector, WebSocketTransport, WebSocketTransportConnector,
        },
    },
};

/// Type alias for WebSocket result.
pub type LighterWsResult<T> = Result<T, LighterError>;

/// Default heartbeat interval in seconds.
const DEFAULT_HEARTBEAT_SECS: u64 = 20;

/// Default reconnect delay in milliseconds.
const DEFAULT_RECONNECT_DELAY_MS: u64 = 1000;

/// Maximum reconnect delay in milliseconds.
const MAX_RECONNECT_DELAY_MS: u64 = 30000;

enum WebSocketCommand {
    Send(OutboundMessage),
    Close,
}

/// WebSocket client for Lighter DEX.
///
/// Provides real-time market data and account updates via WebSocket connections.
/// Supports automatic reconnection with exponential backoff.
pub struct LighterWebSocketClient {
    /// WebSocket URL.
    url: String,
    /// Environment (testnet/mainnet).
    environment: LighterEnvironment,
    /// Whether this client requires authentication.
    requires_auth: bool,
    /// Authentication token (if authenticated).
    auth_token: Arc<RwLock<Option<String>>>,
    /// Heartbeat interval in seconds.
    heartbeat_interval: u64,
    /// Active subscriptions.
    subscriptions: Arc<RwLock<Vec<String>>>,
    /// Indicates if client is running.
    is_running: Arc<AtomicBool>,
    /// Indicates if client is authenticated (for private connections).
    is_authenticated: Arc<AtomicBool>,
    /// Message sender.
    msg_tx: Arc<RwLock<Option<mpsc::UnboundedSender<InboundMessage>>>>,
    /// Command sender for the connection loop.
    command_tx: Arc<RwLock<Option<mpsc::UnboundedSender<WebSocketCommand>>>>,
    /// Transport connector.
    transport_connector: Arc<dyn WebSocketTransportConnector>,
    /// Background task handle.
    task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl std::fmt::Debug for LighterWebSocketClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LighterWebSocketClient")
            .field("url", &self.url)
            .field("environment", &self.environment)
            .field("requires_auth", &self.requires_auth)
            .field("is_running", &self.is_running.load(Ordering::Relaxed))
            .field(
                "is_authenticated",
                &self.is_authenticated.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl LighterWebSocketClient {
    /// Create a new public WebSocket client.
    ///
    /// # Arguments
    ///
    /// * `environment` - The network environment (testnet/mainnet).
    /// * `url` - Optional custom WebSocket URL (uses default if None).
    /// * `heartbeat` - Optional heartbeat interval in seconds.
    #[must_use]
    pub fn new_public(
        environment: LighterEnvironment,
        url: Option<String>,
        heartbeat: Option<u64>,
    ) -> Self {
        let url = url.unwrap_or_else(|| build_public_ws_url(environment));

        Self {
            url,
            environment,
            requires_auth: false,
            auth_token: Arc::new(RwLock::new(None)),
            heartbeat_interval: heartbeat.unwrap_or(DEFAULT_HEARTBEAT_SECS),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            is_authenticated: Arc::new(AtomicBool::new(false)),
            msg_tx: Arc::new(RwLock::new(None)),
            command_tx: Arc::new(RwLock::new(None)),
            transport_connector: Arc::new(TungsteniteWebSocketConnector),
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new private WebSocket client.
    ///
    /// # Arguments
    ///
    /// * `environment` - The network environment (testnet/mainnet).
    /// * `auth_token` - Authentication token for private channels.
    /// * `url` - Optional custom WebSocket URL (uses default if None).
    /// * `heartbeat` - Optional heartbeat interval in seconds.
    #[must_use]
    pub fn new_private(
        environment: LighterEnvironment,
        auth_token: String,
        url: Option<String>,
        heartbeat: Option<u64>,
    ) -> Self {
        let url = url.unwrap_or_else(|| build_private_ws_url(environment));

        Self {
            url,
            environment,
            requires_auth: true,
            auth_token: Arc::new(RwLock::new(Some(auth_token))),
            heartbeat_interval: heartbeat.unwrap_or(DEFAULT_HEARTBEAT_SECS),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            is_authenticated: Arc::new(AtomicBool::new(false)),
            msg_tx: Arc::new(RwLock::new(None)),
            command_tx: Arc::new(RwLock::new(None)),
            transport_connector: Arc::new(TungsteniteWebSocketConnector),
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    #[cfg(test)]
    #[must_use]
    fn new_private_with_transport_connector(
        environment: LighterEnvironment,
        auth_token: String,
        url: String,
        heartbeat: Option<u64>,
        transport_connector: Arc<dyn WebSocketTransportConnector>,
    ) -> Self {
        Self {
            url,
            environment,
            requires_auth: true,
            auth_token: Arc::new(RwLock::new(Some(auth_token))),
            heartbeat_interval: heartbeat.unwrap_or(DEFAULT_HEARTBEAT_SECS),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            is_authenticated: Arc::new(AtomicBool::new(false)),
            msg_tx: Arc::new(RwLock::new(None)),
            command_tx: Arc::new(RwLock::new(None)),
            transport_connector,
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    fn resubscribe_messages(channels: &[String]) -> Vec<OutboundMessage> {
        channels.iter().map(OutboundMessage::subscribe).collect()
    }

    /// Connect to the WebSocket endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or authentication fails (for private clients).
    pub async fn connect(&mut self) -> LighterWsResult<mpsc::UnboundedReceiver<InboundMessage>> {
        if self.is_running.load(Ordering::Relaxed) {
            return Err(LighterError::WebSocket(
                "Client is already connected".to_string(),
            ));
        }

        info!("Connecting to Lighter WebSocket: {}", self.url);

        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        *self.msg_tx.write().await = Some(msg_tx.clone());
        *self.command_tx.write().await = Some(command_tx);

        let url = self.url.clone();
        let requires_auth = self.requires_auth;
        let auth_token = Arc::clone(&self.auth_token);
        let heartbeat_interval = self.heartbeat_interval;
        let subscriptions = Arc::clone(&self.subscriptions);
        let is_running = Arc::clone(&self.is_running);
        let is_authenticated = Arc::clone(&self.is_authenticated);
        let transport_connector = Arc::clone(&self.transport_connector);

        self.is_running.store(true, Ordering::Relaxed);

        let handle = tokio::spawn(async move {
            Self::run_connection_loop(
                url,
                requires_auth,
                auth_token,
                heartbeat_interval,
                subscriptions,
                is_running,
                is_authenticated,
                msg_tx,
                command_rx,
                transport_connector,
            )
            .await;
        });

        *self.task_handle.write().await = Some(handle);

        Ok(msg_rx)
    }

    /// Main connection loop with automatic reconnection.
    async fn run_connection_loop(
        url: String,
        requires_auth: bool,
        auth_token: Arc<RwLock<Option<String>>>,
        heartbeat_interval: u64,
        subscriptions: Arc<RwLock<Vec<String>>>,
        is_running: Arc<AtomicBool>,
        is_authenticated: Arc<AtomicBool>,
        msg_tx: mpsc::UnboundedSender<InboundMessage>,
        mut command_rx: mpsc::UnboundedReceiver<WebSocketCommand>,
        transport_connector: Arc<dyn WebSocketTransportConnector>,
    ) {
        let mut reconnect_delay = DEFAULT_RECONNECT_DELAY_MS;

        while is_running.load(Ordering::Relaxed) {
            match transport_connector.connect(&url).await {
                Ok(mut transport) => {
                    info!("WebSocket connected successfully");
                    reconnect_delay = DEFAULT_RECONNECT_DELAY_MS;

                    if let Err(e) =
                        Self::send_initial_auth(requires_auth, &auth_token, transport.as_mut())
                            .await
                    {
                        error!("Failed to authenticate WebSocket session: {}", e);
                        break;
                    }

                    let subs = subscriptions.read().await.clone();
                    if let Err(e) =
                        Self::send_subscription_channels(transport.as_mut(), &subs).await
                    {
                        error!("Failed to resubscribe WebSocket session: {}", e);
                        break;
                    }

                    Self::run_session_loop(
                        transport.as_mut(),
                        requires_auth,
                        heartbeat_interval,
                        &is_running,
                        &is_authenticated,
                        &msg_tx,
                        &mut command_rx,
                    )
                    .await;

                    let _ = transport.close().await;
                    info!("WebSocket connection closed");
                }
                Err(e) => {
                    error!("Failed to connect to WebSocket: {}", e);
                }
            }

            if is_running.load(Ordering::Relaxed) {
                warn!("Reconnecting in {} ms...", reconnect_delay);
                tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;
                reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY_MS);
            }
        }

        info!("WebSocket client stopped");
    }

    async fn send_initial_auth(
        requires_auth: bool,
        auth_token: &Arc<RwLock<Option<String>>>,
        transport: &mut dyn WebSocketTransport,
    ) -> LighterWsResult<()> {
        if !requires_auth {
            return Ok(());
        }

        let token = auth_token.read().await.clone().ok_or_else(|| {
            LighterError::Auth("Authentication required but no token provided".to_string())
        })?;
        Self::send_outbound_message(transport, OutboundMessage::Auth { token }, "auth").await?;
        debug!("Authentication message sent");
        Ok(())
    }

    async fn send_subscription_channels(
        transport: &mut dyn WebSocketTransport,
        channels: &[String],
    ) -> LighterWsResult<()> {
        for (channel, sub_msg) in channels.iter().zip(Self::resubscribe_messages(channels)) {
            Self::send_outbound_message(transport, sub_msg, "subscribe").await?;
            debug!("Sent subscription for channel: {}", channel);
        }
        Ok(())
    }

    async fn send_outbound_message(
        transport: &mut dyn WebSocketTransport,
        message: OutboundMessage,
        operation: &str,
    ) -> LighterWsResult<()> {
        let json = message.to_json().map_err(|e| {
            LighterError::WebSocket(format!("Failed to serialize {operation}: {e}"))
        })?;
        transport.send_text(json).await.map_err(|e| {
            LighterError::WebSocket(format!("Failed to send {operation} message: {e}"))
        })
    }

    async fn run_session_loop(
        transport: &mut dyn WebSocketTransport,
        requires_auth: bool,
        heartbeat_interval: u64,
        is_running: &Arc<AtomicBool>,
        is_authenticated: &Arc<AtomicBool>,
        msg_tx: &mpsc::UnboundedSender<InboundMessage>,
        command_rx: &mut mpsc::UnboundedReceiver<WebSocketCommand>,
    ) {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(heartbeat_interval.max(1)));
        heartbeat.tick().await;

        loop {
            if !is_running.load(Ordering::Relaxed) {
                break;
            }

            tokio::select! {
                command = command_rx.recv() => {
                    match command {
                        Some(WebSocketCommand::Send(message)) => {
                            if let Err(e) = Self::send_outbound_message(transport, message, "command").await {
                                error!("Failed to send WebSocket command: {}", e);
                                break;
                            }
                        }
                        Some(WebSocketCommand::Close) => {
                            is_running.store(false, Ordering::Relaxed);
                            break;
                        }
                        None => {
                            warn!("WebSocket command channel closed");
                            is_running.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if let Err(e) = Self::send_outbound_message(transport, OutboundMessage::Ping, "ping").await {
                        error!("Failed to send ping: {}", e);
                        break;
                    }
                    debug!("Sent ping");
                }
                frame = transport.recv_text() => {
                    match frame {
                        Ok(Some(text)) => {
                            match Self::parse_message(&text, requires_auth, is_authenticated) {
                                Ok(parsed_msg) => {
                                    if msg_tx.send(parsed_msg).is_err() {
                                        warn!("Message receiver dropped, stopping client");
                                        is_running.store(false, Ordering::Relaxed);
                                        break;
                                    }
                                }
                                Err(e) => warn!("Failed to parse message: {}", e),
                            }
                        }
                        Ok(None) => {
                            info!("WebSocket transport closed by peer");
                            break;
                        }
                        Err(e) => {
                            error!("WebSocket transport receive error: {}", e);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Parse an incoming text message into an `InboundMessage`.
    fn parse_message(
        text: &str,
        requires_auth: bool,
        is_authenticated: &Arc<AtomicBool>,
    ) -> LighterWsResult<InboundMessage> {
        let value: Value = serde_json::from_str(text)
            .map_err(|e| LighterError::Parse(format!("Invalid JSON: {}", e)))?;

        // Check for authentication success first (for private connections)
        if requires_auth && !is_authenticated.load(Ordering::Relaxed) {
            if let Some(msg_type) = value.get("type").and_then(|v| v.as_str()) {
                if msg_type == "auth_success" || msg_type == "authenticated" {
                    is_authenticated.store(true, Ordering::Relaxed);
                    debug!("WebSocket authenticated successfully");
                    return Ok(InboundMessage::AuthSuccess);
                }
            }
        }

        // Check for top-level error field (some APIs use this format)
        if let Some(error) = value.get("error") {
            let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
            let message = error
                .get("message")
                .or_else(|| error.get("msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            return Ok(InboundMessage::Error { code, message });
        }

        // Use the enhanced parsing from InboundMessage
        InboundMessage::parse(&value)
    }

    /// Subscribe to channels.
    ///
    /// # Errors
    ///
    /// Returns an error if subscription fails or client is not connected.
    pub async fn subscribe(&self, subscriptions: Vec<SubscriptionType>) -> LighterWsResult<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err(LighterError::WebSocket(
                "Client is not connected".to_string(),
            ));
        }

        // Check authentication for private channels
        if self.requires_auth && !self.is_authenticated.load(Ordering::Relaxed) {
            for sub in &subscriptions {
                if sub.requires_auth() {
                    return Err(LighterError::Auth(
                        "Authentication required for private channels".to_string(),
                    ));
                }
            }
        }

        let channels: Vec<String> = subscriptions.iter().map(|s| s.to_channel()).collect();
        let command_tx = self.command_tx.read().await.clone().ok_or_else(|| {
            LighterError::WebSocket("WebSocket command channel not ready".to_string())
        })?;

        for channel in &channels {
            command_tx
                .send(WebSocketCommand::Send(OutboundMessage::subscribe(channel)))
                .map_err(|_| {
                    LighterError::WebSocket("WebSocket command channel closed".to_string())
                })?;
            debug!("Queued subscription for channel: {}", channel);
        }

        self.subscriptions.write().await.extend(channels.clone());
        debug!("Subscribed to {} channels", channels.len());
        Ok(())
    }

    /// Unsubscribe from channels.
    ///
    /// # Errors
    ///
    /// Returns an error if unsubscription fails or client is not connected.
    pub async fn unsubscribe(&self, subscriptions: Vec<SubscriptionType>) -> LighterWsResult<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err(LighterError::WebSocket(
                "Client is not connected".to_string(),
            ));
        }

        let channels: Vec<String> = subscriptions.iter().map(|s| s.to_channel()).collect();

        if let Some(command_tx) = self.command_tx.read().await.clone() {
            for channel in &channels {
                command_tx
                    .send(WebSocketCommand::Send(OutboundMessage::unsubscribe(
                        channel,
                    )))
                    .map_err(|_| {
                        LighterError::WebSocket("WebSocket command channel closed".to_string())
                    })?;
                debug!("Queued unsubscription for channel: {}", channel);
            }
        }

        // Remove from subscriptions list
        let mut subs = self.subscriptions.write().await;
        subs.retain(|s| !channels.contains(s));

        debug!("Unsubscribed from {} channels", channels.len());

        Ok(())
    }

    /// Close the WebSocket connection.
    ///
    /// # Errors
    ///
    /// Returns an error if closing fails.
    pub async fn close(&self) -> LighterWsResult<()> {
        info!("Closing WebSocket client");

        self.is_running.store(false, Ordering::Relaxed);
        self.is_authenticated.store(false, Ordering::Relaxed);

        if let Some(command_tx) = self.command_tx.write().await.take() {
            let _ = command_tx.send(WebSocketCommand::Close);
        }

        // Abort background task
        if let Some(handle) = self.task_handle.write().await.take() {
            handle.abort();
        }

        Ok(())
    }

    /// Check if the client is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Check if the client is authenticated (for private clients).
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.is_authenticated.load(Ordering::Relaxed)
    }

    /// Get the current environment.
    #[must_use]
    pub const fn environment(&self) -> LighterEnvironment {
        self.environment
    }

    /// Set or update the authentication token.
    ///
    /// This allows updating the auth token after client creation,
    /// useful when the token is generated dynamically.
    pub async fn set_auth_token(&self, token: String) {
        *self.auth_token.write().await = Some(token);
    }

    /// Subscribe to account updates for a specific account.
    ///
    /// Subscribes to both `Account` (balance updates) and `Orders` (order updates) channels.
    /// These are the available private channels in Lighter's WebSocket API.
    ///
    /// # Arguments
    ///
    /// * `_account_index` - The account index (currently unused as Lighter private channels
    ///   are tied to the authenticated user, not a specific account index).
    ///
    /// # Errors
    ///
    /// Returns an error if subscription fails or client is not connected.
    pub async fn subscribe_account(&self, _account_index: i64) -> LighterWsResult<()> {
        self.subscribe(vec![SubscriptionType::Account, SubscriptionType::Orders])
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        execution::{
            dispatch::{DispatchOutcome, OrderDispatchStatus, dispatch_private_message},
            fixtures::{OrderFixtureStatus, execution_fixture_set},
        },
        websocket::transport::{
            ScriptedWebSocketAction, ScriptedWebSocketConnector, ScriptedWebSocketTransport,
            WebSocketTransport,
        },
    };

    #[test]
    fn test_new_public_client() {
        let client =
            LighterWebSocketClient::new_public(LighterEnvironment::Testnet, None, Some(30));

        assert!(!client.requires_auth);
        assert!(!client.is_running());
        assert!(!client.is_authenticated());
        assert_eq!(client.heartbeat_interval, 30);
        assert!(client.url.contains("testnet"));
    }

    #[test]
    fn test_new_private_client() {
        let client = LighterWebSocketClient::new_private(
            LighterEnvironment::Mainnet,
            "test_token".to_string(),
            None,
            None,
        );

        assert!(client.requires_auth);
        assert!(!client.is_running());
        assert!(!client.is_authenticated());
        assert_eq!(client.heartbeat_interval, DEFAULT_HEARTBEAT_SECS);
        assert!(client.url.contains("mainnet"));
    }

    #[tokio::test]
    async fn test_subscription_management() {
        let client = LighterWebSocketClient::new_public(LighterEnvironment::Testnet, None, None);

        let subs = vec![
            SubscriptionType::Orderbook { market_index: 1 },
            SubscriptionType::Trades { market_index: 1 },
        ];

        // Should fail when not connected
        let result = client.subscribe(subs).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_resubscribe_messages_preserve_channels() {
        let channels = vec![
            "order_book/1".to_string(),
            "trade/1".to_string(),
            "market_stats/1".to_string(),
        ];

        let messages = LighterWebSocketClient::resubscribe_messages(&channels);
        let json: Vec<String> = messages
            .iter()
            .map(OutboundMessage::to_json)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(messages.len(), 3);
        assert!(json[0].contains("order_book/1"));
        assert!(json[1].contains("trade/1"));
        assert!(json[2].contains("market_stats/1"));
    }

    #[tokio::test]
    async fn scripted_transport_records_outbound_and_replays_inbound_text() {
        let (mut transport, probe) = ScriptedWebSocketTransport::new(vec![
            ScriptedWebSocketAction::ExpectSendContains("\"type\":\"ping\""),
            ScriptedWebSocketAction::RecvText(r#"{"type":"auth_success"}"#.to_string()),
            ScriptedWebSocketAction::Close,
        ]);

        transport
            .send_text(r#"{"type":"ping"}"#.to_string())
            .await
            .expect("scripted send");

        assert_eq!(probe.sent_texts(), vec![r#"{"type":"ping"}"#.to_string()]);
        assert_eq!(
            transport
                .recv_text()
                .await
                .expect("scripted recv")
                .as_deref(),
            Some(r#"{"type":"auth_success"}"#)
        );
        assert!(
            transport
                .recv_text()
                .await
                .expect("scripted close")
                .is_none()
        );
    }

    #[tokio::test]
    async fn private_ws_dry_run_uses_scripted_transport_for_auth_subscribe_and_dispatch() {
        let fixtures = execution_fixture_set();
        let filled_order = fixtures
            .orders
            .iter()
            .find(|order| order.status == OrderFixtureStatus::Filled)
            .expect("filled fixture");

        let order_json = format!(
            r#"{{"type":"update/account_all_orders","channel":"account_all_orders:42","order":{{"order_id":"{}","client_order_id":"{}","market_index":{},"status":"{}","side":"{}","order_type":"{}","price":"{}","quantity":"{}","filled_quantity":"{}","timestamp":{}}}}}"#,
            filled_order.order_id,
            filled_order.client_order_id,
            filled_order.market_index,
            filled_order.status.as_lighter_status(),
            filled_order.side,
            filled_order.order_type,
            filled_order.price,
            filled_order.quantity,
            filled_order.filled_quantity,
            filled_order.timestamp_ms,
        );
        let account_json = format!(
            r#"{{"type":"update/account_all","channel":"account_all:42","account":{{"account_index":{},"address":"{}","balances":{{"{}":"{}"}}}},"timestamp":{}}}"#,
            fixtures.account.account_index,
            fixtures.account.address,
            fixtures.account.asset,
            fixtures.account.balance,
            fixtures.account.timestamp_ms,
        );

        let (transport, probe) = ScriptedWebSocketTransport::new(vec![
            ScriptedWebSocketAction::ExpectSendContains("\"type\":\"auth\""),
            ScriptedWebSocketAction::RecvText(r#"{"type":"auth_success"}"#.to_string()),
            ScriptedWebSocketAction::ExpectSendContains("account_all"),
            ScriptedWebSocketAction::ExpectSendContains("account_all_orders"),
            ScriptedWebSocketAction::RecvText(
                r#"{"type":"subscribed/account_all","channel":"account_all"}"#.to_string(),
            ),
            ScriptedWebSocketAction::RecvText(
                r#"{"type":"subscribed/account_all_orders","channel":"account_all_orders"}"#
                    .to_string(),
            ),
            ScriptedWebSocketAction::RecvText(order_json),
            ScriptedWebSocketAction::RecvText(account_json),
            ScriptedWebSocketAction::Close,
        ]);

        let connector = ScriptedWebSocketConnector::new(transport);
        let mut client = LighterWebSocketClient::new_private_with_transport_connector(
            LighterEnvironment::Testnet,
            "fixture-auth-token".to_string(),
            "scripted://lighter-private".to_string(),
            Some(60),
            connector,
        );

        let mut rx = client.connect().await.expect("connect scripted client");

        let auth = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("auth message timeout")
            .expect("auth message");
        assert!(matches!(auth, InboundMessage::AuthSuccess));
        assert!(client.is_authenticated());

        client
            .subscribe_account(42)
            .await
            .expect("subscribe account channels");

        let mut received = Vec::new();
        for _ in 0..4 {
            received.push(
                tokio::time::timeout(Duration::from_secs(1), rx.recv())
                    .await
                    .expect("private message timeout")
                    .expect("private message"),
            );
        }

        assert!(received.iter().any(|message| matches!(
            message,
            InboundMessage::SubscriptionSuccess { channel } if channel == "account_all"
        )));
        assert!(received.iter().any(|message| matches!(
            message,
            InboundMessage::SubscriptionSuccess { channel } if channel == "account_all_orders"
        )));

        let order_message = received
            .iter()
            .find(|message| matches!(message, InboundMessage::OrderUpdate { .. }))
            .expect("order update");
        let account_message = received
            .iter()
            .find(|message| matches!(message, InboundMessage::AccountUpdate { .. }))
            .expect("account update");

        assert_eq!(
            dispatch_private_message(order_message),
            DispatchOutcome::Order {
                order_id: filled_order.order_id.to_string(),
                client_order_id: Some(filled_order.client_order_id.to_string()),
                market_index: filled_order.market_index,
                status: OrderDispatchStatus::Filled,
                venue_status: "filled".to_string(),
                timestamp_ms: filled_order.timestamp_ms,
            }
        );
        assert_eq!(
            dispatch_private_message(account_message),
            DispatchOutcome::Account {
                address: fixtures.account.address,
                balances: vec![(fixtures.account.asset, fixtures.account.balance)],
                timestamp_ms: fixtures.account.timestamp_ms,
            }
        );

        let sent = probe.sent_texts();
        assert_eq!(sent.len(), 3);
        assert!(sent[0].contains("\"type\":\"auth\""));
        assert!(sent[0].contains("fixture-auth-token"));
        assert!(sent.iter().any(|text| text.contains("account_all")));
        assert!(sent.iter().any(|text| text.contains("account_all_orders")));

        client.close().await.expect("close scripted client");
    }
}
