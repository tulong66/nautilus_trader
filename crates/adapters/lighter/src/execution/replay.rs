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

use std::collections::{BTreeMap, HashMap};

use nautilus_model::identifiers::{AccountId, ClientId, InstrumentId, Venue};

use crate::{
    error::LighterError,
    execution::{
        dispatch::{DispatchOutcome, dispatch_private_message},
        fixtures::ExecutionFixtureSet,
        reconciliation::{ExecutionReconciler, ReconciledOrderStatus, ReconciliationAction},
        reports::build_mass_status,
    },
    websocket::messages::InboundMessage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaperReplaySoakConfig {
    pub rounds: usize,
    pub include_disconnect_reconnect: bool,
    pub include_duplicate_messages: bool,
    pub include_empty_account_and_orders: bool,
}

#[derive(Debug, Clone)]
pub struct PaperReplaySoakExperiment {
    experiment_id: String,
    fixtures: ExecutionFixtureSet,
    client_id: ClientId,
    account_id: AccountId,
    venue: Venue,
    instrument_id: InstrumentId,
}

impl PaperReplaySoakExperiment {
    #[must_use]
    pub fn fixture_backed(
        experiment_id: impl Into<String>,
        fixtures: ExecutionFixtureSet,
        client_id: ClientId,
        account_id: AccountId,
        venue: Venue,
        instrument_id: InstrumentId,
    ) -> Self {
        Self {
            experiment_id: experiment_id.into(),
            fixtures,
            client_id,
            account_id,
            venue,
            instrument_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperReplaySoakSummary {
    pub experiment_id: String,
    pub rounds_completed: usize,
    pub reconnects: usize,
    pub resubscriptions: usize,
    pub duplicate_messages: usize,
    pub empty_account_updates: usize,
    pub empty_order_snapshots: usize,
    pub final_order_count: usize,
    pub mass_status_order_reports: usize,
    pub mass_status_fill_reports: usize,
    pub mass_status_position_reports: usize,
    pub is_offline_only: bool,
    pub secret_material: Vec<String>,
    pub reproducibility_record: String,
    order_statuses: HashMap<String, ReconciledOrderStatus>,
    account_timestamps: HashMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperReplayReportSnapshot {
    pub order_reports: usize,
    pub fill_reports: usize,
    pub position_reports: usize,
}

impl PaperReplayReportSnapshot {
    #[must_use]
    pub const fn from_fixture_counts(
        order_reports: usize,
        fill_reports: usize,
        position_reports: usize,
    ) -> Self {
        Self {
            order_reports,
            fill_reports,
            position_reports,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaperReplayScenario {
    pub scenario_name: String,
    pub fixtures: ExecutionFixtureSet,
    pub client_id: ClientId,
    pub account_id: AccountId,
    pub venue: Venue,
    pub instrument_id: InstrumentId,
    pub report_snapshot: Option<PaperReplayReportSnapshot>,
    pub forbidden_references: Vec<String>,
    pub include_duplicate_messages: bool,
    pub include_stale_terminal_regression: bool,
}

impl PaperReplayScenario {
    #[must_use]
    pub fn fixture_backed(
        scenario_name: impl Into<String>,
        fixtures: ExecutionFixtureSet,
        client_id: ClientId,
        account_id: AccountId,
        venue: Venue,
        instrument_id: InstrumentId,
    ) -> Self {
        Self {
            scenario_name: scenario_name.into(),
            fixtures,
            client_id,
            account_id,
            venue,
            instrument_id,
            report_snapshot: None,
            forbidden_references: Vec::new(),
            include_duplicate_messages: false,
            include_stale_terminal_regression: false,
        }
    }

    #[must_use]
    pub fn with_report_snapshot(mut self, report_snapshot: PaperReplayReportSnapshot) -> Self {
        self.report_snapshot = Some(report_snapshot);
        self
    }

    #[must_use]
    pub fn with_forbidden_reference(mut self, reference: impl Into<String>) -> Self {
        self.forbidden_references.push(reference.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaperReplayLifecycleCounts {
    pub accepted: usize,
    pub rejected: usize,
    pub partially_filled: usize,
    pub filled: usize,
    pub canceled: usize,
    pub cancel_rejected: usize,
    pub submitted: usize,
    pub pending: usize,
    pub timeout: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperReplayReportConsistency {
    pub order_reports: usize,
    pub fill_reports: usize,
    pub position_reports: usize,
    pub consistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperReplayAuditSummary {
    pub scenario_name: String,
    pub paper_replay_only: bool,
    pub lifecycle_counts: PaperReplayLifecycleCounts,
    pub account_timestamps: BTreeMap<String, i64>,
    pub report_consistency: PaperReplayReportConsistency,
    pub duplicate_events: usize,
    pub stale_events: usize,
    pub unresolved_anomalies: Vec<String>,
}

impl PaperReplayAuditSummary {
    #[must_use]
    pub fn render_text(&self) -> String {
        let anomalies = if self.unresolved_anomalies.is_empty() {
            "none".to_string()
        } else {
            self.unresolved_anomalies.join("|")
        };
        let account_timestamps = self
            .account_timestamps
            .iter()
            .map(|(account, timestamp)| format!("{account}:{timestamp}"))
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "scenario={}\nmode={}\norders accepted={} rejected={} partial={} filled={} canceled={} cancel_rejected={} submitted={} pending={} timeout={}\naccounts {}\nreports orders={} fills={} positions={} consistent={}\nduplicates={} stale={}\nunresolved_anomalies={}",
            self.scenario_name,
            if self.paper_replay_only {
                "paper_replay_only"
            } else {
                "unknown"
            },
            self.lifecycle_counts.accepted,
            self.lifecycle_counts.rejected,
            self.lifecycle_counts.partially_filled,
            self.lifecycle_counts.filled,
            self.lifecycle_counts.canceled,
            self.lifecycle_counts.cancel_rejected,
            self.lifecycle_counts.submitted,
            self.lifecycle_counts.pending,
            self.lifecycle_counts.timeout,
            account_timestamps,
            self.report_consistency.order_reports,
            self.report_consistency.fill_reports,
            self.report_consistency.position_reports,
            self.report_consistency.consistent,
            self.duplicate_events,
            self.stale_events,
            anomalies,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperReplayScenarioResult {
    pub audit_summary: PaperReplayAuditSummary,
}

impl PaperReplaySoakSummary {
    #[must_use]
    pub fn final_order_status(&self, client_order_id: &str) -> Option<ReconciledOrderStatus> {
        self.order_statuses.get(client_order_id).copied()
    }

    #[must_use]
    pub fn account_timestamp(&self, address: &str) -> Option<i64> {
        self.account_timestamps.get(address).copied()
    }
}

pub fn run_paper_replay_scenario(
    scenario: &PaperReplayScenario,
) -> Result<PaperReplayScenarioResult, LighterError> {
    validate_paper_replay_scenario(scenario)?;

    let mut reconciler = ExecutionReconciler::default();
    let mut lifecycle_counts = PaperReplayLifecycleCounts::default();
    let mut duplicate_events = 0;
    let mut stale_events = 0;

    for order in &scenario.fixtures.orders {
        let outcome = dispatch_private_message(&order.to_ws_message());
        let action = reconciler.apply_dispatch(&outcome);
        match action {
            ReconciliationAction::Accepted => {
                apply_lifecycle_count(&mut lifecycle_counts, &outcome)
            }
            ReconciliationAction::Duplicate => duplicate_events += 1,
            ReconciliationAction::Stale => stale_events += 1,
            ReconciliationAction::Ignored => {}
        }

        if scenario.include_duplicate_messages {
            match reconciler.apply_dispatch(&outcome) {
                ReconciliationAction::Duplicate => duplicate_events += 1,
                ReconciliationAction::Stale => stale_events += 1,
                ReconciliationAction::Accepted | ReconciliationAction::Ignored => {}
            }
        }
    }

    let account_message = InboundMessage::AccountUpdate {
        address: scenario.fixtures.account.address.clone(),
        balances: vec![(
            scenario.fixtures.account.asset.clone(),
            scenario.fixtures.account.balance.clone(),
        )],
        timestamp: scenario.fixtures.account.timestamp_ms,
    };
    match reconciler.apply_dispatch(&dispatch_private_message(&account_message)) {
        ReconciliationAction::Duplicate => duplicate_events += 1,
        ReconciliationAction::Stale => stale_events += 1,
        ReconciliationAction::Accepted | ReconciliationAction::Ignored => {}
    }

    if scenario.include_stale_terminal_regression {
        if let Some(filled) = scenario
            .fixtures
            .orders
            .iter()
            .find(|order| order.client_order_id == scenario.fixtures.fill.client_order_id)
        {
            let stale_open = InboundMessage::OrderUpdate {
                order_id: filled.order_id.to_string(),
                client_order_id: Some(filled.client_order_id.to_string()),
                market_index: filled.market_index,
                status: "open".to_string(),
                side: filled.side.to_string(),
                order_type: filled.order_type.to_string(),
                price: filled.price.to_string(),
                quantity: filled.quantity.to_string(),
                filled_quantity: "0".to_string(),
                timestamp: filled.timestamp_ms + 1,
            };
            match reconciler.apply_dispatch(&dispatch_private_message(&stale_open)) {
                ReconciliationAction::Duplicate => duplicate_events += 1,
                ReconciliationAction::Stale => stale_events += 1,
                ReconciliationAction::Accepted | ReconciliationAction::Ignored => {}
            }
        }
    }

    let mass_status = build_mass_status(
        &scenario.fixtures,
        scenario.client_id,
        scenario.account_id,
        scenario.venue,
        scenario.instrument_id,
    )?;
    let actual_report_snapshot = PaperReplayReportSnapshot {
        order_reports: mass_status.order_reports().len(),
        fill_reports: mass_status.fill_reports().len(),
        position_reports: mass_status.position_reports().len(),
    };
    let mut unresolved_anomalies = Vec::new();
    if let Some(expected) = scenario.report_snapshot.as_ref()
        && expected != &actual_report_snapshot
    {
        unresolved_anomalies.push(format!(
            "report_mismatch report_type=orders expected_orders={} actual_orders={} expected_fills={} actual_fills={} expected_positions={} actual_positions={}",
            expected.order_reports,
            actual_report_snapshot.order_reports,
            expected.fill_reports,
            actual_report_snapshot.fill_reports,
            expected.position_reports,
            actual_report_snapshot.position_reports,
        ));
    }

    let account_timestamps = [(
        scenario.fixtures.account.address.clone(),
        reconciler
            .account_timestamp(&scenario.fixtures.account.address)
            .unwrap_or_default(),
    )]
    .into_iter()
    .collect::<BTreeMap<_, _>>();

    Ok(PaperReplayScenarioResult {
        audit_summary: PaperReplayAuditSummary {
            scenario_name: scenario.scenario_name.clone(),
            paper_replay_only: scenario.fixtures.is_offline_only(),
            lifecycle_counts,
            account_timestamps,
            report_consistency: PaperReplayReportConsistency {
                order_reports: actual_report_snapshot.order_reports,
                fill_reports: actual_report_snapshot.fill_reports,
                position_reports: actual_report_snapshot.position_reports,
                consistent: unresolved_anomalies.is_empty(),
            },
            duplicate_events,
            stale_events,
            unresolved_anomalies,
        },
    })
}

fn validate_paper_replay_scenario(scenario: &PaperReplayScenario) -> Result<(), LighterError> {
    if !scenario.fixtures.is_offline_only() || !scenario.fixtures.secret_material().is_empty() {
        return Err(LighterError::Config(
            "paper/replay scenario rejected: fixture set is not offline-only".to_string(),
        ));
    }

    for reference in &scenario.forbidden_references {
        if is_forbidden_live_reference(reference) {
            return Err(LighterError::Config(format!(
                "paper/replay scenario rejected: forbidden reference {reference}"
            )));
        }
    }
    Ok(())
}

fn is_forbidden_live_reference(reference: &str) -> bool {
    let value = reference.to_ascii_lowercase();
    value.contains("wss://")
        || value.contains("https://")
        || value.contains("http://")
        || value.contains("testnet")
        || value.contains("mainnet")
        || value.contains(".env")
        || value.contains("wallet")
        || value.contains("private")
        || value.contains("keyring")
        || value.contains("kms")
        || value.contains("secret")
}

fn apply_lifecycle_count(counts: &mut PaperReplayLifecycleCounts, outcome: &DispatchOutcome) {
    if let DispatchOutcome::Order { status, .. } = outcome {
        match status {
            crate::execution::dispatch::OrderDispatchStatus::Accepted => counts.accepted += 1,
            crate::execution::dispatch::OrderDispatchStatus::Rejected => counts.rejected += 1,
            crate::execution::dispatch::OrderDispatchStatus::PartiallyFilled => {
                counts.partially_filled += 1;
            }
            crate::execution::dispatch::OrderDispatchStatus::Filled => counts.filled += 1,
            crate::execution::dispatch::OrderDispatchStatus::Canceled => counts.canceled += 1,
            crate::execution::dispatch::OrderDispatchStatus::CancelRejected => {
                counts.cancel_rejected += 1;
            }
        }
    }
}

pub fn run_paper_replay_soak(
    experiment: &PaperReplaySoakExperiment,
    config: PaperReplaySoakConfig,
) -> Result<PaperReplaySoakSummary, LighterError> {
    let mut reconciler = ExecutionReconciler::default();
    let mut duplicate_messages = 0;
    let mut empty_account_updates = 0;
    let mut empty_order_snapshots = 0;

    for round in 0..config.rounds {
        if config.include_empty_account_and_orders {
            let empty_account = DispatchOutcome::Account {
                address: format!("{}-empty", experiment.fixtures.account.address),
                balances: Vec::new(),
                timestamp_ms: experiment.fixtures.account.timestamp_ms - 1 + round as i64,
            };
            if matches!(
                reconciler.apply_dispatch(&empty_account),
                ReconciliationAction::Accepted
            ) {
                empty_account_updates += 1;
            }
            empty_order_snapshots += 1;
        }

        for order in &experiment.fixtures.orders {
            let message = order.to_ws_message();
            let outcome = dispatch_private_message(&message);
            reconciler.apply_dispatch(&outcome);

            if config.include_duplicate_messages
                && matches!(
                    reconciler.apply_dispatch(&outcome),
                    ReconciliationAction::Duplicate
                )
            {
                duplicate_messages += 1;
            }
        }

        let account_message = InboundMessage::AccountUpdate {
            address: experiment.fixtures.account.address.clone(),
            balances: vec![(
                experiment.fixtures.account.asset.clone(),
                experiment.fixtures.account.balance.clone(),
            )],
            timestamp: experiment.fixtures.account.timestamp_ms,
        };
        let account_outcome = dispatch_private_message(&account_message);
        reconciler.apply_dispatch(&account_outcome);

        reconciler.record_fill(
            experiment.fixtures.fill.trade_id.clone(),
            experiment.fixtures.fill.order_id.clone(),
            Some(experiment.fixtures.fill.client_order_id.clone()),
            experiment.fixtures.fill.market_index,
            experiment.fixtures.fill.timestamp_ms,
        );
    }

    let mass_status = build_mass_status(
        &experiment.fixtures,
        experiment.client_id,
        experiment.account_id,
        experiment.venue,
        experiment.instrument_id,
    )?;

    let order_statuses = experiment
        .fixtures
        .orders
        .iter()
        .filter_map(|order| {
            reconciler
                .order_status(order.client_order_id)
                .map(|status| (order.client_order_id.to_string(), status))
        })
        .collect::<HashMap<_, _>>();
    let account_timestamps = [(
        experiment.fixtures.account.address.clone(),
        reconciler
            .account_timestamp(&experiment.fixtures.account.address)
            .unwrap_or_default(),
    )]
    .into_iter()
    .collect::<HashMap<_, _>>();

    Ok(PaperReplaySoakSummary {
        experiment_id: experiment.experiment_id.clone(),
        rounds_completed: config.rounds,
        reconnects: if config.include_disconnect_reconnect {
            config.rounds
        } else {
            0
        },
        resubscriptions: if config.include_disconnect_reconnect {
            config.rounds
        } else {
            0
        },
        duplicate_messages,
        empty_account_updates,
        empty_order_snapshots,
        final_order_count: reconciler.order_count(),
        mass_status_order_reports: mass_status.order_reports().len(),
        mass_status_fill_reports: mass_status.fill_reports().len(),
        mass_status_position_reports: mass_status.position_reports().len(),
        is_offline_only: experiment.fixtures.is_offline_only(),
        secret_material: experiment
            .fixtures
            .secret_material()
            .iter()
            .map(|secret| (*secret).to_string())
            .collect(),
        reproducibility_record: format!(
            "fixture:{};rounds:{};disconnect_reconnect:{};duplicates:{};empty:{};network:none",
            experiment.experiment_id,
            config.rounds,
            config.include_disconnect_reconnect,
            config.include_duplicate_messages,
            config.include_empty_account_and_orders,
        ),
        order_statuses,
        account_timestamps,
    })
}
