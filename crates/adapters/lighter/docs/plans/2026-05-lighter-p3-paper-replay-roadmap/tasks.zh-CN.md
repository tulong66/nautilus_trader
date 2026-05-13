# Lighter P3 Paper/Replay Integration Tasks

> Date: 2026-05-13
> Status: Draft; do not execute until P2-Q and P2-R are complete

## MVP scope

P3 MVP proves that deterministic paper/replay workflows can consume Lighter adapter outputs without live credentials or network access. It includes replay harness, operator audit summary, failure scenario pack, paper accounting consistency checks, safety review entry criteria, and final verification/docs.

## Technical validation tasks

- [ ] 0.1 Validate P2-R completion baseline
  - Confirm P2-Q and P2-R are complete in `docs/STATUS_REPORT.md`.
  - Confirm full Rust tests, Python feature check, Python smoke, and safety searches passed after P2-Q merge.
  - _Requirements: US-1, US-5_

- [ ] 0.2 Locate reusable P2 components
  - Identify exact APIs for execution fixtures, dispatch, reconciliation, report builders, mock report source, retry policy, and scripted transport.
  - Do not duplicate existing fixture/report/reconciliation logic.
  - _Requirements: US-1, US-3, US-4_

- [ ] 0.3 Validate safety baseline
  - Search source/tests for secret-loading, real private WS, and account mutation paths before P3 implementation.
  - Record existing doc/test deny-list hits separately from new implementation paths.
  - _Requirements: US-5_

## Implementation tasks

- [x] P3-A. Add paper/replay integration harness
  - Create deterministic scenario model and runner using local fixtures/mock inputs only.
  - Cover happy-path order/account/fill/report replay.
  - Reject real endpoint or credential references during scenario validation.
  - Use TDD: write failing harness test first, confirm RED, then implement.
  - _Requirements: US-1_

- [x] P3-B. Add operator audit summary
  - Add stable audit data structure and formatter for replay results.
  - Include lifecycle counts, account timestamps, report consistency, duplicates, stale events, and unresolved anomalies.
  - Use TDD with a deterministic audit snapshot/assertion.
  - _Requirements: US-2_

- [ ] P3-C. Add failure scenario replay pack
  - Add deterministic scenarios for disconnect/resubscribe, duplicate messages, stale updates, empty account/orders, mock report error, retry exhaustion, and sequencer non-filled states.
  - Keep scenarios fast and in-memory; no wall-clock long sleep.
  - Use TDD per scenario group.
  - _Requirements: US-3_

- [ ] P3-D. Add paper accounting consistency checks
  - Compare replay-derived fills/positions/account state with report-derived snapshots.
  - Emit deterministic mismatch diagnostics.
  - Cover success and mismatch tests.
  - _Requirements: US-4_

- [x] P3-E. Document safety review entry criteria
  - Added `safety-review-entry.zh-CN.md` explaining that P3 does not authorize live credentials or live trading.
  - Defines required evidence for any future testnet/private credential design: P2/P3 verification, credential lifecycle design, no-log guarantees, explicit human approval, and separate security review.
  - _Requirements: US-5_

- [ ] P3-F. Run P3 verification and update status
  - Run focused replay/audit tests.
  - Run full Rust crate tests.
  - Run Python feature check.
  - Run Python import smoke when available.
  - Run safety searches.
  - Update docs/status only after verification passes.
  - _Requirements: US-1, US-2, US-3, US-4, US-5_

## Suggested Sub-Agent packaging

- Package 1: P3-A + P3-B if the harness and audit output are tightly coupled.
- Package 2: P3-C + P3-D because failure scenarios need consistency checks.
- Package 3: P3-E + P3-F as final docs/verification pass.

Each package should run in its own worktree, produce a local commit only, and leave merge/push to the main thread.

## Verification commands

- `cargo +1.95.0 test -p nautilus-lighter replay`
- `cargo +1.95.0 test -p nautilus-lighter audit`
- `cargo +1.95.0 test -p nautilus-lighter`
- `cargo +1.95.0 check -p nautilus-lighter --features python`
- `uv run --project /Volumes/HY2TB/projects/nautilus_trader-lighter --group test pytest tests/integration_tests/adapters/lighter/test_imports.py -q`
- `rg -n "std::env::var|dotenv|keyring|Keychain|SecretsManager|KMS|wallet|private key file|\.env" crates/adapters/lighter/src crates/adapters/lighter/tests`
- `rg -n "withdraw|transfer|leverage|margin|stake|unstake|sub_account|private_ws|mainnet|testnet" crates/adapters/lighter/src crates/adapters/lighter/tests/signing_surface.rs crates/adapters/lighter/tests`

## Traceability matrix

| Requirement | Tasks |
|-------------|-------|
| US-1 | 0.1, 0.2, P3-A, P3-F |
| US-2 | P3-B, P3-F |
| US-3 | 0.2, P3-C, P3-F |
| US-4 | 0.2, P3-D, P3-F |
| US-5 | 0.1, 0.3, P3-E, P3-F |

## Consistency audit

- Every user story has at least one implementation task and one verification path.
- P3 remains paper/replay only.
- No task authorizes real credentials, real private WS, live orders, live cancels, or account mutation actions.
- P3 execution is explicitly gated on P2-Q and P2-R completion.
