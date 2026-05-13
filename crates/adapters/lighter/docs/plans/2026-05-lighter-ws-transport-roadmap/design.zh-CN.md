# Lighter WebSocket Transport Roadmap Design

> Date: 2026-05-13
> Scope: B-core current implementation target; B-full deferred production evolution

## Inherited from existing docs

- `docs/STATUS_REPORT.md` P1/P2 — adopt: existing fixture/dispatch/report layers and P2-J signing gate are foundations; B-core must not expand live signing or credential access.
- `docs/COMPARISON_AND_MERGE.md` — adopt: pure Rust signing remains the active design; no Go FFI signer work is introduced.
- `docs/INTEGRATED_PLAN.md` — adopt/modify: mock-friendly transport abstraction is extended from HTTP idea to WebSocket I/O.
- `DEVELOPMENT_PLAN.md` and `docs/IMPLEMENTATION_PLAN.md` — defer: auth token refresh, reconnect state recovery, and testnet/private end-to-end behavior belong to B-full or later safety-reviewed live work.

## Overview

The current `LighterWebSocketClient` owns WebSocket protocol behavior and raw I/O in the same implementation: it calls `tokio_tungstenite::connect_async`, splits the stream, sends auth/subscription messages, handles heartbeat, parses incoming text, and emits `InboundMessage` values.

B-core introduces a WebSocket transport boundary without changing the safety posture. Live execution still uses a `tokio_tungstenite` transport. Dry-run tests use a scripted transport that sends and receives deterministic text frames in memory. Auth, subscribe, parse, and private dispatch validation move onto shared session logic so tests cover the same protocol behavior as live code.

B-full remains the final architecture target. B-full will extend the transport boundary with reconnect/resubscribe state machines, fault injection, latency/retry/rate-limit models, soak replay, and execution reconciliation integration. B-core deliberately avoids those broader concerns.

## Architecture

```text
LighterWebSocketClient
  ├─ LighterWebSocketSession
  │   ├─ auth/send protocol
  │   ├─ subscription protocol
  │   ├─ text frame receive loop
  │   ├─ InboundMessage::parse
  │   └─ mpsc<InboundMessage>
  ├─ Live transport factory
  │   └─ TungsteniteWebSocketTransport
  └─ Test/dry-run transport factory
      └─ ScriptedWebSocketTransport
```

### Runtime locations

- `LighterWebSocketClient`: Rust adapter process, used by data and execution clients.
- `LighterWebSocketSession`: Rust adapter process, internal to `crates/adapters/lighter/src/websocket`.
- `TungsteniteWebSocketTransport`: Rust adapter process, live mode only, backed by `tokio_tungstenite`.
- `ScriptedWebSocketTransport`: test-only or adapter-internal dry-run module; no external network.
- `dispatch_private_message`: existing Rust execution dispatch layer; B-core reuses it in integration tests.

## Components and interfaces

### WebSocket transport trait

B-core needs the smallest useful boundary:

```rust
#[async_trait]
pub trait WebSocketTransport: Send {
    async fn send_text(&mut self, text: String) -> Result<(), LighterError>;
    async fn recv_text(&mut self) -> Result<Option<String>, LighterError>;
    async fn close(&mut self) -> Result<(), LighterError>;
}
```

The trait intentionally does not model reconnect, ping/pong policy, binary frames, backpressure metrics, or fault injection in B-core. Those are B-full concerns.

### Live transport

`TungsteniteWebSocketTransport` wraps the existing split sink/stream produced by `tokio_tungstenite::connect_async`. It preserves the current live behavior and maps tungstenite send/receive errors to `LighterError::WebSocket`.

### Scripted transport

`ScriptedWebSocketTransport` is deterministic. It contains:

- expected outbound text frames or predicates;
- inbound text frame queue;
- observed outbound frame log for assertions;
- terminal close/error state.

It MUST NOT open sockets, read files, or load credentials. It is used to prove P2-K auth/subscription/order/account flow through the shared session logic.

### Session logic

`LighterWebSocketSession` owns protocol behavior that should be shared:

1. if private, send `OutboundMessage::Auth { token }`;
2. read `auth_success`/`authenticated` and set authenticated state;
3. send subscriptions via `OutboundMessage::subscribe` or future account-aware format;
4. parse text frames using `InboundMessage::parse` and the existing auth pre-check behavior;
5. emit `InboundMessage` through `mpsc`.

