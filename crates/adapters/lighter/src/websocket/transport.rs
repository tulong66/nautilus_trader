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
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

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
                return Err(LighterError::WebSocket(
                    "Scripted transport is closed".to_string(),
                ));
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
