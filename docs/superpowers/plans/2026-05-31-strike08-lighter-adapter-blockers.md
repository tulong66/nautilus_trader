# Strike 08 Lighter Adapter Blocker Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clear the non-live technical blockers that currently prevent Strike 08 from being safely promoted beyond local shadow replay: PostOnly/TIF=3 mapping, metadata-driven precision/min-size preflight, explicit fail-closed reconciliation readiness, cancel-all-on-exit risk classification, and safety-gate tests.

**Architecture:** Keep this as an adapter-hardening change inside `crates/adapters/lighter`; do not add any live wrapper and do not perform authentication or network actions. Add focused tests first, then minimally extend `LighterExecutionClient` and `ExecutionReconciler` so execution conversion and readiness checks are explicit, deterministic, and testable. Use existing Nautilus order/instrument abstractions (`OrderAny`, `InstrumentAny`, `Instrument` trait methods) instead of exchange credentials or live API calls.

**Tech Stack:** Rust, Nautilus Trader model/execution crates, `cargo test`, existing Lighter adapter unit/integration tests.

---

## Safety Boundary For All Tasks

Every task in this plan is non-live and test-first. Do not read `.env`, API keys, wallet/private key files, keyring/KMS, or any secret store. Do not perform authenticated requests, do not connect private WebSocket, do not place orders, do not cancel real orders, do not transfer, do not withdraw, and do not enable `enable_live_signing` outside in-memory unit-test config construction.

The existing working tree may contain unrelated local changes. Before implementation, run `git status --short` from `/Volumes/HY2TB/projects/nautilus_trader-lighter` and avoid modifying unrelated files such as `uv.lock`, `examples/backtest/lighter_min_size_cost_replay.py`, or `tests/integration_tests/adapters/lighter/test_min_size_cost_replay.py` unless the user separately assigns those files.

---

## File Structure

### Modify

- `crates/adapters/lighter/src/execution/client.rs`
  - Add tests for PostOnly conversion and precision/min-size conversion.
  - Change `convert_time_in_force()` to honor `order.is_post_only()` before ordinary `TimeInForce` mapping.
  - Replace hard-coded submit conversion decimals with metadata-driven helpers.
  - Add explicit fail-closed readiness helpers for live report/reconciliation limitations.

- `crates/adapters/lighter/src/execution/reconciliation.rs`
  - Add risk-classification helpers so pending/open/partial/cancel-pending states are visible to cancel-all-on-exit and Strike 08 readiness logic.
  - Add unit tests for final vs open-risk states.

- `crates/adapters/lighter/tests/execution_dispatch.rs`
  - Add or extend integration-style replay tests verifying open-risk classification after send/update/fill/cancel/cancel-reject flows.

- `crates/adapters/lighter/tests/execution_reports.rs`
  - Add non-live tests documenting that mock/fixture reports can parse, while live report generation remains explicitly unavailable/fail-closed until implemented.

- `docs/superpowers/plans/2026-05-31-strike08-lighter-adapter-blockers.md`
  - This plan file.

### Do Not Modify In This Plan

- No live wrapper entrypoint.
- No private signing implementation changes except using existing signer constants in tests.
- No `.env` or secret-related files.
- No production strategy/backtest files outside adapter readiness scope.

---

## Task 1: Map Nautilus PostOnly Flag To Lighter TIF=3

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`

- [ ] **Step 1: Write the failing PostOnly conversion test**

Add this test inside the existing `#[cfg(test)] mod tests` in `client.rs` near the other execution-client unit tests. If the imports already exist, merge them instead of duplicating.

```rust
use nautilus_model::enums::{OrderSide, OrderType, TimeInForce};
use nautilus_model::identifiers::InstrumentId;
use nautilus_model::orders::builder::OrderTestBuilder;
use nautilus_model::prices::Price;
use nautilus_model::quantities::Quantity;

#[test]
fn convert_time_in_force_maps_post_only_order_flag_to_lighter_tif_3() {
    let order = OrderTestBuilder::new(OrderType::Limit)
        .instrument_id(InstrumentId::from("BTC_USDC.LIGHTER"))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("0.0001"))
        .price(Price::new(100_000.0, 2))
        .time_in_force(TimeInForce::Gtc)
        .post_only(true)
        .build();

    assert!(order.is_post_only());
    assert_eq!(
        LighterExecutionClient::convert_time_in_force(&order),
        LighterSigner::TIF_POST_ONLY,
    );
}
```

- [ ] **Step 2: Write regression tests for ordinary TIF values**

Add this table-style test in the same module so the PostOnly fix cannot break normal GTC/IOC/FOK behavior.

