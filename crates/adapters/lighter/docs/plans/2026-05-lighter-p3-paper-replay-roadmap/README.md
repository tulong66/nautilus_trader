# Lighter P3 Paper/Replay Integration Roadmap

> Date: 2026-05-13
> Scope: Post-P2 paper/replay integration roadmap for the Lighter adapter
> Status: Draft; do not implement until P2-Q and P2-R are complete

## Purpose

P3 moves the Lighter adapter from adapter-internal fixture verification toward an operator-visible paper/replay integration path. P2 proves the adapter pieces in isolation: safety gate, private WebSocket dry-run, mock report source, reconciliation, sequencer semantics, retry/rate-limit policy, read-only risk inputs, and soak replay. P3 should prove that upper-layer paper/replay workflows can consume those adapter outputs deterministically.

This roadmap deliberately remains paper/replay only. It does not authorize real credentials, real private WebSocket connections, live order placement, live cancel, withdrawal, transfer, leverage, margin action, funding action, liquidation action, or production deployment.

## Inherited from existing docs

- `docs/STATUS_REPORT.md` P0/P1 — adopt: Python/PyO3/factory import smoke and offline execution fixtures/report builders are complete foundations.
- `docs/STATUS_REPORT.md` P2-J/P2-R — adopt: live signing is disabled by default; P2-R full verification must complete before P3 implementation begins.
- `docs/plans/2026-05-lighter-ws-transport-roadmap/` — adopt: B-core is implemented; B-full reconnect/replay/fault-injection/soak concerns remain relevant inputs, but P3 should not reopen real private credential flows.
- P2-L mock report integration — adopt: report API semantics can be tested through mock HTTP sources and must not silently fall back to fixture data on HTTP errors.
- P2-M/N reconciliation and sequencer semantics — adopt: replay accounting must distinguish submitted/accepted/executed/pending/timeout from filled, and must deduplicate duplicate/stale events.
- P2-O retry policy — adopt: transient retry/rate-limit behavior is deterministic and fail-fast paths must not hide auth/signing/config errors.
- P2-P risk inputs — adopt: funding/margin/liquidation are read-only model inputs, not account mutation actions.

## Frozen scope

### In scope for P3

- Build a paper/replay integration harness that consumes deterministic Lighter public/private fixtures, mock reports, and reconciliation outputs.
- Produce an operator audit summary for orders, fills, positions, account snapshots, report consistency, and replay anomalies.
- Add replay scenarios for disconnect/resubscribe, duplicate messages, stale updates, empty account, empty orders, report errors, retry exhaustion, and sequencer non-filled states.
- Keep all replay input local, fixture-backed, mock-backed, or in-memory.
- Document the safety-review gate required before any future testnet/private credential work.

### Out of scope for P3

- Real Lighter testnet/mainnet private WebSocket connection.
- Reading `.env`, wallet files, private key files, API key files, keyring, KMS, or credential managers.
- Real auth token generation from private credentials.
- Real order placement, cancel, cancel-all, withdrawal, transfer, leverage, margin, funding, liquidation, staking, sub-account, or other account mutation actions.
- Strategy optimization, market-making research, quote skew, or live signal execution.
- Production deployment or monitoring of live infrastructure.

### Deferred beyond P3

- Formal testnet credential lifecycle design after explicit safety review.
- B-full production reconnect/resubscribe state machine if it requires live credential assumptions.
- Signal-driven execution, maker/taker optimization, and quote skew research.
- Any real funds or private-key operational workflow.

## Roadmap summary

```text
P2 complete
  adapter-internal safety gates, dry-run WS, mock reports,
  reconciliation, sequencer semantics, retry policy,
  read-only risk inputs, soak verification

P3 paper/replay integration
  fixture/mock replay harness
  paper accounting consistency checks
  operator audit summary
  failure scenario replay pack
  safety-review entry criteria for any future testnet work

Post-P3 safety-reviewed live-prep
  only after explicit human approval and separate security review
```

## Proposed task board

| ID | Task | Status | Dependency | Acceptance standard |
|----|------|--------|------------|---------------------|
| P3-A | Paper/replay integration harness | [ ] | P2-R | Local harness replays public/private fixtures and mock reports into adapter execution/report/reconciliation components without network or credentials |
| P3-B | Operator audit summary | [ ] | P3-A | Deterministic audit output summarizes order lifecycle, fills, positions, account timestamps, report consistency, duplicates, stale events, and unresolved anomalies |
| P3-C | Failure scenario replay pack | [ ] | P3-A | Replay scenarios cover disconnect/resubscribe, duplicate messages, stale updates, empty account/orders, HTTP report error, retry exhaustion, and sequencer non-filled states |
| P3-D | Paper accounting consistency checks | [ ] | P3-A/P3-B | Replay verifies fills/positions/account/report snapshots remain internally consistent and records clear mismatch diagnostics |
| P3-E | Safety review entry criteria | [x] | P3-B/P3-C/P3-D | `safety-review-entry.zh-CN.md` defines what evidence is required before any future testnet/private credential design may start; does not approve live work |
| P3-F | P3 verification and docs | [ ] | P3-A-E | Full Rust tests, Python feature check, Python smoke when available, safety searches, and docs/status updates pass |

## Suggested execution workflow

- Wait for P2-Q and P2-R to complete before implementing P3.
- Use the established main-thread + coarse-grained background Sub-Agent workflow.
- Create one worktree per P3 task or per tightly coupled task pair.
- Require TDD for implementation tasks: failing replay/audit tests first, then minimal implementation.
- Agents should create local commits only; main thread performs review, verification, merge, and push.

## Verification baseline for P3

- Focused replay/audit tests for each task.
- `cargo +1.95.0 test -p nautilus-lighter`.
- `cargo +1.95.0 check -p nautilus-lighter --features python`.
- Python import smoke when the environment is available.
- Safety searches for secret loading, real private WS, and account mutation action paths.

## Safety boundary

P3 is still a paper/replay phase. Any task that needs real private credentials, real auth tokens, real account state from Lighter, live order placement, live cancel, withdrawal, transfer, leverage, margin action, funding action, liquidation action, staking, sub-account management, or production deployment must stop and go through a separate safety review.
