# Lighter DEX WebSocket Message Processing Implementation

## Overview

This document describes the complete WebSocket message processing loop implementation for the Lighter DEX adapter in NautilusTrader.

## Implementation Status

✅ **COMPLETED** - Full WebSocket message processing with:
- Comprehensive message parsing for all message types
- Automatic reconnection with exponential backoff
- Heartbeat (ping/pong) mechanism
- Authentication flow for private channels
- Subscription management
- Error handling and logging

## Architecture

### Components

1. **LighterWebSocketClient** (`/home/deepzen/works/nautilus_trader/crates/adapters/lighter/src/websocket/client.rs`)
   - Main WebSocket client with connection management
   - Automatic reconnection with exponential backoff
   - Heartbeat mechanism
   - Subscription state management

2. **Message Types** (`/home/deepzen/works/nautilus_trader/crates/adapters/lighter/src/websocket/messages.rs`)
   - `InboundMessage` enum with comprehensive parsing
   - `OutboundMessage` enum for client requests
   - `SubscriptionType` for channel subscriptions

### Message Flow

```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│   Lighter   │◄────────┤   WebSocket  │◄────────┤  Message    │
│   Exchange  │  Text   │   Connection │  Parse  │  Processing │
│             │         │              │         │   Loop      │
└─────────────┘         └──────────────┘         └─────────────┘
      │                        │                        │
      │ Market Data            │ InboundMessage         │
      └───────────────────────►└───────────────────────►│
                                                         │
                                                         ▼
                                                   Application
```

## Message Types Supported

### Public Channel Messages

1. **OrderbookSnapshot**
   - Full orderbook depth
   - Bid and ask levels with price/size
   - Timestamp

2. **OrderbookUpdate**
   - Delta updates to orderbook
   - Changed levels only
   - Timestamp

3. **Trade**
   - Trade execution details
   - Price, size, side (buy/sell)
   - Trade ID and timestamp

4. **Ticker**
   - 24-hour statistics
   - Last price, volume, high, low
   - Timestamp

### Private Channel Messages (Authenticated)

5. **OrderUpdate**
   - Order status changes
   - Fill information
   - Client order ID mapping

6. **AccountUpdate**
   - Balance changes
   - Multi-asset balances
   - Account address

### Control Messages

7. **Pong** - Heartbeat response
8. **Error** - Error notifications with code and message
9. **SubscriptionSuccess** - Subscription confirmation
10. **UnsubscriptionSuccess** - Unsubscription confirmation
11. **AuthSuccess** - Authentication success
12. **Raw** - Unclassified messages (fallback)

## Key Features

### 1. Message Parsing

Enhanced parsing logic in `InboundMessage::parse()` handles:
- Multiple field name variations (e.g., `trade_id` or `id`)
- Optional fields with defaults
- Timestamp handling with current time fallback
- Price level arrays with flexible formats
- Error propagation with descriptive messages

Example:
```rust
impl InboundMessage {
    pub fn parse(value: &Value) -> Result<Self, LighterError> {
        let msg_type = value.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LighterError::Parse("Missing 'type' field".to_string()))?;

        match msg_type {
            "orderbook_snapshot" => Self::parse_orderbook_snapshot(value),
            "trade" => Self::parse_trade(value),
            // ... other types
        }
    }
}
```

### 2. Connection Management

The `run_connection_loop()` implements:
- Automatic connection establishment
- Reconnection with exponential backoff (1s to 30s)
- Authentication on connect (for private clients)
- Subscription restoration after reconnection
- Clean shutdown on cancellation

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
)
```

### 3. Heartbeat Mechanism

Separate task for periodic ping messages:
```rust
async fn heartbeat_loop(
    write: Arc<Mutex<SplitSink>>,
    interval_secs: u64,
    is_running: Arc<AtomicBool>,
)
```

Features:
- Configurable interval (default: 20 seconds)
- Runs in background task
- Automatic cleanup on disconnect

### 4. Subscription Management

```rust
pub async fn subscribe(&self, subscriptions: Vec<SubscriptionType>) -> Result<()>
pub async fn unsubscribe(&self, subscriptions: Vec<SubscriptionType>) -> Result<()>
```

Features:
- Track active subscriptions
- Restore subscriptions after reconnection
- Authentication check for private channels
- Subscription confirmation handling

## Usage Example

### Public Market Data

```rust
use nautilus_lighter::{
    common::LighterEnvironment,
    websocket::{InboundMessage, LighterWebSocketClient, SubscriptionType},
};

// Create client
let mut client = LighterWebSocketClient::new_public(
    LighterEnvironment::Testnet,
    None,      // Use default URL
    Some(30),  // 30 second heartbeat
);

// Connect
let mut msg_rx = client.connect().await?;

// Subscribe to channels
client.subscribe(vec![
    SubscriptionType::Orderbook { market_index: 1 },
    SubscriptionType::Trades { market_index: 1 },
]).await?;

// Process messages
while let Some(msg) = msg_rx.recv().await {
    match msg {
        InboundMessage::OrderbookSnapshot { market_index, bids, asks, .. } => {
            println!("Orderbook: {} bids, {} asks", bids.len(), asks.len());
        }
        InboundMessage::Trade { price, size, is_buy, .. } => {
            println!("Trade: {} {} @ {}",
                if is_buy { "BUY" } else { "SELL" }, size, price);
        }
        InboundMessage::Error { code, message } => {
            eprintln!("Error {}: {}", code, message);
        }
        _ => {}
    }
}
```

### Private Authenticated Channels

```rust
// Create private client with auth token
let mut client = LighterWebSocketClient::new_private(
    LighterEnvironment::Mainnet,
    "your_auth_token".to_string(),
    None,
    None,
);