```rust
#[test]
fn convert_time_in_force_preserves_non_post_only_tif_mapping() {
    let cases = [
        (TimeInForce::Gtc, LighterSigner::TIF_GOOD_TILL_TIME),
        (TimeInForce::Ioc, LighterSigner::TIF_IMMEDIATE_OR_CANCEL),
        (TimeInForce::Fok, LighterSigner::TIF_FILL_OR_KILL),
        (TimeInForce::Gtd, LighterSigner::TIF_GOOD_TILL_TIME),
        (TimeInForce::Day, LighterSigner::TIF_GOOD_TILL_TIME),
        (TimeInForce::AtTheOpen, LighterSigner::TIF_GOOD_TILL_TIME),
        (TimeInForce::AtTheClose, LighterSigner::TIF_GOOD_TILL_TIME),
    ];

    for (time_in_force, expected) in cases {
        let order = OrderTestBuilder::new(OrderType::Limit)
            .instrument_id(InstrumentId::from("BTC_USDC.LIGHTER"))
            .side(OrderSide::Buy)
            .quantity(Quantity::from("0.0001"))
            .price(Price::new(100_000.0, 2))
            .time_in_force(time_in_force)
            .post_only(false)
            .build();

        assert!(!order.is_post_only());
        assert_eq!(
            LighterExecutionClient::convert_time_in_force(&order),
            expected,
            "unexpected mapping for {time_in_force:?}",
        );
    }
}
```

- [ ] **Step 3: Run the focused test and verify RED**

Run from repo root:

```bash
cargo test -p nautilus-lighter convert_time_in_force_maps_post_only_order_flag_to_lighter_tif_3 --lib
```

Expected before implementation: test fails because `convert_time_in_force()` returns `TIF_GOOD_TILL_TIME` for a GTC PostOnly order instead of `TIF_POST_ONLY`.

- [ ] **Step 4: Implement the minimal mapping fix**

Change `convert_time_in_force()` in `client.rs` to check the order flag before matching `TimeInForce`.

```rust
fn convert_time_in_force(order: &OrderAny) -> u8 {
    if order.is_post_only() {
        return LighterSigner::TIF_POST_ONLY;
    }

    use nautilus_model::enums::TimeInForce;
    match order.time_in_force() {
        TimeInForce::Gtc => LighterSigner::TIF_GOOD_TILL_TIME,
        TimeInForce::Ioc => LighterSigner::TIF_IMMEDIATE_OR_CANCEL,
        TimeInForce::Fok => LighterSigner::TIF_FILL_OR_KILL,
        TimeInForce::Gtd => LighterSigner::TIF_GOOD_TILL_TIME,
        TimeInForce::Day => LighterSigner::TIF_GOOD_TILL_TIME,
        TimeInForce::AtTheOpen => LighterSigner::TIF_GOOD_TILL_TIME,
        TimeInForce::AtTheClose => LighterSigner::TIF_GOOD_TILL_TIME,
    }
}
```

Do not add a nonexistent `TimeInForce::PostOnly` variant. In Nautilus, PostOnly is an order flag exposed by `order.is_post_only()`.

- [ ] **Step 5: Verify GREEN for PostOnly and ordinary mappings**

Run:

```bash
cargo test -p nautilus-lighter convert_time_in_force --lib
```

Expected after implementation: both new conversion tests pass.

- [ ] **Step 6: Commit this task**

Only commit if the repository policy for this work allows local commits and there are no unrelated staged changes.

```bash
git add crates/adapters/lighter/src/execution/client.rs
git commit -m "fix(lighter): map post-only orders to tif 3"
```

---

## Task 2: Add Metadata-Driven Price/Quantity Conversion Helpers

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`

- [ ] **Step 1: Write tests for raw price and quantity conversion with explicit precisions**

Add helper-level tests in `client.rs` before touching submit logic. These tests lock expected integer conversion without requiring network or live signing.

```rust
#[test]
fn convert_price_to_raw_uses_supplied_price_precision() {
    assert_eq!(
        LighterExecutionClient::convert_price_to_raw(100_000.1, 1),
        1_000_001,
    );
    assert_eq!(
        LighterExecutionClient::convert_price_to_raw(100_000.12, 2),
        10_000_012,
    );
}

