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

//! HTTP integration tests for the Lighter adapter.

use std::{
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use axum::{
    Router,
    extract::State,
    routing::{get, post},
    response::Json,
};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use nautilus_lighter::http::{
    client::LighterRawHttpClient,
    endpoints::{ORDER_BOOKS, NEXT_NONCE, SEND_TX, CANDLESTICKS, ACCOUNT, ACCOUNT_ACTIVE_ORDERS, RECENT_TRADES, ORDER_BOOK_ORDERS},
    types::{LighterResponse, LighterList, Market, NextNonceResponse, TxResponse},
};

// ------------------------------------------------------------------------------------------------
// Test server state
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct TestServerState {
    request_count: Arc<Mutex<usize>>,
    last_request_body: Arc<Mutex<Option<Value>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl TestServerState {
    fn new() -> Self {
        Self::default()
    }

    async fn increment_request_count(&self) {
        let mut count = self.request_count.lock().await;
        *count += 1;
    }

    async fn get_request_count(&self) -> usize {
        *self.request_count.lock().await
    }

    async fn set_should_fail(&self, fail: bool) {
        let mut should_fail = self.should_fail.lock().await;
        *should_fail = fail;
    }

    async fn should_fail(&self) -> bool {
        *self.should_fail.lock().await
    }

    async fn store_request_body(&self, body: Value) {
        let mut last_body = self.last_request_body.lock().await;
        *last_body = Some(body);
    }

    async fn get_last_request_body(&self) -> Option<Value> {
        self.last_request_body.lock().await.clone()
    }
}

// ------------------------------------------------------------------------------------------------
// Mock handlers
// ------------------------------------------------------------------------------------------------

async fn handle_order_books(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    if state.should_fail().await {
        return Json(json!({
            "success": false,
            "error": "Internal server error"
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "items": [
                {
                    "marketIndex": 1,
                    "symbol": "ETH_USDC",
                    "baseAsset": "ETH",
                    "quoteAsset": "USDC",
                    "tickSize": "0.01",
                    "stepSize": "0.001",
                    "minOrderSize": "0.001",
                    "maxOrderSize": "1000",
                    "status": "active"
                },
                {
                    "marketIndex": 2,
                    "symbol": "BTC_USDC",
                    "baseAsset": "BTC",
                    "quoteAsset": "USDC",
                    "tickSize": "0.1",
                    "stepSize": "0.0001",
                    "minOrderSize": "0.0001",
                    "maxOrderSize": "100",
                    "status": "active"
                }
            ]
        }
    }))
}

async fn handle_next_nonce(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    if state.should_fail().await {
        return Json(json!({
            "success": false,
            "error": "Account not found"
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "nonce": 12345
        }
    }))
}

async fn handle_send_tx(
    State(state): State<TestServerState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    state.increment_request_count().await;
    state.store_request_body(body).await;

    if state.should_fail().await {
        return Json(json!({
            "success": false,
            "error": "Transaction rejected: insufficient balance"
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "txId": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            "orderIndex": 42
        }
    }))
}

async fn handle_candlesticks(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    if state.should_fail().await {
        return Json(json!({
            "success": false,
            "error": "Invalid market"
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "items": [
                {
                    "timestamp": 1734200000000_i64,
                    "open": "4100.00",
                    "high": "4150.00",
                    "low": "4090.00",
                    "close": "4127.50",
                    "volume": "1234.56"
                },
                {
                    "timestamp": 1734203600000_i64,
                    "open": "4127.50",
                    "high": "4200.00",
                    "low": "4120.00",
                    "close": "4180.00",
                    "volume": "2345.67"
                }
            ]
        }
    }))
}

async fn handle_account(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    if state.should_fail().await {
        return Json(json!({
            "success": false,
            "error": "Unauthorized"
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "account_index": 878,
            "address": "0x1234567890abcdef1234567890abcdef12345678",
            "balances": {
                "USDC": "100000.00",
                "ETH": "10.5"
            },
            "total_equity": "150000.00",
            "free_collateral": "80000.00"
        }
    }))
}

async fn handle_active_orders(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    if state.should_fail().await {
        return Json(json!({
            "success": false,
            "error": "Unauthorized"
        }));
    }

    Json(json!({
        "success": true,
        "data": {
            "items": [
                {
                    "order_id": "12345",
                    "client_order_id": "C-001",
                    "market_index": 1,
                    "status": "open",
                    "side": "buy",
                    "order_type": "limit",
                    "price": "4100.00",
                    "quantity": "1.0",
                    "filled_quantity": "0.0",
                    "timestamp": 1734200000000_i64
                }
            ]
        }
    }))
}

async fn handle_recent_trades(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    Json(json!({
        "success": true,
        "data": {
            "items": [
                {
                    "trade_id": "100001",
                    "market_index": 1,
                    "price": "4127.50",
                    "size": "0.5",
                    "side": "buy",
                    "timestamp": 1734200000000_i64
                },
                {
                    "trade_id": "100002",
                    "market_index": 1,
                    "price": "4128.00",
                    "size": "1.0",
                    "side": "sell",
                    "timestamp": 1734200001000_i64
                }
            ]
        }
    }))
}

async fn handle_order_book_orders(State(state): State<TestServerState>) -> Json<Value> {
    state.increment_request_count().await;

    Json(json!({
        "success": true,
        "data": {
            "market_index": 1,
            "bids": [
                ["4127.50", "10.5", "3"],
                ["4127.00", "20.0", "5"]
            ],
            "asks": [
                ["4128.00", "5.2", "2"],
                ["4128.50", "15.0", "4"]
            ],
            "timestamp": 1734200000000_i64
        }
    }))
}

// ------------------------------------------------------------------------------------------------
// Test server setup
// ------------------------------------------------------------------------------------------------

fn create_test_router(state: TestServerState) -> Router {
    Router::new()
        .route("/api/v1/order_books", get(handle_order_books))
        .route("/api/v1/next_nonce", get(handle_next_nonce))
        .route("/api/v1/send_tx", post(handle_send_tx))
        .route("/api/v1/candlesticks", get(handle_candlesticks))
        .route("/api/v1/account", get(handle_account))
        .route("/api/v1/account_active_orders", get(handle_active_orders))
        .route("/api/v1/recent_trades", get(handle_recent_trades))
        .route("/api/v1/order_book_orders", get(handle_order_book_orders))
        .with_state(state)
}

async fn start_test_server() -> Result<(SocketAddr, TestServerState), Box<dyn std::error::Error + Send + Sync>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let state = TestServerState::new();
    let router = create_test_router(state.clone());

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Wait for server to start
    wait_for_server(addr).await;

    Ok((addr, state))
}

async fn wait_for_server(addr: SocketAddr) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    panic!("Test server did not start in time");
}

