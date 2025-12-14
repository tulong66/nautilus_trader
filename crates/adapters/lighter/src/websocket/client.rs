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

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::{
    common::{LighterEnvironment, build_public_ws_url, build_private_ws_url},
    error::LighterError,
    websocket::messages::{InboundMessage, OutboundMessage, SubscriptionType},
};

/// Type alias for WebSocket result.
pub type LighterWsResult<T> = Result<T, LighterError>;

/// Default heartbeat interval in seconds.
const DEFAULT_HEARTBEAT_SECS: u64 = 20;

/// Default reconnect delay in milliseconds.
const DEFAULT_RECONNECT_DELAY_MS: u64 = 1000;

/// Maximum reconnect delay in milliseconds.
const MAX_RECONNECT_DELAY_MS: u64 = 30000;

/// Type alias for the WebSocket write sink.
type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

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
    /// WebSocket write handle for sending messages.
    ws_write: Arc<tokio::sync::Mutex<Option<WsSink>>>,
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
            .field("is_authenticated", &self.is_authenticated.load(Ordering::Relaxed))
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
            ws_write: Arc::new(tokio::sync::Mutex::new(None)),
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
            ws_write: Arc::new(tokio::sync::Mutex::new(None)),
            task_handle: Arc::new(RwLock::new(None)),
        }
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
        *self.msg_tx.write().await = Some(msg_tx.clone());

        let url = self.url.clone();
        let requires_auth = self.requires_auth;
        let auth_token = Arc::clone(&self.auth_token);
        let heartbeat_interval = self.heartbeat_interval;
        let subscriptions = Arc::clone(&self.subscriptions);
        let is_running = Arc::clone(&self.is_running);
        let is_authenticated = Arc::clone(&self.is_authenticated);
        let ws_write = Arc::clone(&self.ws_write);

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
                ws_write,
            )
            .await;
        });

        *self.task_handle.write().await = Some(handle);
        self.is_running.store(true, Ordering::Relaxed);

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
        ws_write: Arc<tokio::sync::Mutex<Option<WsSink>>>,
    ) {
        let mut reconnect_delay = DEFAULT_RECONNECT_DELAY_MS;

        while is_running.load(Ordering::Relaxed) {
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    info!("WebSocket connected successfully");
                    reconnect_delay = DEFAULT_RECONNECT_DELAY_MS; // Reset delay on successful connection

                    let (mut write, mut read) = ws_stream.split();

                    // Authenticate if required
                    if requires_auth {
                        if let Some(token) = auth_token.read().await.as_ref() {
                            let auth_msg = OutboundMessage::Auth {
                                token: token.clone(),
                            };
                            if let Ok(json) = auth_msg.to_json() {
                                if let Err(e) = write.send(Message::Text(json.into())).await {
                                    error!("Failed to send auth message: {}", e);
                                    continue;
                                }
                                debug!("Authentication message sent");
                            }
                        } else {
                            error!("Authentication required but no token provided");
                            break;
                        }
                    }

                    // Resubscribe to previous subscriptions (one by one)
                    let subs = subscriptions.read().await.clone();
                    for channel in &subs {
                        let sub_msg = OutboundMessage::subscribe(channel);
                        if let Ok(json) = sub_msg.to_json() {
                            if let Err(e) = write.send(Message::Text(json.into())).await {
                                error!("Failed to resubscribe to {}: {}", channel, e);
                                continue;
                            }
                            debug!("Resubscribed to channel: {}", channel);
                        }
                    }

                    // Store write handle in shared state for external access
                    *ws_write.lock().await = Some(write);

                    // Start heartbeat task using shared write handle
                    let heartbeat_handle = {
                        let ws_write_clone = Arc::clone(&ws_write);
                        let is_running = Arc::clone(&is_running);
                        tokio::spawn(async move {
                            Self::heartbeat_loop_shared(ws_write_clone, heartbeat_interval, is_running).await;
                        })
                    };

                    // Process incoming messages
                    while let Some(msg_result) = read.next().await {
                        if !is_running.load(Ordering::Relaxed) {
                            break;
                        }

                        match msg_result {
                            Ok(Message::Text(text)) => {
                                match Self::parse_message(&text, requires_auth, &is_authenticated) {
                                    Ok(parsed_msg) => {
                                        if msg_tx.send(parsed_msg).is_err() {
                                            warn!("Message receiver dropped, stopping client");
                                            is_running.store(false, Ordering::Relaxed);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse message: {}", e);
                                    }
                                }
                            }
                            Ok(Message::Pong(_)) => {
                                debug!("Received pong");
                                if msg_tx.send(InboundMessage::Pong).is_err() {
                                    warn!("Message receiver dropped, stopping client");
                                    is_running.store(false, Ordering::Relaxed);
                                    break;
                                }
                            }
                            Ok(Message::Close(_)) => {
                                info!("WebSocket closed by server");
                                break;
                            }
                            Ok(_) => {
                                debug!("Received non-text message");
                            }
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }

                    // Cleanup
                    heartbeat_handle.abort();
                    info!("WebSocket connection closed");
                }
                Err(e) => {
                    error!("Failed to connect to WebSocket: {}", e);
                }
            }

            // Reconnect logic
            if is_running.load(Ordering::Relaxed) {
                warn!(
                    "Reconnecting in {} ms...",
                    reconnect_delay
                );
                tokio::time::sleep(Duration::from_millis(reconnect_delay)).await;

                // Exponential backoff
                reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY_MS);
            }
        }

        info!("WebSocket client stopped");
    }

    /// Heartbeat loop using shared optional write handle.
    async fn heartbeat_loop_shared(
        ws_write: Arc<tokio::sync::Mutex<Option<WsSink>>>,
        interval_secs: u64,
        is_running: Arc<AtomicBool>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

        while is_running.load(Ordering::Relaxed) {
            interval.tick().await;

            let ping_msg = OutboundMessage::Ping;
            if let Ok(json) = ping_msg.to_json() {
                let mut write_guard = ws_write.lock().await;
                if let Some(write) = write_guard.as_mut() {
                    if let Err(e) = write.send(Message::Text(json.into())).await {
                        error!("Failed to send ping: {}", e);
                        break;
                    }
                    debug!("Sent ping");
                } else {
                    warn!("WebSocket write handle not available for ping");
                    break;
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
            return Err(LighterError::WebSocket("Client is not connected".to_string()));
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

        // Wait for WebSocket connection to be established (up to 5 seconds)
        let mut retries = 0;
        const MAX_RETRIES: u32 = 50;
        const RETRY_DELAY_MS: u64 = 100;

        loop {
            {
                let mut write_guard = self.ws_write.lock().await;
                if let Some(write) = write_guard.as_mut() {
                    // Send subscription messages
                    for channel in &channels {
                        let sub_msg = OutboundMessage::subscribe(channel);
                        if let Ok(json) = sub_msg.to_json() {
                            if let Err(e) = write.send(Message::Text(json.into())).await {
                                error!("Failed to subscribe to {}: {}", channel, e);
                                return Err(LighterError::WebSocket(format!(
                                    "Failed to subscribe to {}: {}", channel, e
                                )));
                            }
                            debug!("Sent subscription for channel: {}", channel);
                        }
                    }
                    break;
                }
            }

            retries += 1;
            if retries >= MAX_RETRIES {
                return Err(LighterError::WebSocket(
                    "WebSocket connection not ready after timeout".to_string(),
                ));
            }

            // Wait a bit before retrying
            tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;

            // Check if still running
            if !self.is_running.load(Ordering::Relaxed) {
                return Err(LighterError::WebSocket("Client disconnected while waiting".to_string()));
            }
        }

        // Add to subscriptions list for reconnection
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
            return Err(LighterError::WebSocket("Client is not connected".to_string()));
        }

        let channels: Vec<String> = subscriptions.iter().map(|s| s.to_channel()).collect();

        // Send unsubscription messages via WebSocket
        {
            let mut write_guard = self.ws_write.lock().await;
            if let Some(write) = write_guard.as_mut() {
                for channel in &channels {
                    let unsub_msg = OutboundMessage::unsubscribe(channel);
                    if let Ok(json) = unsub_msg.to_json() {
                        if let Err(e) = write.send(Message::Text(json.into())).await {
                            error!("Failed to unsubscribe from {}: {}", channel, e);
                            // Continue with other unsubscriptions even if one fails
                        } else {
                            debug!("Sent unsubscription for channel: {}", channel);
                        }
                    }
                }
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
        self.subscribe(vec![
            SubscriptionType::Account,
            SubscriptionType::Orders,
        ])
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_public_client() {
        let client = LighterWebSocketClient::new_public(
            LighterEnvironment::Testnet,
            None,
            Some(30),
        );

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
        let client = LighterWebSocketClient::new_public(
            LighterEnvironment::Testnet,
            None,
            None,
        );

        let subs = vec![
            SubscriptionType::Orderbook { market_index: 1 },
            SubscriptionType::Trades { market_index: 1 },
        ];

        // Should fail when not connected
        let result = client.subscribe(subs).await;
        assert!(result.is_err());
    }
}
