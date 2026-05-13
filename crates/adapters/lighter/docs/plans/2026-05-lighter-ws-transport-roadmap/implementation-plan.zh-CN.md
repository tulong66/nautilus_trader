# Lighter WebSocket B-core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement B-core for Lighter WebSocket: a production-direction transport abstraction plus deterministic scripted private WS dry-run tests for P2-K, while preserving the B-full roadmap.

**Architecture:** Add a narrow `WebSocketTransport` boundary with a live `tokio_tungstenite` connector and a test-only scripted connector. Refactor `LighterWebSocketClient` so auth, subscribe, heartbeat, receive, parse, and message emission run through shared session logic that owns the selected transport. Keep `LighterExecutionClient` live signing gate unchanged; B-core tests use stub tokens and fixture messages only.

**Tech Stack:** Rust, Tokio, tokio-tungstenite, futures-util, async-trait, NautilusTrader `nautilus-lighter` crate, Cargo tests.

---

## Scope and safety boundary

### In scope

- Create `crates/adapters/lighter/src/websocket/transport.rs`.
- Add `WebSocketTransport`, `WebSocketTransportConnector`, `TungsteniteWebSocketTransport`, and `TungsteniteWebSocketConnector`.
- Add test-only `ScriptedWebSocketTransport`, `ScriptedWebSocketConnector`, `ScriptedWebSocketAction`, and probe helpers.
- Refactor `LighterWebSocketClient` to use a connector and command channel instead of directly storing a tungstenite write sink.
- Add B-core dry-run tests proving stub auth, private subscriptions, order/account parsing, and `dispatch_private_message` outcomes.
- Update status/docs only after verification.

### Out of scope

- No real Lighter testnet/mainnet private WS.
- No `.env`, wallet, key file, API key, keyring, KMS, or credential manager reads.
- No real auth token generation for tests.
- No live order/cancel/cancel-all, withdraw, transfer, leverage, margin, funding, or liquidation action.
- No B-full reconnect state machine, soak replay, latency/retry/rate-limit model, or execution reconciliation rewrite.

### Do not commit automatically

Do not run `git commit` unless the user explicitly asks. This plan intentionally omits commit steps because the current session rule is to stop after verification and summarize.

---

## File structure

- Create: `crates/adapters/lighter/src/websocket/transport.rs`
  - Owns WebSocket transport abstraction and live/test transport implementations.
- Modify: `crates/adapters/lighter/src/websocket/mod.rs`
  - Exports `transport` module and production transport traits/connectors.
- Modify: `crates/adapters/lighter/src/websocket/client.rs`
  - Replaces direct `connect_async`/`WsSink` storage with connector + command channel + shared session loop.
  - Keeps public constructors and public methods compatible.
  - Adds B-core unit tests using scripted transport.
- Modify after successful verification only: `crates/adapters/lighter/docs/STATUS_REPORT.md`
  - Mark P2-K complete if verification passes.
  - Keep P2-R incomplete unless all J-Q verification is complete.
- Read-only verification: `crates/adapters/lighter/src/execution/client.rs`, `crates/adapters/lighter/src/execution/dispatch.rs`, `crates/adapters/lighter/src/execution/fixtures.rs`, `crates/adapters/lighter/tests/*`.

---

## Task 0: Technical validation before edits

**Files:**
- Read-only: `crates/adapters/lighter/src/websocket/client.rs`
- Read-only: `crates/adapters/lighter/src/websocket/messages.rs`
- Read-only: `crates/adapters/lighter/src/execution/client.rs`
- Read-only: `crates/adapters/lighter/src/execution/dispatch.rs`
- Read-only: `crates/adapters/lighter/src/execution/fixtures.rs`

- [ ] **Step 0.1: Verify current call sites**

Run from `/Volumes/HY2TB/projects/nautilus_trader-lighter/.claude/worktrees/lighter-execution-wiring`:

```bash
rg -n "LighterWebSocketClient::new_|\.connect\(\)\.await|subscribe_account|subscribe\(" crates/adapters/lighter/src crates/adapters/lighter/tests
```

Expected: call sites are concentrated in Lighter data/execution clients and websocket unit tests. Record any unexpected call site before editing.