#[test]
fn convert_quantity_to_raw_uses_supplied_size_precision() {
    assert_eq!(
        LighterExecutionClient::convert_quantity_to_raw(0.0001, 4),
        1,
    );
    assert_eq!(
        LighterExecutionClient::convert_quantity_to_raw(0.0001, 8),
        10_000,
    );
}
```

- [ ] **Step 2: Run helper tests and verify RED where the old signature is incompatible**

Run:

```bash
cargo test -p nautilus-lighter convert_quantity_to_raw_uses_supplied_size_precision --lib
```

Expected before implementation: compilation fails or test fails because `convert_quantity_to_raw()` currently accepts only `quantity: f64` and hardcodes 8 decimals.

- [ ] **Step 3: Change quantity conversion signature**

Replace the hard-coded quantity converter with a precision-driven version.

```rust
fn convert_quantity_to_raw(quantity: f64, size_decimals: u8) -> i64 {
    let multiplier = 10f64.powi(size_decimals as i32);
    (quantity * multiplier) as i64
}
```

Keep `convert_price_to_raw(price, price_decimals)` as the precision-driven price converter.

- [ ] **Step 4: Update submit path to compile with explicit precision variables**

Temporarily keep the old defaults as local fallback variables, but route through explicit precision arguments. This keeps the helper refactor isolated before instrument metadata is wired in.

```rust
let price_decimals: u8 = 2;
let size_decimals: u8 = 8;
let price_raw = if let Some(price) = order.price() {
    Self::convert_price_to_raw(price.as_f64(), price_decimals)
} else {
    0
};
let trigger_price_raw = if let Some(trigger_price) = order.trigger_price() {
    Self::convert_price_to_raw(trigger_price.as_f64(), price_decimals)
} else {
    0
};
let quantity_raw = Self::convert_quantity_to_raw(order.quantity().as_f64(), size_decimals);
```

Task 3 will replace these fallback decimals with instrument metadata.

- [ ] **Step 5: Verify helper tests GREEN**

Run:

```bash
cargo test -p nautilus-lighter convert_price_to_raw_uses_supplied_price_precision --lib
cargo test -p nautilus-lighter convert_quantity_to_raw_uses_supplied_size_precision --lib
```

Expected after implementation: both tests pass.

- [ ] **Step 6: Commit this task**

```bash
git add crates/adapters/lighter/src/execution/client.rs
git commit -m "refactor(lighter): make order raw conversion precision-driven"
```

---

## Task 3: Use Cached Instrument Metadata For Submit Preflight

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`

- [ ] **Step 1: Add a small metadata struct and lookup helper**

Add a private struct near `OrderState` or inside `impl LighterExecutionClient` support code.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderConversionMetadata {
    price_decimals: u8,
    size_decimals: u8,
    min_quantity: Option<f64>,
    price_increment: f64,
    size_increment: f64,
}
```

Add a helper that reads cached instrument metadata. Import the `Instrument` trait if required by the compiler.

```rust
fn conversion_metadata_for_order(
    &self,
    order: &OrderAny,
) -> anyhow::Result<OrderConversionMetadata> {
    use nautilus_model::instruments::Instrument;

    let instrument_id = order.instrument_id();
    let instrument = self.instruments.get(&instrument_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Lighter instrument metadata unavailable for {}; cannot submit without price/size precision and min quantity preflight",
            instrument_id,
        )
    })?;

    Ok(OrderConversionMetadata {
        price_decimals: instrument.price_precision(),
        size_decimals: instrument.size_precision(),
        min_quantity: instrument.min_quantity().map(|qty| qty.as_f64()),
        price_increment: instrument.price_increment().as_f64(),
        size_increment: instrument.size_increment().as_f64(),
    })
}
```

If `InstrumentAny` exposes these methods directly without importing the trait, remove the local `use` after compile feedback.

- [ ] **Step 2: Add preflight validation helper**

Add this helper to keep submit logic readable.

```rust
fn validate_order_against_metadata(
    order: &OrderAny,
    metadata: OrderConversionMetadata,
) -> anyhow::Result<()> {
    let quantity = order.quantity().as_f64();
    if let Some(min_quantity) = metadata.min_quantity {
        if quantity < min_quantity {
            anyhow::bail!(
                "Lighter order quantity {} is below instrument min quantity {}; refusing submit",
                quantity,
                min_quantity,
            );
        }
    }

    if metadata.price_increment <= 0.0 {
        anyhow::bail!(
            "Lighter instrument price increment {} must be positive",
            metadata.price_increment,
        );
    }
    if metadata.size_increment <= 0.0 {
        anyhow::bail!(
            "Lighter instrument size increment {} must be positive",
            metadata.size_increment,
        );
    }

    Ok(())
}
```

This first pass blocks missing/invalid metadata and below-min-size orders. Tick/step exact divisibility can be added after confirming model `Price`/`Quantity` precision semantics in this adapter; do not guess with fragile float modulus in the first patch.

- [ ] **Step 3: Write a unit test for missing metadata fail-closed**

Add this test in `client.rs`. It does not connect, sign, or use network.

```rust
#[test]
fn conversion_metadata_for_order_fails_closed_when_instrument_missing() {
    let client = test_execution_client();
    let order = OrderTestBuilder::new(OrderType::Limit)
        .instrument_id(InstrumentId::from("BTC_USDC.LIGHTER"))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("0.0001"))
        .price(Price::new(100_000.0, 2))
        .build();

    let err = client
        .conversion_metadata_for_order(&order)
        .expect_err("missing instrument metadata must block live submit conversion");

    assert!(
        err.to_string().contains("instrument metadata unavailable"),
        "unexpected error: {err}",
    );
}
```

- [ ] **Step 4: Write unit tests for min quantity preflight**

These tests use the metadata struct directly and avoid needing to construct a full `InstrumentAny` fixture.

```rust
#[test]
fn validate_order_against_metadata_rejects_below_min_quantity() {
    let order = OrderTestBuilder::new(OrderType::Limit)
        .instrument_id(InstrumentId::from("BTC_USDC.LIGHTER"))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("0.00005"))
        .price(Price::new(100_000.0, 2))
        .build();
    let metadata = OrderConversionMetadata {
        price_decimals: 2,
        size_decimals: 8,
        min_quantity: Some(0.0001),
        price_increment: 0.01,
        size_increment: 0.00000001,
    };

    let err = LighterExecutionClient::validate_order_against_metadata(&order, metadata)
        .expect_err("below-min-size order must fail preflight");

    assert!(
        err.to_string().contains("below instrument min quantity"),
        "unexpected error: {err}",
    );
}

