# Lighter WebSocket Transport Roadmap

> Date: 2026-05-13
> Scope: P2-K/R WebSocket transport abstraction roadmap for Lighter adapter
> Status: B-core implemented and verified; B-full deferred

## Purpose

This package preserves the roadmap for moving Lighter private WebSocket dry-run support from the current direct `tokio_tungstenite::connect_async` implementation toward a production-grade transport abstraction.

The immediate implementation target is **B-core**: introduce the core transport boundary and scripted dry-run transport needed for P2-K, without pulling P2-O/Q retry, rate-limit, reconnect soak, or live credential flows into this phase.

The final architecture target is **B-full**: all live, dry-run, replay, soak, and future mock transports share the same WebSocket session protocol logic through an injectable transport interface.

## Documents

- [requirements.zh-CN.md](requirements.zh-CN.md) — user stories and EARS acceptance criteria.
- [design.zh-CN.md](design.zh-CN.md) — architecture, components, data flow, error handling, and test strategy.
- [tasks.zh-CN.md](tasks.zh-CN.md) — implementation tasks with requirement traceability and verification evidence.
- [implementation-plan.zh-CN.md](implementation-plan.zh-CN.md) — executable TDD implementation plan for B-core.

## Inherited from existing docs

- `docs/STATUS_REPORT.md` P1 — adopt: offline execution fixtures, private order/account dispatch, fixture-backed reports, and safety surface audit are already complete.
- `docs/STATUS_REPORT.md` P2 — adopt: P2-J live signing gate is complete; P2-K must use fixture/stub auth and private channel replay without reading `.env` or connecting real private WS.
- `docs/STATUS_REPORT.md` P2-R — adopt: verification must include `cargo +1.95.0 test -p nautilus-lighter`, `cargo +1.95.0 check -p nautilus-lighter --features python`, and Python smoke when the environment is available.
- `docs/COMPARISON_AND_MERGE.md` — adopt: pure Rust signing is the current decision; do not reintroduce FFI signer assumptions.
- `docs/COMPARISON_AND_MERGE.md` and `docs/INTEGRATED_PLAN.md` — adopt: dynamic market precision, REST+WebSocket dual confirmation, per-API-key nonce, JSON application heartbeat, and mock-friendly transport abstractions remain important production concerns.
- `docs/INTEGRATED_PLAN.md` — modify: earlier `HttpTransport` abstraction idea is extended to WebSocket transport; this roadmap does not rewrite HTTP.
- `DEVELOPMENT_PLAN.md` — defer: auth token refresh and true private channel credential lifecycle are not part of B-core; they remain future live-safety work after P2-K/R.
- `docs/IMPLEMENTATION_PLAN.md` — defer: full reconnect state restoration and testnet end-to-end flows are B-full/P2-O/Q work, not B-core.

## Frozen scope

### In scope for B-core

- Define a WebSocket transport abstraction at the Lighter adapter WebSocket layer.
- Keep the live transport backed by `tokio_tungstenite`.
- Add scripted/in-memory transport for deterministic dry-run tests.
- Move auth, subscribe, read, parse, and dispatch validation onto shared session logic that can run with either live or scripted transport.
- Prove P2-K with stub auth token and fixture private order/account messages.
- Update local docs/status pointers so future sessions know B-full is the final target.

### Out of scope for B-core

- Real Lighter testnet/mainnet private WebSocket connection.
- Reading `.env`, wallet files, key files, API keys, or credential managers.
- Generating real auth tokens from private credentials in dry-run tests.
- Real order placement, real cancel, real cancel-all, withdraw, transfer, funding, margin, or liquidation actions.
- Full reconnect policy redesign, retry/rate-limit model, long soak replay, execution reconciliation state machine, and sendTx sequencer semantics.

### Deferred to B-full

- Reconnect/resubscribe state restoration as a first-class state machine.
- Transport-level fault injection for latency, disconnects, duplicate messages, malformed frames, and backpressure.
- Replay/soak transports for long deterministic public/private fixture runs.
- Unified dry-run/live/session telemetry and metrics.
- Integration with P2-M/N/O/Q state reconciliation and soak verification.

## Roadmap summary

```text
Current
  direct connect_async inside LighterWebSocketClient
  private dry-run not isolated enough for production-grade tests

B-core
  WebSocketTransport trait
  Tungstenite live transport
  Scripted dry-run transport
  shared auth/subscribe/read/parse protocol path
  P2-K fixture/stub validation

B-full
  reconnect/resubscribe state machine
  replay/soak/fault-injection transports
  latency/retry/rate-limit integration
  execution reconciliation and sequencer semantics integration
```

## Safety boundary

This roadmap does not authorize real private credentials or live trading. Any task that needs real private WS credentials, real auth tokens, real account state, live order placement, live cancel, withdrawal, transfer, funding/margin action, or production deployment must stop and go through a separate safety review.
