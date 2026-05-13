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
        replay::{PaperReplaySoakConfig, PaperReplaySoakExperiment, run_paper_replay_soak},
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