#[test]
fn validate_order_against_metadata_accepts_min_quantity() {
    let order = OrderTestBuilder::new(OrderType::Limit)
        .instrument_id(InstrumentId::from("BTC_USDC.LIGHTER"))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("0.0001"))
        .price(Price::new(100_000.0, 2))
        .build();
    let metadata = OrderConversionMetadata {
        price_decimals: 2,
        size_decimals: 8,
        min_quantity: Some(0.0001),
        price_increment: 0.01,
        size_increment: 0.00000001,
    };

    LighterExecutionClient::validate_order_against_metadata(&order, metadata)
        .expect("min-size order should pass preflight");
}
```

- [ ] **Step 5: Run the focused preflight tests and verify RED/GREEN loop**

Run after writing tests but before implementation if possible; expected initial result is compile failure because helpers do not exist.

```bash
cargo test -p nautilus-lighter conversion_metadata_for_order_fails_closed_when_instrument_missing --lib
cargo test -p nautilus-lighter validate_order_against_metadata --lib
```

After helper implementation, expected result: all focused tests pass.

- [ ] **Step 6: Wire metadata into `submit_order` conversion**

In `submit_order`, replace hardcoded decimals with lookup and preflight before signing.

```rust
let conversion_metadata = self.conversion_metadata_for_order(order)?;
Self::validate_order_against_metadata(order, conversion_metadata)?;

let price_raw = if let Some(price) = order.price() {
    Self::convert_price_to_raw(price.as_f64(), conversion_metadata.price_decimals)
} else {
    0
};
let trigger_price_raw = if let Some(trigger_price) = order.trigger_price() {
    Self::convert_price_to_raw(trigger_price.as_f64(), conversion_metadata.price_decimals)
} else {
    0
};
let quantity_raw = Self::convert_quantity_to_raw(
    order.quantity().as_f64(),
    conversion_metadata.size_decimals,
);
```

Keep existing `market_index` lookup. Missing instrument metadata and missing market index should both block submit before signing.

- [ ] **Step 7: Run submit-path focused tests**

Run:

```bash
cargo test -p nautilus-lighter conversion_metadata_for_order --lib
cargo test -p nautilus-lighter validate_order_against_metadata --lib
cargo test -p nautilus-lighter convert_quantity_to_raw --lib
```

Expected: all focused conversion/preflight tests pass.

- [ ] **Step 8: Commit this task**

```bash
git add crates/adapters/lighter/src/execution/client.rs
git commit -m "feat(lighter): require instrument metadata for order conversion"
```

---

## Task 4: Make Reconciliation Risk Explicit And Fail-Closed

**Files:**
- Modify: `crates/adapters/lighter/src/execution/reconciliation.rs`
- Modify: `crates/adapters/lighter/tests/execution_dispatch.rs`

- [ ] **Step 1: Add status classification helpers**

In `reconciliation.rs`, implement explicit classification on `ReconciledOrderStatus`.

```rust
impl ReconciledOrderStatus {
    pub fn requires_reconciliation(self) -> bool {
        matches!(
            self,
            Self::Sent
                | Self::Submitted
                | Self::Accepted
                | Self::Pending
                | Self::Timeout
                | Self::PartiallyFilled
                | Self::CancelPending
                | Self::CancelRejected
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Filled | Self::Canceled | Self::Rejected | Self::Executed)
    }
}
```

Rationale: for Strike 08, `CancelRejected`, `Timeout`, and `PartiallyFilled` are not safe terminal states; they require reconciliation before claiming the session is clean.

- [ ] **Step 2: Add reconciler aggregate helpers**

Add methods on `ExecutionReconciler`.

```rust
pub fn orders_requiring_reconciliation(&self) -> Vec<(ClientOrderId, ReconciledOrderStatus)> {
    self.orders
        .iter()
        .filter_map(|(client_order_id, state)| {
            state
                .status
                .requires_reconciliation()
                .then(|| (*client_order_id, state.status))
        })
        .collect()
}

