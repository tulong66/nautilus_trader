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

//! Example demonstrating WebSocket message processing for Lighter DEX adapter.
//!
//! This example shows how to:
//! - Connect to the Lighter WebSocket
//! - Subscribe to market data channels
//! - Process different message types (orderbook, trades, ticker)
//! - Handle authentication for private channels
//! - Implement automatic reconnection

use nautilus_lighter::{
    common::LighterEnvironment,
    websocket::{InboundMessage, LighterWebSocketClient, SubscriptionType},
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Starting Lighter WebSocket example");

    // Create public WebSocket client
    let mut client = LighterWebSocketClient::new_public(
        LighterEnvironment::Testnet,
        None,      // Use default URL
        Some(30),  // 30 second heartbeat
    );

    // Connect and get message receiver
    let mut msg_rx = client.connect().await?;
    info!("Connected to Lighter WebSocket");

    // Subscribe to market data for market index 1
    let subscriptions = vec![
        SubscriptionType::Orderbook { market_index: 1 },
        SubscriptionType::Trades { market_index: 1 },
        SubscriptionType::Ticker { market_index: 1 },
    ];

    client.subscribe(subscriptions).await?;
    info!("Subscribed to market data channels");

    // Message processing loop
    while let Some(msg) = msg_rx.recv().await {
        match msg {
            InboundMessage::OrderbookSnapshot {
                market_index,
                bids,
                asks,
                timestamp,
            } => {
                info!(
                    "Orderbook snapshot for market {}: {} bids, {} asks at {}",
                    market_index,
                    bids.len(),
                    asks.len(),
                    timestamp
                );
                if !bids.is_empty() {
                    info!("Best bid: {} @ {}", bids[0].1, bids[0].0);
                }
                if !asks.is_empty() {
                    info!("Best ask: {} @ {}", asks[0].1, asks[0].0);
                }
            }

            InboundMessage::OrderbookUpdate {
                market_index,
                bids,
                asks,
                timestamp,
            } => {
                info!(
                    "Orderbook update for market {}: {} bid updates, {} ask updates at {}",
                    market_index,
                    bids.len(),
                    asks.len(),
                    timestamp
                );
            }

            InboundMessage::Trade {
                market_index,
                trade_id,
                price,
                size,
                is_buy,
                timestamp,
            } => {
                info!(
                    "Trade {} on market {}: {} {} @ {} at {}",
                    trade_id,
                    market_index,
                    if is_buy { "BUY" } else { "SELL" },
                    size,
                    price,
                    timestamp
                );
            }

            InboundMessage::Ticker {
                market_index,
                last_price,
                bid_price,
                ask_price,
                volume_24h,
                high_24h,
                low_24h,
                timestamp,
                ..
            } => {
                info!("Ticker update for market {}: last={:?}, bid={:?}, ask={:?}, 24h vol={:?}, high={:?}, low={:?} at {}",
                    market_index, last_price, bid_price, ask_price, volume_24h, high_24h, low_24h, timestamp);
            }

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
                info!(
                    "Order update: id={}, client_id={:?}, market={}, status={}, side={}, type={}, price={}, qty={}, filled={} at {}",
                    order_id, client_order_id, market_index, status, side, order_type, price, quantity, filled_quantity, timestamp
                );
            }

            InboundMessage::AccountUpdate {
                address,
                balances,
                timestamp,
            } => {
                info!(
                    "Account update for {}: {} balances at {}",
                    address,
                    balances.len(),
                    timestamp
                );
                for (asset, balance) in &balances {
                    info!("  {} : {}", asset, balance);
                }
            }

            InboundMessage::Pong => {
                info!("Received pong");
            }

            InboundMessage::Error { code, message } => {
                error!("WebSocket error: code={}, message={}", code, message);
            }

            InboundMessage::SubscriptionSuccess { channel } => {
                info!("Successfully subscribed to channel: {}", channel);
            }

            InboundMessage::UnsubscriptionSuccess { channel } => {
                info!("Successfully unsubscribed from channel: {}", channel);
            }

            InboundMessage::AuthSuccess => {
                info!("Authentication successful");
            }

            InboundMessage::Raw(value) => {
                info!("Received raw message: {:?}", value);
            }
        }
    }

    // Close connection
    client.close().await?;
    info!("WebSocket connection closed");

    Ok(())
}