- [ ] **Step 0.2: Verify private fixture parser support**

Run:

```bash
rg -n "auth_success|authenticated|account_all|account_all_orders|dispatch_private_message|to_ws_message" crates/adapters/lighter/src crates/adapters/lighter/tests
```

Expected:
- `InboundMessage::parse` supports `auth_success` / `authenticated`.
- `parse_lighter_data_message` routes `account_all` and `account_all_orders`.
- `dispatch_private_message` handles `OrderUpdate` and `AccountUpdate`.
- `execution_fixture_set()` has order/account fixtures.

- [ ] **Step 0.3: Verify safety baseline before implementation**

Run:

```bash
rg -n "std::env::var|dotenv|keyring|Keychain|SecretsManager|KMS|wallet|private key file|\.env" crates/adapters/lighter/src
rg -n "withdraw|transfer|leverage|margin|stake|unstake|sub_account" crates/adapters/lighter/src crates/adapters/lighter/tests/signing_surface.rs
```

Expected: no secret-loading implementation; high-risk operation names only appear in deny-list tests or non-strategy docs/comments.

---

## Task 1: Add failing transport abstraction tests

**Files:**
- Modify: `crates/adapters/lighter/src/websocket/client.rs`

- [ ] **Step 1.1: Add failing unit tests that reference missing transport types**

Inside `#[cfg(test)] mod tests` in `client.rs`, extend imports and add the tests below. These tests intentionally reference `crate::websocket::transport::*`, which does not exist yet.

Add imports inside the test module:

```rust
use std::time::Duration;

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
```

Add this direct scripted transport test:

```rust
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

    assert_eq!(
        probe.sent_texts(),
        vec![r#"{"type":"ping"}"#.to_string()]
    );
    assert_eq!(
        transport.recv_text().await.expect("scripted recv").as_deref(),
        Some(r#"{"type":"auth_success"}"#)
    );
    assert!(transport.recv_text().await.expect("scripted close").is_none());
}
```

Add this private dry-run test:

```rust
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
            r#"{"type":"subscribed/account_all_orders","channel":"account_all_orders"}"#.to_string(),
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
```

- [ ] **Step 1.2: Run tests to verify RED**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter scripted_transport_records_outbound_and_replays_inbound_text private_ws_dry_run_uses_scripted_transport_for_auth_subscribe_and_dispatch
```

Expected: compile failure because `crate::websocket::transport` and `new_private_with_transport_connector` do not exist.

---

## Task 2: Add transport module and scripted transport

**Files:**
- Create: `crates/adapters/lighter/src/websocket/transport.rs`
- Modify: `crates/adapters/lighter/src/websocket/mod.rs`

- [ ] **Step 2.1: Create `transport.rs` with live and scripted transport**

Create `crates/adapters/lighter/src/websocket/transport.rs` with this content:

```rust
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

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::Message,
};

use crate::error::LighterError;

pub type BoxWebSocketTransport = Box<dyn WebSocketTransport>;

#[async_trait]
pub trait WebSocketTransport: Send {
    async fn send_text(&mut self, text: String) -> Result<(), LighterError>;
    async fn recv_text(&mut self) -> Result<Option<String>, LighterError>;
    async fn close(&mut self) -> Result<(), LighterError>;
}

#[async_trait]
pub trait WebSocketTransportConnector: Send + Sync {
    async fn connect(&self, url: &str) -> Result<BoxWebSocketTransport, LighterError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TungsteniteWebSocketConnector;

#[async_trait]
impl WebSocketTransportConnector for TungsteniteWebSocketConnector {
    async fn connect(&self, url: &str) -> Result<BoxWebSocketTransport, LighterError> {
        let (stream, _) = connect_async(url)
            .await
            .map_err(|e| LighterError::WebSocket(format!("Failed to connect: {e}")))?;
        Ok(Box::new(TungsteniteWebSocketTransport::new(stream)))
    }
}

pub struct TungsteniteWebSocketTransport {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl TungsteniteWebSocketTransport {
    #[must_use]
    pub const fn new(stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
        Self { stream }
    }
}

#[async_trait]
impl WebSocketTransport for TungsteniteWebSocketTransport {
    async fn send_text(&mut self, text: String) -> Result<(), LighterError> {
        self.stream
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| LighterError::WebSocket(format!("Failed to send text frame: {e}")))
    }

