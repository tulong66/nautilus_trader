// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

use nautilus_lighter::{
    common::LIGHTER_VENUE,
    execution::{
        fixtures::execution_fixture_set,
        reconciliation::ReconciledOrderStatus,
        replay::{
            PaperReplayFailureScenario, PaperReplayReportSnapshot, PaperReplayScenario,
            PaperReplaySoakConfig, PaperReplaySoakExperiment, check_paper_accounting_consistency,
            run_paper_replay_failure_scenario, run_paper_replay_scenario, run_paper_replay_soak,
        },
    },
};
use nautilus_model::identifiers::{AccountId, ClientId, InstrumentId};

fn fixture_account_id() -> AccountId {
    AccountId::new("FIXTURE-001")
}

fn fixture_client_id() -> ClientId {
    ClientId::new("LIGHTER")
}

fn fixture_instrument_id() -> InstrumentId {
    InstrumentId::from("ETH_USDC.LIGHTER")
}

#[test]
fn paper_replay_soak_replays_disconnects_resubscribe_duplicates_and_reports_consistently() {
    let fixtures = execution_fixture_set();
    let experiment = PaperReplaySoakExperiment::fixture_backed(
        "p2-q-fixture-soak",
        fixtures.clone(),
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    );

    let summary = run_paper_replay_soak(
        &experiment,
        PaperReplaySoakConfig {
            rounds: 3,
            include_disconnect_reconnect: true,
            include_duplicate_messages: true,
            include_empty_account_and_orders: true,
        },
    )
    .expect("paper replay soak summary");

    assert_eq!(summary.experiment_id, "p2-q-fixture-soak");
    assert_eq!(summary.rounds_completed, 3);
    assert_eq!(summary.reconnects, 3);
    assert_eq!(summary.resubscriptions, 3);
    assert!(summary.duplicate_messages >= fixtures.orders.len());
    assert_eq!(summary.empty_account_updates, 3);
    assert_eq!(summary.empty_order_snapshots, 3);
    assert_eq!(summary.final_order_count, fixtures.orders.len());
    assert_eq!(
        summary.final_order_status("fixture-client-filled"),
        Some(ReconciledOrderStatus::Filled)
    );
    assert_eq!(
        summary.final_order_status("fixture-client-cancel-rejected"),
        Some(ReconciledOrderStatus::CancelRejected)
    );
    assert_eq!(
        summary.account_timestamp("fixture-account"),
        Some(fixtures.account.timestamp_ms)
    );
    assert_eq!(summary.mass_status_order_reports, fixtures.orders.len());
    assert_eq!(summary.mass_status_fill_reports, 1);
    assert_eq!(summary.mass_status_position_reports, 1);
    assert!(summary.is_offline_only);
    assert!(summary.secret_material.is_empty());
    assert!(
        summary
            .reproducibility_record
            .contains("fixture:p2-q-fixture-soak")
    );
    assert!(summary.reproducibility_record.contains("rounds:3"));
    assert!(summary.reproducibility_record.contains("network:none"));
}

#[test]
fn paper_replay_harness_rejects_live_endpoint_or_credentials_before_execution() {
    let scenario = PaperReplayScenario::fixture_backed(
        "p3-a-live-reference-rejected",
        execution_fixture_set(),
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    )
    .with_report_snapshot(PaperReplayReportSnapshot::from_fixture_counts(6, 1, 1))
    .with_forbidden_reference("wss://testnet.zklighter.elliot.ai/stream");

    let err = run_paper_replay_scenario(&scenario).expect_err("live endpoint is rejected");

    assert!(err.to_string().contains("paper/replay scenario rejected"));
    assert!(err.to_string().contains("forbidden reference"));
}

#[test]
fn paper_replay_harness_emits_deterministic_operator_audit_summary() {
    let fixtures = execution_fixture_set();
    let mut scenario = PaperReplayScenario::fixture_backed(
        "p3-a-b-fixture-audit",
        fixtures.clone(),
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    )
    .with_report_snapshot(PaperReplayReportSnapshot::from_fixture_counts(6, 1, 1));
    scenario.include_duplicate_messages = true;
    scenario.include_stale_terminal_regression = true;

    let result = run_paper_replay_scenario(&scenario).expect("paper replay scenario result");
    let audit = result.audit_summary;

    assert_eq!(audit.scenario_name, "p3-a-b-fixture-audit");
    assert!(audit.paper_replay_only);
    assert_eq!(audit.lifecycle_counts.accepted, 1);
    assert_eq!(audit.lifecycle_counts.rejected, 1);
    assert_eq!(audit.lifecycle_counts.partially_filled, 1);
    assert_eq!(audit.lifecycle_counts.filled, 1);
    assert_eq!(audit.lifecycle_counts.canceled, 1);
    assert_eq!(audit.lifecycle_counts.cancel_rejected, 1);
    assert_eq!(
        audit.report_consistency.order_reports,
        fixtures.orders.len()
    );
    assert_eq!(audit.report_consistency.fill_reports, 1);
    assert_eq!(audit.report_consistency.position_reports, 1);
    assert!(audit.report_consistency.consistent);
    assert!(audit.duplicate_events >= fixtures.orders.len());
    assert_eq!(audit.stale_events, 1);
    assert_eq!(
        audit.account_timestamps.get("fixture-account"),
        Some(&fixtures.account.timestamp_ms)
    );
    assert!(audit.unresolved_anomalies.is_empty());

    let rendered = audit.render_text();
    assert!(rendered.contains("scenario=p3-a-b-fixture-audit"));
    assert!(rendered.contains("mode=paper_replay_only"));
    assert!(
        rendered.contains(
            "orders accepted=1 rejected=1 partial=1 filled=1 canceled=1 cancel_rejected=1"
        )
    );
    assert!(rendered.contains("reports orders=6 fills=1 positions=1 consistent=true"));
    assert!(rendered.contains("duplicates="));
    assert!(rendered.contains("stale=1"));
    assert!(rendered.contains("unresolved_anomalies=none"));
}