pub fn has_reconciliation_risk(&self) -> bool {
    self.orders
        .values()
        .any(|state| state.status.requires_reconciliation())
}
```

If `ClientOrderId` is not `Copy`, use `client_order_id.clone()` instead of `*client_order_id`.

- [ ] **Step 3: Write unit tests for state classification**

Add tests in `reconciliation.rs`.

```rust
#[test]
fn reconciled_order_status_classifies_terminal_and_risk_states() {
    assert!(ReconciledOrderStatus::Filled.is_terminal());
    assert!(ReconciledOrderStatus::Canceled.is_terminal());
    assert!(ReconciledOrderStatus::Rejected.is_terminal());

    assert!(!ReconciledOrderStatus::Filled.requires_reconciliation());
    assert!(!ReconciledOrderStatus::Canceled.requires_reconciliation());
    assert!(!ReconciledOrderStatus::Rejected.requires_reconciliation());

    assert!(ReconciledOrderStatus::Accepted.requires_reconciliation());
    assert!(ReconciledOrderStatus::PartiallyFilled.requires_reconciliation());
    assert!(ReconciledOrderStatus::CancelPending.requires_reconciliation());
    assert!(ReconciledOrderStatus::CancelRejected.requires_reconciliation());
    assert!(ReconciledOrderStatus::Timeout.requires_reconciliation());
}
```

- [ ] **Step 4: Write integration-style dispatch replay test**

In `tests/execution_dispatch.rs`, add a test that proves the reconciler remains risky after an open/partial/cancel-rejected replay and becomes clean only after terminal cancel/fill/reject where appropriate. Use existing fixture/style in that file; the final assertions should be:

```rust
assert!(reconciler.has_reconciliation_risk());
let risky = reconciler.orders_requiring_reconciliation();
assert!(risky.iter().any(|(_, status)| {
    matches!(
        status,
        ReconciledOrderStatus::Accepted
            | ReconciledOrderStatus::PartiallyFilled
            | ReconciledOrderStatus::CancelRejected
    )
}));
```

Then replay a terminal `canceled` or `filled` update for the same order and assert:

```rust
assert!(!reconciler.has_reconciliation_risk());
assert!(reconciler.orders_requiring_reconciliation().is_empty());
```

- [ ] **Step 5: Run focused reconciliation tests**

Run:

```bash
cargo test -p nautilus-lighter reconciled_order_status_classifies_terminal_and_risk_states --lib
cargo test -p nautilus-lighter --test execution_dispatch reconciliation
```

Expected: all focused reconciliation tests pass.

- [ ] **Step 6: Commit this task**

```bash
git add crates/adapters/lighter/src/execution/reconciliation.rs crates/adapters/lighter/tests/execution_dispatch.rs
git commit -m "feat(lighter): expose reconciliation risk states"
```

---

## Task 5: Add Explicit Live Report Readiness Guard

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`
- Modify: `crates/adapters/lighter/tests/execution_reports.rs`

- [ ] **Step 1: Add a named readiness error helper**

In `client.rs`, add a private associated function that centralizes the current live-report limitation. This prevents silent “empty reports” from being interpreted as safe reconciliation by future Strike 08 code.

```rust
fn live_reports_unavailable_error(operation: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "Lighter {operation} live report generation is not implemented; private WS-only state is insufficient for Strike 08 fail-closed reconciliation",
    )
}
```

- [ ] **Step 2: Add unit test for the readiness message**

In `client.rs` tests:

```rust
#[test]
fn live_reports_unavailable_error_is_explicitly_fail_closed() {
    let err = LighterExecutionClient::live_reports_unavailable_error("order status");
    let msg = err.to_string();

    assert!(msg.contains("live report generation is not implemented"));
    assert!(msg.contains("fail-closed reconciliation"));
}
```

- [ ] **Step 3: Change live report generation methods to return explicit errors instead of silent empty success**

For the live API branches in:

- `generate_order_status_report`
- `generate_order_status_reports`
- `generate_fill_reports`

replace the current `warn!(...)` + `Ok(None)` / `Ok(Vec::new())` live-not-implemented returns with explicit fail-closed errors:

```rust
return Err(Self::live_reports_unavailable_error("order status"));
```

```rust
return Err(Self::live_reports_unavailable_error("order status reports"));
```

```rust
return Err(Self::live_reports_unavailable_error("fill reports"));
```

Keep mock/fixture report paths unchanged. This is intentionally stricter: downstream code must not confuse “not implemented” with “no open orders/no fills.”

- [ ] **Step 4: Add non-live regression tests for mock report paths**

In `tests/execution_reports.rs`, ensure existing mock/fixture tests still pass. If adding a test, use existing fixture-based helpers and assert reports still build from local fixtures. The important assertion is that this task does not break non-live report parsing.

```rust
// Add only if no equivalent assertion exists in current tests.
assert!(!reports.is_empty(), "fixture reports should still parse in non-live tests");
```

