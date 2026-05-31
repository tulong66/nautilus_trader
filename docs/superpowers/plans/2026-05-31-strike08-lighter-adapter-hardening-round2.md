# Strike 08 Lighter Adapter Non-Live Hardening Round 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the second non-live hardening pass for Strike 08 by adding Decimal-safe tick/step preflight, multi-precision metadata fixtures, request-body readiness tests, live report API design documentation, and fresh verification.

**Architecture:** Keep changes inside the Lighter adapter and research reports. Refactor order conversion into a non-live testable request builder so tests can inspect `CreateOrderRequest` without signing, networking, private WebSocket, or live order placement. Use `rust_decimal::Decimal` for exact tick/step/min-size divisibility checks and keep live reports fail-closed while documenting the eventual API design.

**Tech Stack:** Rust, `rust_decimal`, Nautilus Trader model types, existing `nautilus-lighter` tests, Markdown readiness reports.

---

## Safety Boundary

This round remains strictly non-live. Do not read `.env`, API keys, wallet/private key files, keyring/KMS, or any secret store. Do not perform authenticated requests, do not connect private WebSocket, do not place orders, do not cancel real orders, do not transfer, do not withdraw, and do not enable live signing for runtime operations. Unit tests may continue to use existing fixed dummy key strings already present in the repo.

---

## Task 1: Add Decimal-Safe Tick/Step/Min-Size Preflight

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`

Steps:

1. Add `price_increment_raw` and `size_increment_raw` to `OrderConversionMetadata` as `Option<u64>` so tests can validate exact divisibility without float modulus.
2. Add helper:
   ```rust
   fn decimal_to_raw_checked(value: f64, decimals: u8, field_name: &str) -> anyhow::Result<i64>
   ```
   Implementation should use `Decimal::from_f64_retain(value)`, scale by `10^decimals`, reject fractional raw values, and reject negative raw values.
3. Add helper:
   ```rust
   fn validate_raw_increment(raw: i64, increment_raw: Option<u64>, field_name: &str) -> anyhow::Result<()>
   ```
   It should fail if `increment_raw == Some(0)` or if `raw % increment_raw != 0`.
4. Update `validate_order_against_metadata()` to validate:
   - quantity >= min_quantity
   - price increment > 0
   - size increment > 0
   - order price raw divisible by `price_increment_raw` if price exists
   - trigger price raw divisible by `price_increment_raw` if trigger exists
   - quantity raw divisible by `size_increment_raw`
5. Add focused tests:
   - accepts BTC-like metadata: price 100000.1 with price decimals 1, size 0.0001 with size decimals 4
   - rejects price not on tick
   - rejects quantity not on step
   - rejects below min quantity
   - rejects raw conversion with too many decimals

Verification:

```bash
cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter decimal_to_raw_checked --lib
cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter validate_order_against_metadata --lib
```

---

## Task 2: Add Non-Live Request Body Builder Tests

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`

Steps:

1. Add internal struct:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   struct OrderRequestFields {
       client_order_index: i64,
       base_amount: i64,
       price: u32,
       is_ask: bool,
       order_type: u8,
       time_in_force: u8,
       reduce_only: bool,
       trigger_price: u32,
       order_expiry: i64,
   }
   ```
2. Extract conversion logic into:
   ```rust
   fn build_order_request_fields(
       order: &OrderAny,
       metadata: OrderConversionMetadata,
   ) -> anyhow::Result<OrderRequestFields>
   ```
   This function must not sign, spawn tasks, send HTTP, or touch nonce manager.
3. Update `submit_order()` to call `build_order_request_fields()` and use the returned fields for signer/request construction.
4. Add tests:
   - PostOnly buy request has `time_in_force == 3`, `is_ask == false`, raw price/qty match BTC metadata.
   - Sell reduce-only request has `is_ask == true`, `reduce_only == true`.
   - ETH-like metadata with price decimals 2 and size decimals 3 produces expected raw values.

Verification:

```bash
cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter build_order_request_fields --lib
cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter convert_time_in_force --lib
```

---

## Task 3: Document Live Report API Design Without Implementing Live Calls

**Files:**
- Modify: `/Volumes/HY2TB/projects/micro-structure-alpha-lab/reports/round2/mid-price-exchange-structure/strikes/strike_08/STRIKE_08_ADAPTER_READINESS_AUDIT.md`

Steps:

1. Add a section `Live report API design sketch, not implemented`.
2. Specify required future endpoints/semantics:
   - active orders by account/market/client order id
   - fills by account/market/order id/time range
   - pagination/cursor handling
   - stale/private WS mismatch handling
   - fail-closed behavior on timeout/error/empty ambiguous response
3. Explicitly state no live calls were made and live report implementation remains blocked.

Verification:

```bash
python3 - <<'PY'
from pathlib import Path
p = Path('/Volumes/HY2TB/projects/micro-structure-alpha-lab/reports/round2/mid-price-exchange-structure/strikes/strike_08/STRIKE_08_ADAPTER_READINESS_AUDIT.md')
text = p.read_text(encoding='utf-8')
required = ['Live report API design sketch', 'not implemented', 'pagination', 'fail-closed', '未认证', '未下单']
missing = [s for s in required if s not in text]
print({'line_count': len(text.splitlines()), 'missing': missing})
raise SystemExit(1 if missing else 0)
PY
```

---

## Task 4: Final Verification And Status Update

**Files:**
- Modify: readiness report only if verification results differ from expected.

Steps:

1. Run focused tests:
   ```bash
   cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter decimal_to_raw_checked --lib
   cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter validate_order_against_metadata --lib
   cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter build_order_request_fields --lib
   ```
2. Run package tests:
   ```bash
   cargo test --manifest-path /Volumes/HY2TB/projects/nautilus_trader-lighter/Cargo.toml -p nautilus-lighter
   ```
3. Run safety search:
   ```bash
   rg -n "dotenv|API_KEY|PRIVATE_KEY|private_key|wallet|keyring|KMS|std::env::var|read_to_string\(" /Volumes/HY2TB/projects/nautilus_trader-lighter/crates/adapters/lighter/src /Volumes/HY2TB/projects/nautilus_trader-lighter/crates/adapters/lighter/tests
   ```
4. Commit only intended adapter files and plan file. Do not stage unrelated `uv.lock`, examples, or integration test files unless they are directly part of this round.

Expected end state:

```text
Precision/min-size: READY_TESTED_NON_LIVE for Decimal tick/step/min-size preflight.
Request body readiness: READY_TESTED_NON_LIVE at non-signing conversion layer.
Live report API: DESIGN_SKETCHED_NOT_IMPLEMENTED_FAIL_CLOSED.
Strike 08 live readiness: still BLOCKED_FOR_LIVE_RUN.
```
