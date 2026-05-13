# Lighter P3-E Safety Review Entry Criteria

> Date: 2026-05-13
> Status: P3-E docs-only draft
> Scope: Entry criteria for any future Lighter testnet/private credential design after P3 paper/replay work

## Non-authorization statement

P3 paper/replay completion is not live or testnet authorization. P3 only proves deterministic local replay, paper accounting, mock report reconciliation, and operator audit behavior. It does not approve real private credentials, real private WebSocket connections, real auth token generation, real HTTP authenticated requests, live order placement, live cancel, withdrawals, transfers, leverage, margin, funding, liquidation, staking, sub-account actions, production deployment, or any real-funds workflow.

Any future testnet/private credential work must start as a separate proposal and pass a dedicated safety review before implementation begins. A merged P3 task, passing P3 tests, or a clean paper/replay audit cannot be used as implicit approval for live/testnet access.

## Required entry evidence before future testnet/private credential design

A future proposal may enter safety review only when all evidence below is available and attached to the review record:

1. **P2/P3 verification evidence**
   - P2-R final verification summary showing adapter live-prep gates, dry-run private WS, mock reports, reconciliation, sequencer semantics, retry policy, risk-input parsing, and soak replay completed without live credentials.
   - P3 verification summary showing paper/replay harness, audit summary, failure replay pack, and accounting consistency checks completed with local fixtures/mock inputs only.
   - Focused and full test command output summaries, including any skipped tests and why they were skipped.
   - Safety search summaries confirming no new secret-loading, private credential, live private WS, or account mutation implementation path was introduced by P3.

2. **Scope and threat model**
   - Exact environment target: testnet only, mainnet explicitly excluded unless a separate later review approves it.
   - Assets at risk: credentials, accounts, balances, positions, orders, logs, CI artifacts, developer machines, and operator terminals.
   - Trust boundaries: local process, network transport, credential source, signing component, report source, audit/log pipeline, CI, and operator approval path.
   - Abuse cases: credential exfiltration, accidental mainnet endpoint use, unintended order/cancel, replay/live confusion, stale config reuse, logging secrets, and bypassing dry-run gates.

3. **Credential lifecycle design**
   - Credential source and ownership model, with explicit prohibition on ad-hoc `.env`, wallet, private key, API key, keyring, KMS, or credential manager reads until the design names and reviews the approved mechanism.
   - Provisioning, rotation, revocation, expiration, least-privilege permissions, emergency disablement, and destruction procedures.
   - Separation between testnet and mainnet credentials, including hard endpoint/account allowlists and an explicit mainnet deny-by-default rule.
   - Local developer, CI, and operator-machine handling rules, including where credentials may not appear.
   - Backup and recovery rules that do not copy raw private material into repo, logs, screenshots, chat transcripts, or artifacts.

4. **Implementation boundary proposal**
   - Exact code modules proposed for change and exact modules that must remain untouched.
   - Explicit statement whether any private WebSocket, auth token, signing, HTTP authenticated request, order/cancel, or account mutation path is introduced.
   - Compile-time or runtime feature gate plan, default-off behavior, and fail-closed behavior when configuration is incomplete.
   - Dry-run and paper/replay parity plan proving that testnet code cannot be reached by existing P3 replay workflows.

5. **Operational controls**
   - Human approval gate, rollback plan, kill-switch plan, observability plan, and incident response owner.
   - Testnet-only account permissions and balance limits, if any account is ever approved.
   - Runbook for stopping the adapter, revoking credentials, disabling feature flags, and preserving audit evidence without leaking secrets.

## Safety review checklist

Before any future implementation starts, reviewers must record an explicit answer for each item:

- [ ] Is the proposal limited to testnet, with mainnet denied by default?
- [ ] Is there a named credential lifecycle design covering provision, storage, use, rotation, revocation, and destruction?
- [ ] Are credentials excluded from repo files, `.env`, logs, traces, test snapshots, CI artifacts, screenshots, and chat transcripts?
- [ ] Does the design fail closed when credentials, endpoint allowlists, feature gates, or human approvals are missing?
- [ ] Are private WS/auth/HTTP endpoints explicitly allowlisted and separated from paper/replay paths?
- [ ] Are live order, cancel, cancel-all, withdrawal, transfer, leverage, margin, funding, liquidation, staking, and sub-account actions either prohibited or individually reviewed?
- [ ] Is there a deterministic dry-run or mock equivalent for every proposed private/testnet behavior?
- [ ] Are safety searches and focused tests required before and after implementation?
- [ ] Are rollback, kill-switch, observability, and incident response procedures documented and owned?
- [ ] Has a human owner approved the exact reviewed scope, branch, commit range, and verification evidence?