The session should avoid logging tokens. If auth/subscription fails, errors should include operation context but not secret content.

## Data flow

### Live mode

```text
LighterWebSocketClient::connect()
  → Tungstenite transport factory connects URL
  → session sends auth if private
  → session resubscribes remembered channels
  → session reads text frames
  → InboundMessage::parse
  → mpsc receiver consumed by LighterExecutionClient/DataClient
```

### Dry-run mode

```text
private_ws_dry_run test
  → ScriptedWebSocketTransport with stub token expectations
  → shared session sends auth/subscriptions
  → scripted transport emits fixture auth/subscribed/order/account JSON
  → InboundMessage::parse
  → dispatch_private_message
  → deterministic DispatchOutcome assertions
```

## Decisions

### Decision: B-core instead of C-lite

**Context:** C-lite local mock server is closer to socket-level behavior but would still be a transitional harness. The user wants the roadmap preserved and prefers investing in production-grade direction when feasible.

**Options considered:**

1. C-lite local mock WebSocket server — close to network behavior; lower refactor cost; still transitional.
2. B-core transport abstraction — higher refactor cost; creates production architecture spine.
3. B-full now — maximum completeness; high risk of scope expansion into P2-O/Q/M/N.

**Decision:** Implement B-core first, document B-full as the final target.

**Rationale:** B-core avoids throwaway C-lite work while keeping scope bounded. It gives P2-K a deterministic dry-run harness and preserves the path to production-grade reconnect/replay/soak later.

### Decision: minimal transport trait

**Context:** A broad trait with reconnect, fault injection, ping policy, metrics, and backpressure would pull B-full into B-core.

**Decision:** B-core transport exposes only `send_text`, `recv_text`, and `close`.

**Rationale:** These operations are sufficient for auth/subscription/read/parse P2-K validation. B-full can extend or wrap this boundary later.

### Decision: no credential provider in B-core

**Context:** P2-J intentionally keeps live signing disabled by default. P2-K must not read real secrets.

**Decision:** B-core dry-run uses explicit stub token only. Real token generation/refresh is deferred.

**Rationale:** This preserves the safety boundary and avoids accidental private WS enablement.

## Error handling

- Transport connection errors map to `LighterError::WebSocket`.
- Send/receive errors include operation context such as `send auth`, `send subscribe`, or `receive frame`, but never token value.
- Scripted transport expectation mismatch fails tests with expected operation/channel names, not secrets.
- Missing auth token in a private live session remains an auth error.
- Unknown inbound message types continue to map to `InboundMessage::Raw` unless existing parser returns a parse error.

## Testing strategy

### Unit tests

- Transport trait live wrapper compile coverage.
- Scripted transport records outbound frames and returns inbound frames in order.
- Scripted transport reports expectation mismatches.
- Auth token is never printed in debug/error strings.

### Integration tests

- P2-K private dry-run sends auth frame with stub token through scripted transport.
- P2-K private dry-run sends account/order subscription frames after auth success.
- P2-K private dry-run parses fixture order/account update JSON into `InboundMessage` variants.
- P2-K private dry-run dispatches parsed private messages into deterministic `DispatchOutcome` values.
- Existing `execution_dispatch`, `execution_fixtures`, and `signing_surface` tests remain green.

### Verification commands

- `cargo +1.95.0 test -p nautilus-lighter private_ws`
- `cargo +1.95.0 test -p nautilus-lighter execution_dispatch`
- `cargo +1.95.0 test -p nautilus-lighter`
- `cargo +1.95.0 check -p nautilus-lighter --features python`
- Python import smoke when environment is available.

## B-full deferred design notes

B-full should add:

- reconnect/resubscribe state machine;
- scriptable disconnect/reconnect/fault injection;
- replay transport for long public/private fixture soak;
- latency/retry/rate-limit model hooks;
- execution reconciliation integration with duplicate/out-of-order messages;
- sendTx accepted/submitted/executed/rejected semantics integration;
- metrics/tracing around connection lifecycle and transport health.

These are intentionally not B-core tasks. They should be planned under P2-M/N/O/Q or a later B-full roadmap update.