- [ ] **Step 5: Run report-focused tests**

Run:

```bash
cargo test -p nautilus-lighter live_reports_unavailable_error_is_explicitly_fail_closed --lib
cargo test -p nautilus-lighter --test execution_reports
```

Expected: explicit readiness helper test passes and fixture/mock report tests continue to pass.

- [ ] **Step 6: Commit this task**

```bash
git add crates/adapters/lighter/src/execution/client.rs crates/adapters/lighter/tests/execution_reports.rs
git commit -m "fix(lighter): fail closed when live reports are unavailable"
```

---

## Task 6: Add Cancel-All-On-Exit Readiness Checks

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`
- Modify: `crates/adapters/lighter/src/execution/reconciliation.rs`

- [ ] **Step 1: Add client-level reconciliation risk snapshot helper**

In `client.rs`, add a helper that the future Strike 08 wrapper can call after cancel-all-on-exit. It does not send cancels itself; it only reports whether the local reconciler still sees risky states.

```rust
fn orders_requiring_reconciliation_snapshot(
    &self,
) -> Vec<(ClientOrderId, ReconciledOrderStatus)> {
    self.reconciler
        .lock()
        .expect("reconciler mutex poisoned")
        .orders_requiring_reconciliation()
}

fn ensure_no_reconciliation_risk(&self) -> anyhow::Result<()> {
    let risky = self.orders_requiring_reconciliation_snapshot();
    if !risky.is_empty() {
        anyhow::bail!(
            "Lighter reconciliation risk remains after cancel-all-on-exit: {:?}",
            risky,
        );
    }
    Ok(())
}
```

If `ClientOrderId` or `ReconciledOrderStatus` is not imported in `client.rs`, add narrow imports from the existing reconciliation module/model identifiers.

- [ ] **Step 2: Write unit test for clean snapshot**

In `client.rs` tests:

```rust
#[test]
fn ensure_no_reconciliation_risk_allows_empty_reconciler() {
    let client = test_execution_client();

    client
        .ensure_no_reconciliation_risk()
        .expect("empty reconciler should be clean");
}
```

- [ ] **Step 3: Write unit test for risky snapshot**

Use existing reconciler APIs to insert/send an order state in the client’s reconciler. If the current API uses `record_send` or similar, use the existing method names from `reconciliation.rs`; the test intent must be exactly:

```rust
#[test]
fn ensure_no_reconciliation_risk_rejects_open_order_state() {
    let client = test_execution_client();
    let client_order_id = ClientOrderId::from("strike08_probe_open_001");

    {
        let mut reconciler = client.reconciler.lock().expect("reconciler mutex poisoned");
        reconciler.record_order_status_for_test(
            client_order_id,
            ReconciledOrderStatus::Accepted,
        );
    }

    let err = client
        .ensure_no_reconciliation_risk()
        .expect_err("accepted order must remain risky until terminal reconciliation");

    assert!(
        err.to_string().contains("reconciliation risk remains"),
        "unexpected error: {err}",
    );
}
```

If no test-only setter exists, add one behind `#[cfg(test)]` in `ExecutionReconciler`:

```rust
#[cfg(test)]
pub(crate) fn record_order_status_for_test(
    &mut self,
    client_order_id: ClientOrderId,
    status: ReconciledOrderStatus,
) {
    self.orders.insert(
        client_order_id,
        ReconciledOrderState {
            status,
            last_event_id: None,
            updated_at: UnixNanos::default(),
        },
    );
}
```

Adjust field names to match the actual `ReconciledOrderState` struct in `reconciliation.rs`.

- [ ] **Step 4: Run cancel-all readiness focused tests**

Run:

```bash
cargo test -p nautilus-lighter ensure_no_reconciliation_risk --lib
```

Expected: clean empty reconciler passes; accepted/open state fails closed.

- [ ] **Step 5: Commit this task**

```bash
git add crates/adapters/lighter/src/execution/client.rs crates/adapters/lighter/src/execution/reconciliation.rs
git commit -m "feat(lighter): expose cancel-all reconciliation readiness"
```

---

## Task 7: Preserve And Extend Safety Gate Coverage

**Files:**
- Modify: `crates/adapters/lighter/src/execution/client.rs`
- Modify: `crates/adapters/lighter/src/config.rs` only if existing config tests need a clearer assertion name

- [ ] **Step 1: Confirm existing live-signing gate tests still exist**

Do not delete or weaken these existing tests:

```rust
#[tokio::test]
async fn connect_rejects_default_config_before_live_signing_or_private_ws() { ... }

#[test]
fn live_signing_gate_allows_explicit_opt_in() { ... }
```

- [ ] **Step 2: Add a test proving conversion/preflight helpers do not require live signing**

This test prevents future developers from conflating local conversion readiness with actual live authorization.