If any checklist item is unanswered or fails, implementation must not start.

## No-log and no-secret handling requirements

Future testnet/private credential work must guarantee that private material is never emitted or persisted by default:

- Do not print tokens, private keys, wallet secrets, API keys, auth headers, signatures, raw credential files, seed material, or credential manager payloads.
- Redact account identifiers when they could identify a real operator account, unless the review explicitly approves a stable masked form.
- Logs may include only high-level credential state such as `credential_present=true`, `credential_source=approved_testnet_provider`, or `auth_mode=testnet_dry_run`, never raw values.
- Test snapshots must use synthetic credentials only and must fail review if they include realistic private material.
- Error messages must classify auth/config/signing failures without embedding request headers, tokens, private keys, signatures, or raw response bodies containing sensitive data.
- CI artifacts, replay outputs, audit summaries, and screenshots must follow the same redaction rules.

## Allowed and prohibited actions

| Category | Allowed before safety review | Requires separate safety review and human approval | Prohibited for P3 and this document |
|----------|------------------------------|----------------------------------------------------|-------------------------------------|
| Local fixtures and replay | Deterministic checked-in fixtures, in-memory scenario events, mock report responses | N/A | Treating fixture success as live authorization |
| Public unauthenticated data | Existing public-data tests and documented replay inputs | Any new network-dependent validation tied to private/testnet work | Using public data tests to bypass private credential review |
| Private WebSocket | Scripted/dry-run transport and fixture messages only | Real testnet private WS with approved credential lifecycle | Mainnet private WS; private WS from P3 replay tasks |
| Auth/token generation | Stub or synthetic token in dry-run tests | Real testnet auth token generation after review | Auth from `.env`, wallet files, private key files, keyring, KMS, or credential managers without approved design |
| HTTP authenticated reports | Mock server and fixture-backed reports | Real testnet authenticated read-only reports after review | Real auth/token request during P3-E or unreviewed implementation |
| Orders and cancels | Synthetic replay states and mock sendTx semantics | Testnet create/cancel/cancel-all only if individually reviewed | Any real order/cancel in P3; any mainnet order/cancel |
| Account mutation actions | Read-only risk inputs from fixtures | None unless a later dedicated review explicitly expands scope | Withdrawal, transfer, leverage, margin, funding, liquidation, staking, sub-account mutation |
| Production deployment | Documentation and local verification only | Separate deployment/security/operations review | Production/live deployment from P3 completion |

## Credential lifecycle design minimum contents

A future design must include at least:

1. Credential type, source, owner, permissions, and environment.
2. Storage and retrieval mechanism, including why alternatives were rejected.
3. Runtime injection path and process boundary.
4. Redaction rules for logs, traces, metrics, errors, audit summaries, test snapshots, and CI artifacts.
5. Rotation, revocation, expiration, emergency disablement, and destruction procedures.
6. Endpoint and account allowlists, with mainnet deny-by-default.
7. Feature gates and configuration validation that fail closed.
8. Test plan covering missing credentials, wrong environment, revoked credentials, endpoint mismatch, auth failure, and signing failure.
9. Operator runbook for approval, launch, monitoring, rollback, and incident response.
10. Evidence retention plan that preserves review/audit records without storing secrets.

## Rollback, kill-switch, and observability requirements

Future testnet/private credential work must define these controls before implementation:

- **Rollback:** exact git revert or feature-flag rollback path, expected state after rollback, and verification commands to prove private/testnet paths are disabled.
- **Kill-switch:** default-off runtime gate that can immediately block private WS, auth token generation, authenticated HTTP, and any order/cancel/account mutation path without code changes.
- **Observability:** counters and structured audit fields for gate decisions, denied actions, endpoint environment, auth failure class, order/cancel attempt class, and redaction status, without logging secrets.
- **Alerting:** operator-visible signal for any denied mainnet endpoint, missing approval, unexpected account mutation attempt, or secret-redaction failure.
- **Incident response:** named owner, stop procedure, credential revocation procedure, evidence preservation procedure, and post-incident review requirement.

## Human approval gate

A future task may proceed beyond design only after a human owner explicitly approves all of the following in writing:

- reviewed scope and non-goals;
- exact branch or commit range under review;
- credential lifecycle design;
- allowed endpoint environment and account identifiers in masked form;
- allowed operation classes;
- verification commands and safety search results;
- rollback and kill-switch procedures;
- confirmation that P3 paper/replay completion is not being used as live/testnet authorization.

Approval must be scope-specific. Approval for testnet read-only reports does not approve private WS, order placement, cancel, mainnet access, account mutation actions, production deployment, or any other future expansion.
