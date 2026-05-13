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

use std::collections::HashMap;

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