```rust
#[test]
fn post_only_conversion_does_not_require_live_signing_opt_in() {
    let client = test_execution_client();
    assert!(!client.config.enable_live_signing);

    let order = OrderTestBuilder::new(OrderType::Limit)
        .instrument_id(InstrumentId::from("BTC_USDC.LIGHTER"))
        .side(OrderSide::Buy)
        .quantity(Quantity::from("0.0001"))
        .price(Price::new(100_000.0, 2))
        .post_only(true)
        .build();

    assert_eq!(
        LighterExecutionClient::convert_time_in_force(&order),
        LighterSigner::TIF_POST_ONLY,
    );
}
```

- [ ] **Step 3: Add safety search command to the verification notes**

Before final completion, run this command from repo root and inspect matches:

```bash
rg -n "read_to_string\(|dotenv|API_KEY|PRIVATE_KEY|private_key|wallet|keyring|KMS|connect_private|submit_order\(|cancel_order\(|cancel_all_orders\(" crates/adapters/lighter/src crates/adapters/lighter/tests
```

Expected: matches may include existing config fields, existing adapter methods, and test names. There must be no new secret-reading path and no new live network/order/cancel action in tests.

- [ ] **Step 4: Run safety-gate focused tests**

Run:

```bash
cargo test -p nautilus-lighter live_signing_gate --lib
cargo test -p nautilus-lighter post_only_conversion_does_not_require_live_signing_opt_in --lib
cargo test -p nautilus-lighter connect_rejects_default_config_before_live_signing_or_private_ws --lib
```

Expected: all safety-gate tests pass.

- [ ] **Step 5: Commit this task**

```bash
git add crates/adapters/lighter/src/execution/client.rs crates/adapters/lighter/src/config.rs
git commit -m "test(lighter): preserve non-live safety gates"
```

---

## Task 8: Run Adapter Readiness Verification Suite

**Files:**
- No code changes expected unless tests reveal a defect in prior tasks.

- [ ] **Step 1: Run focused conversion tests**

```bash
cargo test -p nautilus-lighter convert_time_in_force --lib
cargo test -p nautilus-lighter convert_price_to_raw --lib
cargo test -p nautilus-lighter convert_quantity_to_raw --lib
cargo test -p nautilus-lighter validate_order_against_metadata --lib
```

Expected: all focused tests pass.

- [ ] **Step 2: Run reconciliation and dispatch tests**

```bash
cargo test -p nautilus-lighter reconciled_order_status_classifies_terminal_and_risk_states --lib
cargo test -p nautilus-lighter ensure_no_reconciliation_risk --lib
cargo test -p nautilus-lighter --test execution_dispatch
```

Expected: all pass.

- [ ] **Step 3: Run report tests**

```bash
cargo test -p nautilus-lighter live_reports_unavailable_error_is_explicitly_fail_closed --lib
cargo test -p nautilus-lighter --test execution_reports
```

Expected: all pass. If existing tests expected `Ok(None)` or empty vectors from live report branches, update those expectations to explicit fail-closed errors.

- [ ] **Step 4: Run package-level adapter tests if time allows**

```bash
cargo test -p nautilus-lighter
```

Expected: package tests pass. If unrelated pre-existing failures occur, record them exactly and do not claim full package green.

- [ ] **Step 5: Run safety search**

```bash
rg -n "dotenv|API_KEY|PRIVATE_KEY|private_key|wallet|keyring|KMS|std::env::var|read_to_string\(" crates/adapters/lighter/src crates/adapters/lighter/tests
```

Expected: no new secret-reading behavior. Existing config field names are acceptable if they are not reading secrets from disk/env in tests.

- [ ] **Step 6: Check git status and diff scope**

```bash
git status --short
git diff --stat
```

Expected: modified files are limited to the adapter readiness files in this plan plus this plan file. Unrelated pre-existing files should remain unstaged and unchanged by this work.

---

## Task 9: Update Strike 08 Readiness Documentation After Verification

**Files:**
- Modify: `/Volumes/HY2TB/projects/micro-structure-alpha-lab/reports/round2/mid-price-exchange-structure/strikes/strike_08/STRIKE_08_ADAPTER_READINESS_AUDIT.md`
- Optional Modify: `/Volumes/HY2TB/projects/micro-structure-alpha-lab/reports/round2/mid-price-exchange-structure/strikes/strike_08/STRIKE_08_SHADOW_REPLAY_REPORT.md`

- [ ] **Step 1: Update status labels based on verified code, not intention**

After tests pass, update `STRIKE_08_ADAPTER_READINESS_AUDIT.md` readiness matrix as follows only if the corresponding verification evidence exists:

```text
PostOnly / TIF=3: READY_TESTED_NON_LIVE
Precision/min-size: PARTIAL_READY_TESTED_NON_LIVE
live order/fill reports: FAIL_CLOSED_TESTED_NON_LIVE, still not live reconciliation implemented
cancel_all_orders: READY_STATIC + RECONCILIATION_RISK_HELPER_TESTED
private WS dispatch: PARTIAL_READY_STATIC + RECONCILIATION_RISK_TESTED
```

