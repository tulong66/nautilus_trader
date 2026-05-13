# Lighter P3 Paper/Replay Integration Design

> Date: 2026-05-13
> Status: Draft; gated on P2-Q and P2-R completion

## Overview

P3 adds a paper/replay integration layer around the Lighter adapter's already-tested components. P2 validates the adapter internals: fixture dispatch, report builders, mock report source, reconciliation, sendTx semantics, retry policy, risk input parsing, and soak replay. P3 should orchestrate those components into deterministic replay scenarios and produce operator-readable audit summaries.

The design remains local-only. It uses fixtures, mock HTTP responses, and scripted/in-memory replay inputs. It does not introduce real private credential loading, real private WebSocket connections, real auth token generation, or account mutation actions.

## Architecture

```text
P3 replay scenario
  ├─ fixture event stream
  │   ├─ public market events
  │   └─ private order/account/fill events
  ├─ mock report source
  │   ├─ order reports
  │   ├─ fill reports
  │   ├─ position reports
  │   └─ mass status
  ├─ replay runner
  │   ├─ dispatch_private_message
  │   ├─ ExecutionReconciler
  │   ├─ retry/fail-fast classification inputs
  │   └─ report consistency checks
  └─ audit summary
      ├─ lifecycle results
      ├─ accounting consistency
      ├─ duplicates/stale events
      └─ unresolved anomalies
```

## Components

### Replay scenario model

A replay scenario is a deterministic in-memory description of:

- scenario name and fixture version;
- ordered private/public messages;
- optional mock report pages or errors;
- optional retry/fail-fast events;
- expected final order/account/report outcomes.

Runtime location: Rust adapter test/support code under `crates/adapters/lighter/src/execution` or integration tests. It should not read external files unless a later task explicitly adds checked-in fixtures.

### Replay runner

The runner applies scenario steps to existing adapter components:

1. parse or construct fixture `InboundMessage` values;
2. call `dispatch_private_message` for private execution messages;
3. apply outcomes to `ExecutionReconciler`;
4. collect report outputs from mock/offline report source;
5. classify retry/fail-fast events using existing retry policy;
6. produce replay result data for audit.

Runtime location: Rust adapter tests or internal execution replay module. It must not open network sockets.

### Audit summary

The audit summary is a stable data structure and optional string/table formatter. It records:

- scenario name;
- counts of accepted, rejected, partially filled, filled, canceled, cancel rejected, submitted, pending, timeout, duplicate, and stale outcomes;
- account timestamp summary;
- report consistency summary;
- mismatch diagnostics;
- safety mode indicator such as `paper_replay_only`.

Runtime location: Rust adapter execution/replay module or tests. If exposed later to Python, that should be a separate task.

### Consistency checker

The checker compares replay-derived state against report-derived state:

- order status by client order ID and venue order ID;
- fills by trade ID and order ID;
- positions by instrument/market index;
- account timestamps by address;
- empty responses and HTTP error semantics.

Mismatch output should be deterministic and targeted; it should not dump raw fixtures unless needed for a test assertion.

## Error handling

- Invalid scenario configuration fails before replay starts.
- Real endpoint/credential references are rejected as configuration errors.
- Mock report HTTP errors remain errors and are included in audit diagnostics; no silent fixture fallback.
- Duplicate and stale messages are audit events, not panics.
- Auth/config/signing fail-fast classifications remain fail-fast and must not be hidden by retry exhaustion.

## Testing strategy

### Focused tests

- Happy-path replay produces no unresolved anomalies.
- Empty account/order/fill/position responses produce deterministic empty reports.
- Duplicate messages are counted as duplicates and do not change final state.
- Stale updates after terminal states are counted as stale and do not regress state.
- Mock report errors are surfaced and do not fall back to fixtures.
- Sequencer submitted/accepted/executed/pending/timeout states do not become filled without fill replay.
- Retry exhaustion and auth/config fail-fast classifications are reflected in audit output.

### Verification commands

- `cargo +1.95.0 test -p nautilus-lighter replay`
- `cargo +1.95.0 test -p nautilus-lighter audit`
- `cargo +1.95.0 test -p nautilus-lighter`
- `cargo +1.95.0 check -p nautilus-lighter --features python`
- Python import smoke when environment is available
- Safety searches for secrets, private WS, and account mutation paths

## Safety boundary

The P3 design proves paper/replay integration only. It is not a credential lifecycle design and not a testnet authorization. Any future live credential or real private WebSocket task must start from a separate safety-reviewed design.