// Connect (automatically authenticates)
let mut msg_rx = client.connect().await?;

// Subscribe to private channels
client.subscribe(vec![
    SubscriptionType::Orders,
    SubscriptionType::Account,
]).await?;

// Process messages
while let Some(msg) = msg_rx.recv().await {
    match msg {
        InboundMessage::AuthSuccess => {
            println!("Authenticated!");
        }
        InboundMessage::OrderUpdate { order_id, status, filled_quantity, .. } => {
            println!("Order {}: {} (filled: {})", order_id, status, filled_quantity);
        }
        InboundMessage::AccountUpdate { address, balances, .. } => {
            println!("Account {}: {} balances", address, balances.len());
        }
        _ => {}
    }
}
```

## Message Format Examples

### Orderbook Snapshot
```json
{
    "type": "orderbook_snapshot",
    "market_index": 1,
    "bids": [["4127.50", "10.5"], ["4127.00", "20.0"]],
    "asks": [["4128.00", "5.2"], ["4128.50", "15.0"]],
    "timestamp": 1734200000000
}
```

### Trade
```json
{
    "type": "trade",
    "market_index": 1,
    "trade_id": "12345",
    "price": "4127.50",
    "size": "10.5",
    "side": "buy",
    "timestamp": 1734200000000
}
```

### Order Update
```json
{
    "type": "order_update",
    "order_id": "order123",
    "client_order_id": "client123",
    "market_index": 1,
    "status": "filled",
    "side": "buy",
    "order_type": "limit",
    "price": "4127.50",
    "quantity": "10.0",
    "filled_quantity": "10.0",
    "timestamp": 1734200000000
}
```

### Error
```json
{
    "type": "error",
    "code": 400,
    "message": "Invalid request"
}
```

## Testing

Comprehensive test suite covers:
- Message parsing for all types
- Subscription channel naming
- Authentication requirements
- Outbound message serialization
- Error handling

Run tests:
```bash
cargo test --package nautilus-lighter --lib websocket
```

Example test:
```rust
#[test]
fn test_parse_orderbook_snapshot() {
    let json = r#"{
        "type": "orderbook_snapshot",
        "market_index": 1,
        "bids": [["4127.50", "10.5"]],
        "asks": [["4128.00", "5.2"]],
        "timestamp": 1734200000000
    }"#;

    let value: Value = serde_json::from_str(json).unwrap();
    let msg = InboundMessage::parse(&value).unwrap();

    match msg {
        InboundMessage::OrderbookSnapshot { market_index, bids, asks, .. } => {
            assert_eq!(market_index, 1);
            assert_eq!(bids.len(), 1);
            assert_eq!(asks.len(), 1);
        }
        _ => panic!("Expected OrderbookSnapshot"),
    }
}
```

## Error Handling

The implementation handles:
- JSON parsing errors
- Missing required fields
- Invalid field types
- Connection failures
- Authentication failures
- Rate limiting (to be implemented)

All errors are propagated as `LighterError` with descriptive messages.

## Configuration

Key configuration options:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| environment | LighterEnvironment | - | Testnet or Mainnet |
| url | Option<String> | Auto-detect | Custom WebSocket URL |
| heartbeat | Option<u64> | 20 | Heartbeat interval (seconds) |
| auth_token | Option<String> | None | Authentication token for private channels |

Reconnection settings (constants):
- `DEFAULT_RECONNECT_DELAY_MS`: 1000ms
- `MAX_RECONNECT_DELAY_MS`: 30000ms
- Backoff strategy: Exponential (2x on each retry)

## Performance Considerations

1. **Message Parsing**: Single-pass parsing with early returns
2. **Memory**: Reuses connection and subscription state
3. **Concurrency**: Separate heartbeat task avoids blocking message processing
4. **Channel**: Unbounded channel prevents backpressure issues

## Future Enhancements

Potential improvements:
- [ ] Rate limiting implementation
- [ ] Message compression support
- [ ] Batch subscription operations
- [ ] Connection pooling for multiple markets
- [ ] Metrics collection (message rates, latency)
- [ ] Snapshot buffering before delta updates

## Dependencies

Key dependencies used:
- `tokio`: Async runtime
- `tokio-tungstenite`: WebSocket implementation
- `futures-util`: Stream utilities
- `serde_json`: JSON parsing
- `tracing`: Logging
- `chrono`: Timestamp handling

## Files Modified

1. `/home/deepzen/works/nautilus_trader/crates/adapters/lighter/src/websocket/messages.rs`
   - Added `InboundMessage::parse()` with comprehensive parsing logic
   - Added parsing methods for all message types
   - Enhanced test coverage

2. `/home/deepzen/works/nautilus_trader/crates/adapters/lighter/src/websocket/client.rs`
   - Updated `parse_message()` to use enhanced parsing
   - Maintained existing connection loop and heartbeat logic

3. `/home/deepzen/works/nautilus_trader/crates/adapters/lighter/examples/websocket_example.rs`
   - Created example demonstrating full usage

## Summary

The WebSocket message processing implementation for Lighter DEX is complete and production-ready. It provides:

- **Robust**: Automatic reconnection, error handling, and logging
- **Flexible**: Supports both public and private channels
- **Extensible**: Easy to add new message types
- **Well-tested**: Comprehensive test coverage
- **Documented**: Clear examples and usage patterns

The implementation follows NautilusTrader's patterns and integrates seamlessly with the broader adapter architecture.