    async fn recv_text(&mut self) -> Result<Option<String>, LighterError> {
        loop {
            let Some(message) = self.stream.next().await else {
                return Ok(None);
            };

            match message {
                Ok(Message::Text(text)) => return Ok(Some(text.to_string())),
                Ok(Message::Pong(_)) => return Ok(Some(r#"{"type":"pong"}"#.to_string())),
                Ok(Message::Close(_)) => return Ok(None),
                Ok(_) => continue,
                Err(e) => {
                    return Err(LighterError::WebSocket(format!(
                        "Failed to receive text frame: {e}"
                    )));
                }
            }
        }
    }

    async fn close(&mut self) -> Result<(), LighterError> {
        self.stream
            .close(None)
            .await
            .map_err(|e| LighterError::WebSocket(format!("Failed to close WebSocket: {e}")))
    }
}

#[cfg(test)]
mod scripted {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use tokio::sync::Notify;

    use super::{BoxWebSocketTransport, WebSocketTransport, WebSocketTransportConnector};
    use crate::error::LighterError;

    #[derive(Debug)]
    pub enum ScriptedWebSocketAction {
        ExpectSendContains(&'static str),
        RecvText(String),
        Close,
    }

    #[derive(Clone, Default)]
    pub struct ScriptedWebSocketProbe {
        sent_texts: Arc<Mutex<Vec<String>>>,
    }

    impl ScriptedWebSocketProbe {
        #[must_use]
        pub fn sent_texts(&self) -> Vec<String> {
            self.sent_texts.lock().expect("probe poisoned").clone()
        }
    }

    pub struct ScriptedWebSocketTransport {
        actions: Arc<Mutex<VecDeque<ScriptedWebSocketAction>>>,
        notify: Arc<Notify>,
        probe: ScriptedWebSocketProbe,
        closed: bool,
    }

    impl ScriptedWebSocketTransport {
        #[must_use]
        pub fn new(actions: Vec<ScriptedWebSocketAction>) -> (Self, ScriptedWebSocketProbe) {
            let probe = ScriptedWebSocketProbe::default();
            (
                Self {
                    actions: Arc::new(Mutex::new(actions.into())),
                    notify: Arc::new(Notify::new()),
                    probe: probe.clone(),
                    closed: false,
                },
                probe,
            )
        }
    }

    #[async_trait]
    impl WebSocketTransport for ScriptedWebSocketTransport {
        async fn send_text(&mut self, text: String) -> Result<(), LighterError> {
            if self.closed {
                return Err(LighterError::WebSocket("Scripted transport is closed".to_string()));
            }

            let mut actions = self.actions.lock().expect("script poisoned");
            match actions.pop_front() {
                Some(ScriptedWebSocketAction::ExpectSendContains(expected)) => {
                    if !text.contains(expected) {
                        return Err(LighterError::WebSocket(format!(
                            "Unexpected outbound frame; expected text containing {expected}"
                        )));
                    }
                    self.probe
                        .sent_texts
                        .lock()
                        .expect("probe poisoned")
                        .push(text);
                    drop(actions);
                    self.notify.notify_waiters();
                    Ok(())
                }
                Some(action) => {
                    actions.push_front(action);
                    Err(LighterError::WebSocket(
                        "Unexpected outbound frame before scripted send expectation".to_string(),
                    ))
                }
                None => Err(LighterError::WebSocket(
                    "Unexpected outbound frame after script completed".to_string(),
                )),
            }
        }

        async fn recv_text(&mut self) -> Result<Option<String>, LighterError> {
            loop {
                if self.closed {
                    return Ok(None);
                }

                let next = {
                    let mut actions = self.actions.lock().expect("script poisoned");
                    match actions.front() {
                        Some(ScriptedWebSocketAction::RecvText(_)) => actions.pop_front(),
                        Some(ScriptedWebSocketAction::Close) => actions.pop_front(),
                        Some(ScriptedWebSocketAction::ExpectSendContains(_)) => None,
                        None => Some(ScriptedWebSocketAction::Close),
                    }
                };

                match next {
                    Some(ScriptedWebSocketAction::RecvText(text)) => return Ok(Some(text)),
                    Some(ScriptedWebSocketAction::Close) => {
                        self.closed = true;
                        return Ok(None);
                    }
                    Some(ScriptedWebSocketAction::ExpectSendContains(_)) => unreachable!(),
                    None => self.notify.notified().await,
                }
            }
        }

        async fn close(&mut self) -> Result<(), LighterError> {
            self.closed = true;
            self.notify.notify_waiters();
            Ok(())
        }
    }

    pub struct ScriptedWebSocketConnector {
        transport: Mutex<Option<ScriptedWebSocketTransport>>,
    }

    impl ScriptedWebSocketConnector {
        #[must_use]
        pub fn new(transport: ScriptedWebSocketTransport) -> Arc<Self> {
            Arc::new(Self {
                transport: Mutex::new(Some(transport)),
            })
        }
    }

    #[async_trait]
    impl WebSocketTransportConnector for ScriptedWebSocketConnector {
        async fn connect(&self, _url: &str) -> Result<BoxWebSocketTransport, LighterError> {
            let transport = self
                .transport
                .lock()
                .expect("scripted connector poisoned")
                .take()
                .ok_or_else(|| {
                    LighterError::WebSocket("Scripted transport already consumed".to_string())
                })?;
            Ok(Box::new(transport))
        }
    }
}

#[cfg(test)]
pub use scripted::{
    ScriptedWebSocketAction, ScriptedWebSocketConnector, ScriptedWebSocketProbe,
    ScriptedWebSocketTransport,
};
```

- [ ] **Step 2.2: Export the transport module**

Modify `crates/adapters/lighter/src/websocket/mod.rs` from:

```rust
pub mod client;
pub mod messages;

pub use client::LighterWebSocketClient;
pub use messages::{
    InboundMessage, OutboundMessage, SubscriptionChannel, SubscriptionType, WebSocketError,
};
```

to:

```rust
pub mod client;
pub mod messages;
pub mod transport;

pub use client::LighterWebSocketClient;
pub use messages::{
    InboundMessage, OutboundMessage, SubscriptionChannel, SubscriptionType, WebSocketError,
};
pub use transport::{
    BoxWebSocketTransport, TungsteniteWebSocketConnector, TungsteniteWebSocketTransport,
    WebSocketTransport, WebSocketTransportConnector,
};
```

- [ ] **Step 2.3: Run the direct scripted transport test**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter scripted_transport_records_outbound_and_replays_inbound_text
```

Expected: the direct scripted transport test passes, while the private dry-run test still fails because `new_private_with_transport_connector` and client command-channel refactor are not implemented.

---

## Task 3: Refactor `LighterWebSocketClient` around connector + command channel

**Files:**
- Modify: `crates/adapters/lighter/src/websocket/client.rs`

- [ ] **Step 3.1: Replace direct tungstenite imports and sink alias**

Change imports near the top of `client.rs`.

Replace:

```rust
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
```

with:

```rust
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
```

Change the crate imports from:

```rust
use crate::{
    common::{LighterEnvironment, build_public_ws_url, build_private_ws_url},
    error::LighterError,
    websocket::messages::{InboundMessage, OutboundMessage, SubscriptionType},
};
```

to:

```rust
use crate::{
    common::{LighterEnvironment, build_private_ws_url, build_public_ws_url},
    error::LighterError,
    websocket::{
        messages::{InboundMessage, OutboundMessage, SubscriptionType},
        transport::{TungsteniteWebSocketConnector, WebSocketTransport, WebSocketTransportConnector},
    },
};
```

Remove this type alias entirely:

```rust
type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;
```

- [ ] **Step 3.2: Add command enum and replace `ws_write` field**

Add this enum near the constants:

```rust
enum WebSocketCommand {
    Send(OutboundMessage),
    Close,
}
```

In `LighterWebSocketClient`, replace:

```rust
/// WebSocket write handle for sending messages.
ws_write: Arc<tokio::sync::Mutex<Option<WsSink>>>,
```

with:

```rust
/// Command sender for the connection loop.
command_tx: Arc<RwLock<Option<mpsc::UnboundedSender<WebSocketCommand>>>>,
/// Transport connector.
transport_connector: Arc<dyn WebSocketTransportConnector>,
```

- [ ] **Step 3.3: Update constructors**

In both `new_public` and `new_private`, replace:

```rust
ws_write: Arc::new(tokio::sync::Mutex::new(None)),
task_handle: Arc::new(RwLock::new(None)),
```

with:

```rust
command_tx: Arc::new(RwLock::new(None)),
transport_connector: Arc::new(TungsteniteWebSocketConnector),
task_handle: Arc::new(RwLock::new(None)),
```

Add this test-only constructor inside `impl LighterWebSocketClient` after `new_private`:

```rust
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
```

- [ ] **Step 3.4: Update `connect()` to create a command channel**

In `connect()`, after creating `msg_tx`, add:

```rust
let (command_tx, command_rx) = mpsc::unbounded_channel();
*self.command_tx.write().await = Some(command_tx);
```

Replace the old `ws_write` clone and `run_connection_loop` call with:

```rust
let transport_connector = Arc::clone(&self.transport_connector);

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
```

- [ ] **Step 3.5: Replace `run_connection_loop` with transport-backed session loop**

Replace the `run_connection_loop` function with:

```rust
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

                if let Err(e) = Self::send_initial_auth(requires_auth, &auth_token, transport.as_mut()).await {
                    error!("Failed to authenticate WebSocket session: {}", e);
                    break;
                }

                let subs = subscriptions.read().await.clone();
                if let Err(e) = Self::send_subscription_channels(transport.as_mut(), &subs).await {
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
```

Add these helper methods after `run_connection_loop`:

```rust
async fn send_initial_auth(
    requires_auth: bool,
    auth_token: &Arc<RwLock<Option<String>>>,
    transport: &mut dyn WebSocketTransport,
) -> LighterWsResult<()> {
    if !requires_auth {
        return Ok(());
    }

    let token = auth_token
        .read()
        .await
        .clone()
        .ok_or_else(|| LighterError::Auth("Authentication required but no token provided".to_string()))?;
    let auth_msg = OutboundMessage::Auth { token };
    Self::send_outbound_message(transport, auth_msg, "auth").await?;
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
    let json = message
        .to_json()
        .map_err(|e| LighterError::WebSocket(format!("Failed to serialize {operation}: {e}")))?;
    transport.send_text(json).await.map_err(|e| {
        LighterError::WebSocket(format!("Failed to send {operation} message: {e}"))
    })
}
```

Add this session loop helper:

```rust
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
```

- [ ] **Step 3.6: Remove old heartbeat helper**

Delete the old `heartbeat_loop_shared` function because heartbeat is now part of `run_session_loop` and uses `WebSocketTransport`.

- [ ] **Step 3.7: Update `subscribe()` to use command channel**

Replace the write-handle wait loop in `subscribe()` with:

```rust
let channels: Vec<String> = subscriptions.iter().map(|s| s.to_channel()).collect();
let command_tx = self
    .command_tx
    .read()
    .await
    .clone()
    .ok_or_else(|| LighterError::WebSocket("WebSocket command channel not ready".to_string()))?;

for channel in &channels {
    command_tx
        .send(WebSocketCommand::Send(OutboundMessage::subscribe(channel)))
        .map_err(|_| LighterError::WebSocket("WebSocket command channel closed".to_string()))?;
    debug!("Queued subscription for channel: {}", channel);
}

self.subscriptions.write().await.extend(channels.clone());
debug!("Subscribed to {} channels", channels.len());
Ok(())
```

Keep the existing `is_running` and private auth checks before this block.

- [ ] **Step 3.8: Update `unsubscribe()` to use command channel**

Replace the write-handle block in `unsubscribe()` with:

```rust
if let Some(command_tx) = self.command_tx.read().await.clone() {
    for channel in &channels {
        command_tx
            .send(WebSocketCommand::Send(OutboundMessage::unsubscribe(channel)))
            .map_err(|_| LighterError::WebSocket("WebSocket command channel closed".to_string()))?;
        debug!("Queued unsubscription for channel: {}", channel);
    }
}
```

Keep the existing subscription removal logic after it.

- [ ] **Step 3.9: Update `close()` to clear command sender**

In `close()`, after setting flags false, add:

```rust
if let Some(command_tx) = self.command_tx.write().await.take() {
    let _ = command_tx.send(WebSocketCommand::Close);
}
```

Keep the existing task abort logic.

- [ ] **Step 3.10: Run focused tests**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter scripted_transport_records_outbound_and_replays_inbound_text private_ws_dry_run_uses_scripted_transport_for_auth_subscribe_and_dispatch
```

Expected: both tests pass. If `private_ws_dry_run...` times out, inspect the script action order and the `tokio::select!` command/recv ordering.

---

## Task 4: Preserve existing WebSocket behavior tests

**Files:**
- Modify only if needed: `crates/adapters/lighter/src/websocket/client.rs`

- [ ] **Step 4.1: Run existing websocket unit tests**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter websocket::client::tests
```

Expected: existing tests pass, including:
- `test_new_public_client`
- `test_new_private_client`
- `test_subscription_management`
- `test_resubscribe_messages_preserve_channels`
- new B-core tests

- [ ] **Step 4.2: If constructor tests fail, update only field assertions**

If tests fail because private fields changed from `ws_write` to `command_tx`, do not remove behavior assertions. Keep assertions for:

```rust
assert!(client.requires_auth);
assert!(!client.is_running());
assert!(!client.is_authenticated());
assert_eq!(client.heartbeat_interval, DEFAULT_HEARTBEAT_SECS);
assert!(client.url.contains("mainnet"));
```

Expected: no externally visible behavior changes.

---

## Task 5: Verify P2-J execution gate remains intact

**Files:**
- Read-only unless tests fail: `crates/adapters/lighter/src/execution/client.rs`

- [ ] **Step 5.1: Run live signing gate tests**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter live_signing
cargo +1.95.0 test -p nautilus-lighter connect_rejects_default_config_before_live_signing_or_private_ws
```

Expected: tests pass and default execution client still rejects before HTTP/private WS/auth token work.

- [ ] **Step 5.2: If tests fail, restore gate ordering**

In `LighterExecutionClient::connect()`, this must remain before any market fetch, nonce fetch, auth token creation, or private WS connect:

```rust
self.ensure_live_signing_enabled()?;
```

Expected: default `enable_live_signing=false` cannot enter live signing or private execution connectivity.

---

## Task 6: Safety audit after B-core changes

**Files:**
- Read-only verification over `crates/adapters/lighter/src` and focused tests.

- [ ] **Step 6.1: Search for secret-loading additions**

Run:

```bash
rg -n "std::env::var|dotenv|keyring|Keychain|SecretsManager|KMS|wallet|private key file|\.env" crates/adapters/lighter/src
```

Expected: no secret-loading implementation. Comments that mention `private_key` fields are acceptable; actual env/keyring/KMS/dotenv loading is not.

- [ ] **Step 6.2: Search for high-risk operation widening**

Run:

```bash
rg -n "withdraw|transfer|leverage|margin|stake|unstake|sub_account" crates/adapters/lighter/src crates/adapters/lighter/tests/signing_surface.rs
```

Expected: high-risk names only appear in deny-list tests or non-strategy documentation; no callable signer/execution path is added.

- [ ] **Step 6.3: Search for real private WS endpoint usage in tests**

Run:

```bash
rg -n "mainnet\.zklighter|testnet\.zklighter|/stream|fixture-auth-token|scripted://" crates/adapters/lighter/src crates/adapters/lighter/tests
```

Expected: B-core dry-run tests use `scripted://` and `fixture-auth-token`; they do not connect to real Lighter private WS endpoints.

---

## Task 7: Update status report only after verification

**Files:**
- Modify: `crates/adapters/lighter/docs/STATUS_REPORT.md`

- [ ] **Step 7.1: Update P2-K row after focused and crate tests pass**

Only after Tasks 3-6 pass, update P2-K row from `[ ]` to `[x]` with wording like:

```markdown
| K | Private WS/auth dry-run harness：建立可注入 token/auth stub 与 private channel 订阅重放测试 | [x] | J 后可并行 | B-core 已建立 WebSocket transport abstraction；live transport 仍走 `tokio_tungstenite`，dry-run 使用 scripted transport；tests 覆盖 stub auth、account/orders subscription、fixture order/account replay、parse + dispatch；不读取 `.env`；不连接真实 private WS；Roadmap 保存在 `docs/plans/2026-05-lighter-ws-transport-roadmap/` |
```

Do not mark P2-R complete unless J-Q all required verification has been completed.

- [ ] **Step 7.2: Update roadmap tasks checkboxes if desired**

Optionally update `docs/plans/2026-05-lighter-ws-transport-roadmap/tasks.zh-CN.md` B-core checkboxes for tasks actually completed. Do not check deferred B-full items.

- [ ] **Step 7.3: Verify doc diff**

Run:

```bash
git diff -- crates/adapters/lighter/docs/STATUS_REPORT.md crates/adapters/lighter/docs/plans/2026-05-lighter-ws-transport-roadmap/tasks.zh-CN.md
```

Expected: only P2-K/B-core status updates and actual verification notes.

---

## Task 8: Final verification suite

**Files:**
- No code changes unless verification exposes a real bug.

- [ ] **Step 8.1: Run focused B-core tests**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter private_ws_dry_run_uses_scripted_transport_for_auth_subscribe_and_dispatch
cargo +1.95.0 test -p nautilus-lighter scripted_transport_records_outbound_and_replays_inbound_text
cargo +1.95.0 test -p nautilus-lighter execution_dispatch
cargo +1.95.0 test -p nautilus-lighter live_signing
```

Expected: all focused tests pass. Record exact output.

- [ ] **Step 8.2: Run full crate tests**

Run:

```bash
cargo +1.95.0 test -p nautilus-lighter
```

Expected: all `nautilus-lighter` tests pass. Record exact pass/fail/ignored counts.

- [ ] **Step 8.3: Run Python feature check**

Run:

```bash
cargo +1.95.0 check -p nautilus-lighter --features python
```

Expected: check passes.

- [ ] **Step 8.4: Run Python import smoke test if environment is available**

Run:

```bash
uv run --project /Volumes/HY2TB/projects/nautilus_trader-lighter --group test pytest /Volumes/HY2TB/projects/nautilus_trader-lighter/tests/integration_tests/adapters/lighter/test_imports.py -q
```

Expected: smoke test passes, historically `3 passed`. If environment setup fails for unrelated dependency reasons, record the exact failure and do not claim full verification.

- [ ] **Step 8.5: Inspect final git state**

Run:

```bash
git status --short
git diff --stat
git diff -- crates/adapters/lighter/src/websocket/client.rs crates/adapters/lighter/src/websocket/mod.rs crates/adapters/lighter/src/websocket/transport.rs crates/adapters/lighter/docs/STATUS_REPORT.md crates/adapters/lighter/docs/plans/2026-05-lighter-ws-transport-roadmap
```

Expected: changed files are limited to B-core WebSocket implementation, B-core tests, and docs/status updates.

---

## Self-review

- Spec coverage: RQ-1.1 through RQ-5.5 are covered by Tasks 0-8. B-full is preserved as deferred scope in docs and not implemented here.
- Safety coverage: Tasks 0, 5, 6, and 8 ensure no secret loading, no real private WS, no live trading, and no P2-J gate regression.
- TDD coverage: Task 1 writes failing tests before `transport.rs` and client refactor implementation. Task 3 completes the minimal production code needed to satisfy the B-core tests.
- Type consistency: the plan uses `WebSocketTransport`, `WebSocketTransportConnector`, `TungsteniteWebSocketConnector`, `ScriptedWebSocketTransport`, `ScriptedWebSocketConnector`, `ScriptedWebSocketAction`, and `new_private_with_transport_connector` consistently.
- Scope check: reconnect/resubscribe state machine, fault injection, latency/retry/rate-limit, execution reconciliation, sequencer semantics, and real credential lifecycle are explicitly deferred to B-full/P2-M/N/O/Q.
- Placeholder scan: no TBD/TODO/fill-in placeholders are present; implementation snippets are concrete enough for execution.