Do not change overall `live_readiness = BLOCKED` to READY unless live report implementation, private lifecycle testing, and user-approved live rehearsal are actually completed. For this plan, expected overall status remains:

```text
live_readiness = BLOCKED_FOR_LIVE_RUN_BUT_ADAPTER_BLOCKERS_PARTIALLY_CLEARED_NON_LIVE
```

- [ ] **Step 2: Add a verification evidence section**

Append a section like this, filling exact command outputs from the fresh run:

```markdown
## 12. Non-live blocker cleanup verification

日期：2026-05-31  
范围：非 live 单元/集成测试；未认证、未连接 private WS、未下单、未撤单。

已验证：
- `cargo test -p nautilus-lighter convert_time_in_force --lib`：PASS
- `cargo test -p nautilus-lighter validate_order_against_metadata --lib`：PASS
- `cargo test -p nautilus-lighter ensure_no_reconciliation_risk --lib`：PASS
- `cargo test -p nautilus-lighter --test execution_dispatch`：PASS
- `cargo test -p nautilus-lighter --test execution_reports`：PASS

仍阻塞真实 live：
- live order/fill report API 仍未实现，只是从 silent empty 改成 fail-closed error。
- private WS ack/fill/cancel 全生命周期仍未做真实或 sandbox rehearsal。
- cancel-all-on-exit 仍未通过真实交易所残留订单检查。
```

- [ ] **Step 3: Run documentation sanity check**

```bash
python3 - <<'PY'
from pathlib import Path
p = Path('/Volumes/HY2TB/projects/micro-structure-alpha-lab/reports/round2/mid-price-exchange-structure/strikes/strike_08/STRIKE_08_ADAPTER_READINESS_AUDIT.md')
text = p.read_text(encoding='utf-8')
required = [
    'READY_TESTED_NON_LIVE',
    'FAIL_CLOSED_TESTED_NON_LIVE',
    'live_readiness',
    '未认证',
    '未连接 private WS',
    '未下单',
    '未撤单',
]
missing = [s for s in required if s not in text]
print({'line_count': len(text.splitlines()), 'missing': missing})
raise SystemExit(1 if missing else 0)
PY
```

Expected: `missing` is empty.

- [ ] **Step 4: Commit documentation update if the report directory is in a git repo**

First check:

```bash
git -C /Volumes/HY2TB/projects/micro-structure-alpha-lab rev-parse --show-toplevel
```

If it is not a git repo, do not force a commit. If it is a git repo and status is clean aside from intended docs:

```bash
git -C /Volumes/HY2TB/projects/micro-structure-alpha-lab add reports/round2/mid-price-exchange-structure/strikes/strike_08/STRIKE_08_ADAPTER_READINESS_AUDIT.md
git -C /Volumes/HY2TB/projects/micro-structure-alpha-lab commit -m "docs: update strike08 adapter readiness after tests"
```

---

## Final Verification Checklist

Before telling the user this blocker-cleanup stage is complete, fresh verification evidence is required:

```bash
cargo test -p nautilus-lighter convert_time_in_force --lib
cargo test -p nautilus-lighter convert_price_to_raw --lib
cargo test -p nautilus-lighter convert_quantity_to_raw --lib
cargo test -p nautilus-lighter validate_order_against_metadata --lib
cargo test -p nautilus-lighter ensure_no_reconciliation_risk --lib
cargo test -p nautilus-lighter --test execution_dispatch
cargo test -p nautilus-lighter --test execution_reports
rg -n "dotenv|API_KEY|PRIVATE_KEY|private_key|wallet|keyring|KMS|std::env::var|read_to_string\(" crates/adapters/lighter/src crates/adapters/lighter/tests
git status --short
git diff --stat
```

Completion claim rules:

- If all focused tests pass but `cargo test -p nautilus-lighter` was not run, say “focused tests passed,” not “all tests passed.”
- If live reports still return fail-closed errors, say “live reconciliation remains blocked but no longer silently succeeds.”
- Do not say Strike 08 is live-ready unless user-approved live/sandbox lifecycle rehearsal has actually happened.
- Do not run live/sandbox lifecycle rehearsal in this plan.

---

## Expected End State

After this plan is executed and verified:

```text
PostOnly/TIF=3 blocker: cleared at non-live unit-test level.
Precision/min-size blocker: partially cleared at non-live preflight/conversion level.
Live report blocker: converted from silent empty success to explicit fail-closed behavior; full live reports still not implemented.
Cancel-all-on-exit blocker: improved with reconciliation risk helpers; real exchange rehearsal still pending.
Safety gates: preserved and tested.
Strike 08 live run: still blocked until user explicitly approves a separate live/sandbox lifecycle rehearsal plan.
```
