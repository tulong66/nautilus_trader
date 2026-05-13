# Lighter P3 Paper/Replay Integration Requirements

> Date: 2026-05-13
> Status: Draft; gated on P2-Q and P2-R completion

## 继承与边界

P3 继承 P0/P1/P2 的 adapter 内部能力，但目标从“模块内可测”推进到“paper/replay 工作流可消费”。P3 不改变安全边界：不读取 secrets，不连接真实 private WebSocket，不生成真实 auth token，不下单/撤单/提现/转账，不实现任何 leverage/margin/funding/liquidation 账户动作。

## User stories

### US-1: Paper/replay harness

As an adapter maintainer, I want a deterministic paper/replay harness for Lighter fixtures and mock reports, so that I can verify adapter outputs without live credentials or network access.

Acceptance criteria:

1. WHEN the replay harness runs with local public/private fixtures THEN it SHALL process the scenario without opening sockets or reading credential files.
2. WHEN the replay harness receives private order/account fixture messages THEN it SHALL feed them through the same parse/dispatch/reconciliation path used by adapter tests.
3. WHEN the replay harness consumes mock report responses THEN it SHALL compare report snapshots with replayed order/fill/position/account state.
4. IF a replay scenario references a real endpoint, credential file, keyring, KMS, or private key THEN the harness SHALL fail configuration validation before execution.

### US-2: Operator audit summary

As an operator reviewing a replay run, I want a concise audit summary, so that I can see order lifecycle, fill, position, account, report, and anomaly status without inspecting raw logs.

Acceptance criteria:

1. WHEN a replay completes THEN the system SHALL emit a deterministic summary of orders, fills, positions, account timestamps, report consistency, duplicate events, stale events, and unresolved anomalies.
2. WHEN no anomalies are found THEN the audit SHALL explicitly state that no unresolved anomalies were detected.
3. WHEN inconsistencies exist THEN the audit SHALL identify the affected client order ID, venue order ID, account, report type, or scenario step.
4. WHEN the same replay input is run twice THEN the audit output SHALL be stable except for explicitly controlled metadata such as scenario name or fixture version.

### US-3: Failure scenario replay pack

As a maintainer, I want a replay pack covering known failure modes, so that regressions in reconnect, duplicate handling, report errors, retry exhaustion, and sequencer semantics are caught before any live-prep work.

Acceptance criteria:

1. WHEN a disconnect/resubscribe scenario is replayed THEN the system SHALL preserve intended subscriptions and avoid double-counting replayed events.
2. WHEN duplicate messages are replayed THEN reconciliation SHALL mark duplicates deterministically and preserve final order/account state.
3. WHEN stale updates are replayed after newer terminal states THEN reconciliation SHALL mark stale updates and preserve the terminal state.
4. WHEN empty account, empty order, empty fill, or empty position responses are replayed THEN reports SHALL return empty deterministic results rather than fixture fallback surprises.
5. WHEN mock report HTTP errors are replayed THEN the system SHALL expose the error and SHALL NOT silently fall back to offline fixture reports.
6. WHEN retry exhaustion or fail-fast classification is replayed THEN the audit SHALL record the final classification without hiding auth/config/signing errors.
7. WHEN sendTx returns submitted/accepted/executed/pending/timeout without fill replay THEN the final order state SHALL NOT be reported as filled.

### US-4: Paper accounting consistency

As a maintainer, I want paper accounting consistency checks, so that replayed fills, positions, account snapshots, and reports can be compared before higher-level strategy integration.

Acceptance criteria:

1. WHEN fills are replayed THEN paper position quantities SHALL match the accumulated fill quantities for the fixture scenario.
2. WHEN positions are replayed from reports THEN they SHALL be compared with replay-derived position expectations.
3. WHEN account snapshots are replayed THEN account timestamps SHALL be monotonic per account unless the event is explicitly classified as stale.
4. IF replay-derived and report-derived state disagree THEN the check SHALL emit a deterministic mismatch diagnostic.

### US-5: Safety review entry criteria

As a project owner, I want explicit criteria before any future testnet/private credential design, so that paper/replay completion cannot be mistaken for live authorization.

Acceptance criteria:

1. WHEN P3 docs are updated THEN they SHALL state that P3 does not approve live trading or real credential usage.
2. WHEN future testnet work is proposed THEN docs SHALL require separate security review, credential lifecycle design, no-log guarantees, and explicit human approval.
3. WHEN P3 verification completes THEN safety searches SHALL confirm no secret-loading, live private WS, or account mutation path was added.

## Non-functional requirements

- Replay tests SHALL be deterministic and fast enough for normal crate test runs.
- Replay scenarios SHALL avoid wall-clock long sleeps; use scripted/in-memory events instead.
- Audit summaries SHALL avoid printing tokens, keys, credentials, or private material.
- New code SHALL prefer existing fixture, dispatch, reconciliation, report, retry, and scripted transport components over new abstractions.