fn create_test_client(addr: SocketAddr) -> LighterRawHttpClient {
    let base_url = format!("http://{}", addr);
    LighterRawHttpClient::with_base_url(&base_url, None, Some(30), None, None)
        .expect("Failed to create test client")
}

// ------------------------------------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn test_get_markets() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let response: LighterResponse<LighterList<Market>> = client
        .get(ORDER_BOOKS, None)
        .await
        .expect("Request failed");

    assert!(response.success);
    assert!(response.data.is_some());

    let markets = response.data.unwrap();
    assert_eq!(markets.items.len(), 2);
    assert_eq!(markets.items[0].symbol, "ETH_USDC");
    assert_eq!(markets.items[0].market_index, 1);
    assert_eq!(markets.items[1].symbol, "BTC_USDC");
    assert_eq!(markets.items[1].market_index, 2);

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_get_next_nonce() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let response: NextNonceResponse = client
        .get(NEXT_NONCE, Some("account_index=878"))
        .await
        .expect("Request failed");

    assert!(response.success);
    assert!(response.data.is_some());
    assert_eq!(response.data.unwrap().nonce, 12345);

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_send_transaction() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let request_body = json!({
        "market_index": 1,
        "base_amount": 100000000,
        "price": 412750,
        "is_ask": false,
        "order_type": 1,
        "time_in_force": 0,
        "nonce": 12345,
        "signature": "0xabcdef"
    });

    let response: TxResponse = client
        .post(SEND_TX, Some(request_body.clone()))
        .await
        .expect("Request failed");

    assert!(response.success);
    assert!(response.data.is_some());

    let tx_data = response.data.unwrap();
    assert_eq!(tx_data.order_index, Some(42));
    assert!(tx_data.tx_id.is_some());

    // Verify request body was captured
    let captured_body = state.get_last_request_body().await;
    assert!(captured_body.is_some());
    assert_eq!(captured_body.unwrap()["market_index"], 1);

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_error_handling_markets() {
    let (addr, state) = start_test_server().await.unwrap();
    state.set_should_fail(true).await;

    let client = create_test_client(addr);

    let response: LighterResponse<LighterList<Market>> = client
        .get(ORDER_BOOKS, None)
        .await
        .expect("Request failed");

    assert!(!response.success);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("Internal server error"));
}

