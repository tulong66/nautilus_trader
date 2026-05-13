# Lighter WebSocket Transport Roadmap Tasks

> Date: 2026-05-13
> Scope: B-core current tasks; B-full deferred roadmap

## MVP scope

B-core MVP is the only implementation scope for this task list: create the core WebSocket transport abstraction, keep live behavior backed by `tokio_tungstenite`, add deterministic scripted dry-run transport, prove private auth/subscription/order/account replay through shared session logic, and update status/docs. B-full items are recorded as deferred tasks only and must not be implemented under B-core unless this roadmap is updated first.

## Technical validation tasks

- [x] 0.1 Validate current WebSocket call sites
  - Confirm `LighterWebSocketClient::new_public`, `new_private`, `connect`, `subscribe`, `subscribe_account`, `close`, `is_running`, and `is_authenticated` call sites.
  - Confirm execution client still depends on P2-J `enable_live_signing` gate before private live connect.
  - _Requirements: RQ-1.1, RQ-NF-2_

- [x] 0.2 Validate parser support for private fixture messages
  - Confirm `InboundMessage::parse` supports `auth_success`, `authenticated`, `update/account_all`, and `update/account_all_orders` shapes intended for B-core tests.
  - Confirm `dispatch_private_message` handles `OrderUpdate` and `AccountUpdate` from those parsed variants.
  - _Requirements: RQ-3.2, RQ-3.4, RQ-3.5_

- [x] 0.3 Validate safety search baseline
  - Search Lighter adapter source for `std::env::var`, `dotenv`, `keyring`, `Keychain`, `SecretsManager`, `KMS`, `wallet`, `private key file`, and `.env` before implementation.
  - Record any existing non-secret comments separately from actual secret-loading code.
  - _Requirements: RQ-2.1, RQ-2.5, RQ-NF-3_

## B-core implementation tasks

- [x] 1. Add WebSocket transport boundary
  - Create or update `crates/adapters/lighter/src/websocket/transport.rs`.
  - Define minimal `WebSocketTransport` trait with `send_text`, `recv_text`, and `close`.
  - Export transport module from `websocket/mod.rs` as needed.
  - Keep trait narrow; do not add reconnect, metrics, fault injection, retry, or rate-limit policy in B-core.
  - _Requirements: RQ-1.2, RQ-1.4_

- [x] 2. Add live tungstenite transport adapter
  - Implement `TungsteniteWebSocketTransport` using current `tokio_tungstenite` split sink/stream behavior.
  - Map send/receive/close errors to `LighterError::WebSocket`.
  - Ensure auth token values are not logged.
  - _Requirements: RQ-1.1, RQ-NF-3, RQ-NF-4_

- [x] 3. Extract shared WebSocket session logic
  - Move auth send, subscription send, text receive, auth-success detection, parse, and message emission into shared session logic that operates on `dyn WebSocketTransport`.
  - Preserve existing `LighterWebSocketClient` public API where possible.
  - Preserve existing running/authenticated state semantics.
  - Keep heartbeat/reconnect behavior equivalent or explicitly mark unchanged portions for B-full if not fully moved.
  - _Requirements: RQ-1.1, RQ-3.1, RQ-3.2, RQ-3.3, RQ-NF-2_

- [x] 4. Add scripted dry-run transport
  - Implement deterministic scripted transport under test-only module or adapter-internal test support.
  - Support expected outbound auth/subscription frames and queued inbound frames.
  - Fail clearly on unexpected outbound frames without printing secret values.
  - Do not open sockets or read files.
  - _Requirements: RQ-1.2, RQ-2.1, RQ-2.2, RQ-2.3, RQ-NF-1, RQ-NF-3_

- [x] 5. Add P2-K private WS dry-run tests
  - Add focused tests for stub auth token frame, account/orders subscription flow, private order/account fixture replay, parse, and dispatch.
  - Use fixture data only.
  - Assert `DispatchOutcome::Order` and `DispatchOutcome::Account` values.
  - _Requirements: RQ-2.3, RQ-2.4, RQ-3.1, RQ-3.3, RQ-3.4, RQ-3.5, RQ-5.1_

- [x] 6. Preserve P2-J live signing gate
  - Ensure `LighterExecutionClient::connect()` still rejects default config before any HTTP/private WS/auth token work.
  - Ensure B-core dry-run tests do not require enabling live signing or generating real auth tokens.
  - _Requirements: RQ-2.5, RQ-5.5_

- [x] 7. Update documentation pointers and status
  - Update `docs/STATUS_REPORT.md` to point P2-K/R readers to `docs/plans/2026-05-lighter-ws-transport-roadmap/`.
  - Mark P2-K complete only after B-core tests pass.
  - Do not mark full P2-R complete unless J-Q verification is actually complete.
  - _Requirements: RQ-4.1, RQ-4.2, RQ-4.3, RQ-5.5_

## Verification tasks

### Verification results