#[test]
fn operator_audit_identifies_report_count_mismatches() {
    let scenario = PaperReplayScenario::fixture_backed(
        "p3-b-report-mismatch",
        execution_fixture_set(),
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    )
    .with_report_snapshot(PaperReplayReportSnapshot::from_fixture_counts(99, 0, 0));

    let audit = run_paper_replay_scenario(&scenario)
        .expect("paper replay scenario result")
        .audit_summary;

    assert!(!audit.report_consistency.consistent);
    assert_eq!(audit.unresolved_anomalies.len(), 1);
    assert!(audit.unresolved_anomalies[0].contains("report_mismatch"));
    assert!(audit.unresolved_anomalies[0].contains("report_type=orders"));
    assert!(audit.unresolved_anomalies[0].contains("expected_orders=99"));
    assert!(audit.unresolved_anomalies[0].contains("actual_orders=6"));
    assert!(
        audit
            .render_text()
            .contains("unresolved_anomalies=report_mismatch")
    );
}
#[test]
fn failure_replay_pack_covers_retry_exhaustion_empty_snapshots_stale_duplicates_and_non_filled_sequencer_states()
 {
    let fixtures = execution_fixture_set();
    let scenario = PaperReplayFailureScenario::fixture_backed(
        "p3-c-failure-pack",
        fixtures.clone(),
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    )
    .with_retry_exhaustion("mock-report", 3)
    .with_mock_report_error("mock report error: unavailable");

    let result =
        run_paper_replay_failure_scenario(&scenario).expect("failure replay scenario result");

    assert_eq!(result.scenario_name, "p3-c-failure-pack");
    assert_eq!(result.disconnects, 1);
    assert_eq!(result.resubscriptions, 1);
    assert!(result.duplicate_events >= fixtures.orders.len());
    assert_eq!(result.stale_events, 2);
    assert_eq!(result.empty_account_updates, 1);
    assert_eq!(result.empty_order_snapshots, 1);
    assert_eq!(
        result.retry_exhausted.as_deref(),
        Some("mock-report attempts=3")
    );
    assert_eq!(
        result.mock_report_error.as_deref(),
        Some("mock report error: unavailable")
    );
    assert_eq!(
        result.non_filled_sequencer_states,
        vec!["submitted", "accepted", "pending", "timeout"]
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|line| line == "sequencer_non_filled status=pending")
    );
}

#[test]
fn paper_accounting_consistency_matches_replay_state_to_report_snapshot() {
    let fixtures = execution_fixture_set();
    let scenario = PaperReplayScenario::fixture_backed(
        "p3-d-accounting-success",
        fixtures.clone(),
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    );

    let consistency =
        check_paper_accounting_consistency(&scenario).expect("paper accounting consistency");

    assert!(consistency.consistent);
    assert!(consistency.diagnostics.is_empty());
    assert_eq!(consistency.replay_fills, 1);
    assert_eq!(consistency.report_fills, 1);
    assert_eq!(consistency.replay_positions, 1);
    assert_eq!(consistency.report_positions, 1);
    assert_eq!(
        consistency.replay_account_timestamp,
        Some(fixtures.account.timestamp_ms)
    );
    assert_eq!(
        consistency.report_account_timestamp,
        Some(fixtures.account.timestamp_ms)
    );
}

#[test]
fn paper_accounting_consistency_reports_deterministic_mismatch_diagnostics() {
    let mut fixtures = execution_fixture_set();
    fixtures.fill.quantity = "0.1250".to_string();
    fixtures.position.size = "0.2500".to_string();
    fixtures.account.timestamp_ms += 5;
    let scenario = PaperReplayScenario::fixture_backed(
        "p3-d-accounting-mismatch",
        fixtures,
        fixture_client_id(),
        fixture_account_id(),
        *LIGHTER_VENUE,
        fixture_instrument_id(),
    )
    .with_report_account_timestamp_ms(1_700_000_001_500);

    let consistency =
        check_paper_accounting_consistency(&scenario).expect("paper accounting consistency");

    assert!(!consistency.consistent);
    assert_eq!(
        consistency.diagnostics,
        vec![
            "fill_position_quantity_mismatch replay_fills=0.1250 report_position_size=0.2500".to_string(),
            "account_timestamp_mismatch replay_account_timestamp=1700000000105 report_account_timestamp=1700000001500".to_string(),
        ]
    );
}