#[tokio::test]
async fn test_error_handling_send_tx() {
    let (addr, state) = start_test_server().await.unwrap();
    state.set_should_fail(true).await;

    let client = create_test_client(addr);

    let request_body = json!({
        "market_index": 1,
        "nonce": 12345
    });

    let response: TxResponse = client
        .post(SEND_TX, Some(request_body))
        .await
        .expect("Request failed");

    assert!(!response.success);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("insufficient balance"));
}

#[tokio::test]
async fn test_multiple_requests() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    // Make multiple requests
    for _ in 0..5 {
        let _: LighterResponse<LighterList<Market>> = client
            .get(ORDER_BOOKS, None)
            .await
            .expect("Request failed");
    }

    assert_eq!(state.get_request_count().await, 5);
}

#[tokio::test]
async fn test_get_candlesticks() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let query = "order_book_id=1&interval=1h";
    let response: Value = client
        .get(CANDLESTICKS, Some(query))
        .await
        .expect("Request failed");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["items"].is_array());
    assert_eq!(response["data"]["items"].as_array().unwrap().len(), 2);

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_get_account() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let response: Value = client
        .get(ACCOUNT, Some("account_index=878"))
        .await
        .expect("Request failed");

    assert!(response["success"].as_bool().unwrap());
    assert_eq!(response["data"]["account_index"].as_i64().unwrap(), 878);
    assert_eq!(response["data"]["balances"]["USDC"].as_str().unwrap(), "100000.00");

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_get_active_orders() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let response: Value = client
        .get(ACCOUNT_ACTIVE_ORDERS, Some("account_index=878"))
        .await
        .expect("Request failed");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["items"].is_array());

    let orders = response["data"]["items"].as_array().unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["order_id"].as_str().unwrap(), "12345");
    assert_eq!(orders[0]["status"].as_str().unwrap(), "open");

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_get_recent_trades() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let response: Value = client
        .get(RECENT_TRADES, Some("order_book_id=1&limit=10"))
        .await
        .expect("Request failed");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["items"].is_array());

    let trades = response["data"]["items"].as_array().unwrap();
    assert_eq!(trades.len(), 2);
    assert_eq!(trades[0]["price"].as_str().unwrap(), "4127.50");

    assert_eq!(state.get_request_count().await, 1);
}

#[tokio::test]
async fn test_get_order_book_orders() {
    let (addr, state) = start_test_server().await.unwrap();
    let client = create_test_client(addr);

    let response: Value = client
        .get(ORDER_BOOK_ORDERS, Some("order_book_id=1"))
        .await
        .expect("Request failed");

    assert!(response["success"].as_bool().unwrap());
    assert!(response["data"]["bids"].is_array());
    assert!(response["data"]["asks"].is_array());

    let bids = response["data"]["bids"].as_array().unwrap();
    let asks = response["data"]["asks"].as_array().unwrap();

    assert_eq!(bids.len(), 2);
    assert_eq!(asks.len(), 2);
    assert_eq!(response["data"]["market_index"].as_i64().unwrap(), 1);

    assert_eq!(state.get_request_count().await, 1);
}