- Focused WebSocket client tests: `cargo +1.95.0 test -p nautilus-lighter websocket::client::tests` → 6 passed.
- Focused private dry-run test: `cargo +1.95.0 test -p nautilus-lighter private_ws_dry_run_uses_scripted_transport_for_auth_subscribe_and_dispatch` → 1 passed.
- Execution dispatch integration: `cargo +1.95.0 test -p nautilus-lighter --test execution_dispatch` → 4 passed.
- P2-J gate: `cargo +1.95.0 test -p nautilus-lighter live_signing` → 3 unit tests passed plus 2 `signing_surface` filtered tests passed.
- Full crate: `cargo +1.95.0 test -p nautilus-lighter` → lib 134 passed / 2 ignored; integration tests 32 passed; doc-tests 1 passed / 3 ignored.
- Python feature check: `cargo +1.95.0 check -p nautilus-lighter --features python` → passed.
- Python smoke: `uv run --project /Volumes/HY2TB/projects/nautilus_trader-lighter --group test pytest /Volumes/HY2TB/projects/nautilus_trader-lighter/tests/integration_tests/adapters/lighter/test_imports.py -q` → 3 passed; uv emitted the existing `exclude-newer = "3 days"` parsing warning.
- Safety searches: no secret-loading implementation, real private WS credential use, or high-risk signing/execution path widening found; hits were existing environment fields, endpoint constants/docs/tests, deny-list tests, and data/report margin fields.

- [x] 8. Run focused B-core tests
  - Run `cargo +1.95.0 test -p nautilus-lighter private_ws` or the final focused test name.
  - Run `cargo +1.95.0 test -p nautilus-lighter execution_dispatch`.
  - Record exact pass/fail counts.
  - _Requirements: RQ-5.1_

- [x] 9. Run full Rust crate tests
  - Run `cargo +1.95.0 test -p nautilus-lighter`.
  - Record exact pass/fail/ignored counts.
  - _Requirements: RQ-5.2_

- [x] 10. Run Python feature check
  - Run `cargo +1.95.0 check -p nautilus-lighter --features python`.
  - Record result.
  - _Requirements: RQ-5.3_

- [x] 11. Run Python smoke when available
  - Run `uv run --project /Volumes/HY2TB/projects/nautilus_trader-lighter --group test pytest /Volumes/HY2TB/projects/nautilus_trader-lighter/tests/integration_tests/adapters/lighter/test_imports.py -q` when the environment is available.
  - If the environment fails for unrelated dependency reasons, record the exact failure and do not claim full verification.
  - _Requirements: RQ-5.4, RQ-5.5_

- [x] 12. Re-run safety searches
  - Search for secret-loading additions and high-risk signing path widening.
  - Confirm no `.env`, keyring, wallet, KMS, real private WS credential, withdraw, transfer, leverage, or margin action path was added.
  - _Requirements: RQ-2.1, RQ-2.5, RQ-NF-3_

## Deferred B-full tasks

- [ ] D1. Design reconnect/resubscribe state machine
  - Add explicit states for disconnected, connecting, authenticating, subscribed, draining, reconnecting, and closed.
  - Include idempotent resubscribe and duplicate subscription handling.
  - _Deferred requirements: RQ-4.2_

- [ ] D2. Add fault-injection and replay transports
  - Add latency, disconnect, malformed frame, duplicate frame, out-of-order frame, and backpressure scripts.
  - Integrate with P2-Q soak replay.
  - _Deferred requirements: RQ-4.2_

- [ ] D3. Integrate retry/rate-limit/latency model
  - Coordinate with P2-O.
  - Keep retry policy separate from basic transport send/receive operations.
  - _Deferred requirements: RQ-4.2_

- [ ] D4. Integrate execution reconciliation and sequencer semantics
  - Coordinate with P2-M/N.
  - Feed WebSocket updates and REST `sendTx` states into a shared state model.
  - _Deferred requirements: RQ-4.2_

- [ ] D5. Add credential/token lifecycle design under safety review
  - Only after explicit approval for real private credential design.
  - Include auth token refresh, expiry, secure storage boundary, and no-log guarantees.
  - _Deferred requirements: RQ-2.5_

## Dependencies

- Task 0.1 blocks tasks 1-3.
- Task 0.2 blocks task 5.
- Task 0.3 blocks tasks 4-7.
- Tasks 1 and 2 block task 3.
- Task 3 blocks tasks 4 and 5.
- Tasks 4 and 5 block verification tasks 8-12.
- Task 7 must run after implementation tests pass.

## Traceability matrix

| Requirement | Tasks |
|-------------|-------|
| RQ-1.1 | 0.1, 2, 3 |
| RQ-1.2 | 1, 4 |
| RQ-1.3 | 7 |
| RQ-1.4 | 1 |
| RQ-2.1 | 0.3, 4, 12 |
| RQ-2.2 | 4 |
| RQ-2.3 | 4, 5 |
| RQ-2.4 | 5 |
| RQ-2.5 | 0.3, 6, 12, D5 |
| RQ-3.1 | 3, 5 |
| RQ-3.2 | 0.2, 3 |
| RQ-3.3 | 3, 5 |
| RQ-3.4 | 0.2, 5 |
| RQ-3.5 | 0.2, 5 |
| RQ-4.1 | 7 |
| RQ-4.2 | 7, D1, D2, D3, D4 |
| RQ-4.3 | 7 |
| RQ-4.4 | 7 |
| RQ-5.1 | 5, 8 |
| RQ-5.2 | 9 |
| RQ-5.3 | 10 |
| RQ-5.4 | 11 |
| RQ-5.5 | 7, 11 |
| RQ-NF-1 | 4 |
| RQ-NF-2 | 0.1, 3 |
| RQ-NF-3 | 2, 4, 12 |
| RQ-NF-4 | 2 |

## Consistency audit

- Every B-core requirement has at least one implementation or verification task.
- Every B-full concept is explicitly deferred and is not part of the MVP scope.
- New modules have runtime locations in `design.zh-CN.md`.
- No requirement authorizes real credentials, real private WS, or live trading.
- P2-R completion remains conditional on actual verification results.
