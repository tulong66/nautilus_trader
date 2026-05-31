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

use std::collections::{HashMap, HashSet};

use crate::{
    execution::dispatch::{DispatchOutcome, OrderDispatchStatus},
    http::types::TxResponse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReconciledOrderStatus {
    Sent,
    Submitted,
    Accepted,
    Executed,
    Pending,
    Timeout,
    PartiallyFilled,
    Filled,
    CancelPending,
    Canceled,
    Rejected,
    CancelRejected,
}

impl ReconciledOrderStatus {
    #[must_use]
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

    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Executed | Self::Filled | Self::Canceled | Self::Rejected)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReconciliationAction {
    Accepted,
    Duplicate,
    Stale,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequencerExecutionStatus {
    Submitted,
    Accepted,
    Executed,
    Rejected,
    Pending,
    Timeout,
    Unknown,
    Filled,
}

#[must_use]
pub fn interpret_send_tx_response(response: &TxResponse) -> SequencerExecutionStatus {
    if !response.success {
        return SequencerExecutionStatus::Rejected;
    }

    let Some(data) = response.data.as_ref() else {
        return SequencerExecutionStatus::Unknown;
    };

    match data
        .status
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("executed") => SequencerExecutionStatus::Executed,
        Some("rejected" | "reject" | "failed" | "error") => SequencerExecutionStatus::Rejected,
        Some("pending") => SequencerExecutionStatus::Pending,
        Some("timeout" | "timed_out") => SequencerExecutionStatus::Timeout,
        Some("submitted") => SequencerExecutionStatus::Submitted,
        Some("accepted") => SequencerExecutionStatus::Accepted,
        Some(_) => SequencerExecutionStatus::Unknown,
        None if data.code == Some(200) || data.tx_id.is_some() || data.order_index.is_some() => {
            SequencerExecutionStatus::Accepted
        }
        None => SequencerExecutionStatus::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReconciledOrderState {
    status: ReconciledOrderStatus,
    market_index: u16,
    last_timestamp_ms: i64,
    venue_order_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionReconciler {
    orders: HashMap<String, ReconciledOrderState>,
    venue_to_client: HashMap<String, String>,
    seen_events: HashSet<ReconciliationKey>,
    accounts: HashMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ReconciliationKey {
    Send {
        client_order_id: String,
        timestamp_ms: i64,
    },
    Cancel {
        client_order_id: String,
        timestamp_ms: i64,
    },
    SendTx {
        client_order_id: String,
        timestamp_ms: i64,
        status: SequencerExecutionStatus,
    },
    OrderUpdate {
        order_id: String,
        client_order_id: Option<String>,
        status: OrderDispatchStatus,
        timestamp_ms: i64,
    },
    Fill {
        trade_id: String,
    },
    Account {
        address: String,
        timestamp_ms: i64,
    },
}

impl ExecutionReconciler {
    pub fn record_send(
        &mut self,
        client_order_id: impl Into<String>,
        market_index: u16,
        timestamp_ms: i64,
    ) -> ReconciliationAction {
        let client_order_id = client_order_id.into();
        let key = ReconciliationKey::Send {
            client_order_id: client_order_id.clone(),
            timestamp_ms,
        };
        if !self.seen_events.insert(key) {
            return ReconciliationAction::Duplicate;
        }

        self.upsert_order(
            client_order_id,
            None,
            market_index,
            ReconciledOrderStatus::Sent,
            timestamp_ms,
        )
    }

    pub fn record_send_tx_response(
        &mut self,
        client_order_id: impl Into<String>,
        market_index: u16,
        timestamp_ms: i64,
        response: &TxResponse,
    ) -> ReconciliationAction {
        let client_order_id = client_order_id.into();
        let status = interpret_send_tx_response(response);
        let key = ReconciliationKey::SendTx {
            client_order_id: client_order_id.clone(),
            timestamp_ms,
            status,
        };
        if !self.seen_events.insert(key) {
            return ReconciliationAction::Duplicate;
        }

        self.upsert_order(
            client_order_id,
            response
                .data
                .as_ref()
                .and_then(|data| data.order_index)
                .map(|order_index| order_index.to_string()),
            market_index,
            reconciled_status_from_send_tx(status),
            timestamp_ms,
        )
    }

    pub fn record_cancel_request(
        &mut self,
        client_order_id: impl Into<String>,
        market_index: u16,
        timestamp_ms: i64,
    ) -> ReconciliationAction {
        let client_order_id = client_order_id.into();
        let key = ReconciliationKey::Cancel {
            client_order_id: client_order_id.clone(),
            timestamp_ms,
        };
        if !self.seen_events.insert(key) {
            return ReconciliationAction::Duplicate;
        }

        self.upsert_order(
            client_order_id,
            None,
            market_index,
            ReconciledOrderStatus::CancelPending,
            timestamp_ms,
        )
    }

    pub fn record_fill(
        &mut self,
        trade_id: impl Into<String>,
        venue_order_id: impl Into<String>,
        client_order_id: Option<impl Into<String>>,
        market_index: u16,
        timestamp_ms: i64,
    ) -> ReconciliationAction {
        let trade_id = trade_id.into();
        let key = ReconciliationKey::Fill { trade_id };
        if !self.seen_events.insert(key) {
            return ReconciliationAction::Duplicate;
        }

        let venue_order_id = venue_order_id.into();
        let client_order_id = client_order_id
            .map(Into::into)
            .or_else(|| self.venue_to_client.get(&venue_order_id).cloned())
            .unwrap_or_else(|| venue_order_id.clone());

        self.upsert_order(
            client_order_id,
            Some(venue_order_id),
            market_index,
            ReconciledOrderStatus::Filled,
            timestamp_ms,
        )
    }

    pub fn apply_dispatch(&mut self, outcome: &DispatchOutcome) -> ReconciliationAction {
        match outcome {
            DispatchOutcome::Order {
                order_id,
                client_order_id,
                market_index,
                status,
                timestamp_ms,
                ..
            } => {
                let key = ReconciliationKey::OrderUpdate {
                    order_id: order_id.clone(),
                    client_order_id: client_order_id.clone(),
                    status: *status,
                    timestamp_ms: *timestamp_ms,
                };
                if !self.seen_events.insert(key) {
                    return ReconciliationAction::Duplicate;
                }

                let client_order_id = client_order_id
                    .clone()
                    .or_else(|| self.venue_to_client.get(order_id).cloned())
                    .unwrap_or_else(|| order_id.clone());
                self.upsert_order(
                    client_order_id,
                    Some(order_id.clone()),
                    *market_index,
                    reconciled_status_from_dispatch(*status),
                    *timestamp_ms,
                )
            }
            DispatchOutcome::Account {
                address,
                timestamp_ms,
                ..
            } => {
                let key = ReconciliationKey::Account {
                    address: address.clone(),
                    timestamp_ms: *timestamp_ms,
                };
                if !self.seen_events.insert(key) {
                    return ReconciliationAction::Duplicate;
                }
                match self.accounts.get(address) {
                    Some(previous) if *previous > *timestamp_ms => ReconciliationAction::Stale,
                    _ => {
                        self.accounts.insert(address.clone(), *timestamp_ms);
                        ReconciliationAction::Accepted
                    }
                }
            }
            DispatchOutcome::Ignored => ReconciliationAction::Ignored,
        }
    }

    #[must_use]
    pub fn order_status(&self, client_order_id: &str) -> Option<ReconciledOrderStatus> {
        self.orders.get(client_order_id).map(|state| state.status)
    }

    #[must_use]
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    #[must_use]
    pub fn orders_requiring_reconciliation(&self) -> Vec<(String, ReconciledOrderStatus)> {
        self.orders
            .iter()
            .filter_map(|(client_order_id, state)| {
                state
                    .status
                    .requires_reconciliation()
                    .then(|| (client_order_id.clone(), state.status))
            })
            .collect()
    }

    #[must_use]
    pub fn has_reconciliation_risk(&self) -> bool {
        self.orders
            .values()
            .any(|state| state.status.requires_reconciliation())
    }

    #[cfg(test)]
    pub(crate) fn record_order_status_for_test(
        &mut self,
        client_order_id: impl Into<String>,
        status: ReconciledOrderStatus,
    ) {
        self.orders.insert(
            client_order_id.into(),
            ReconciledOrderState {
                status,
                market_index: 0,
                last_timestamp_ms: 0,
                venue_order_id: None,
            },
        );
    }

    #[must_use]
    pub fn account_timestamp(&self, address: &str) -> Option<i64> {
        self.accounts.get(address).copied()
    }

    fn upsert_order(
        &mut self,
        client_order_id: String,
        venue_order_id: Option<String>,
        market_index: u16,
        next_status: ReconciledOrderStatus,
        timestamp_ms: i64,
    ) -> ReconciliationAction {
        let previous = self.orders.get(&client_order_id).cloned();
        if let Some(previous) = previous.as_ref() {
            if timestamp_ms < previous.last_timestamp_ms {
                return ReconciliationAction::Stale;
            }
            if is_regressive(previous.status, next_status) {
                return ReconciliationAction::Stale;
            }
        }

        if let Some(venue_order_id) = venue_order_id.as_ref() {
            self.venue_to_client
                .insert(venue_order_id.clone(), client_order_id.clone());
        }

        let state = ReconciledOrderState {
            status: next_status,
            market_index,
            last_timestamp_ms: timestamp_ms,
            venue_order_id,
        };
        self.orders.insert(client_order_id, state);
        ReconciliationAction::Accepted
    }
}

fn reconciled_status_from_send_tx(status: SequencerExecutionStatus) -> ReconciledOrderStatus {
    match status {
        SequencerExecutionStatus::Submitted => ReconciledOrderStatus::Submitted,
        SequencerExecutionStatus::Accepted => ReconciledOrderStatus::Accepted,
        SequencerExecutionStatus::Executed => ReconciledOrderStatus::Executed,
        SequencerExecutionStatus::Rejected => ReconciledOrderStatus::Rejected,
        SequencerExecutionStatus::Pending => ReconciledOrderStatus::Pending,
        SequencerExecutionStatus::Timeout | SequencerExecutionStatus::Unknown => {
            ReconciledOrderStatus::Timeout
        }
        SequencerExecutionStatus::Filled => ReconciledOrderStatus::Filled,
    }
}

fn reconciled_status_from_dispatch(status: OrderDispatchStatus) -> ReconciledOrderStatus {
    match status {
        OrderDispatchStatus::Accepted => ReconciledOrderStatus::Accepted,
        OrderDispatchStatus::Rejected => ReconciledOrderStatus::Rejected,
        OrderDispatchStatus::PartiallyFilled => ReconciledOrderStatus::PartiallyFilled,
        OrderDispatchStatus::Filled => ReconciledOrderStatus::Filled,
        OrderDispatchStatus::Canceled => ReconciledOrderStatus::Canceled,
        OrderDispatchStatus::CancelRejected => ReconciledOrderStatus::CancelRejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciled_order_status_classifies_terminal_and_risk_states() {
        assert!(ReconciledOrderStatus::Executed.is_terminal());
        assert!(ReconciledOrderStatus::Filled.is_terminal());
        assert!(ReconciledOrderStatus::Canceled.is_terminal());
        assert!(ReconciledOrderStatus::Rejected.is_terminal());

        assert!(!ReconciledOrderStatus::Executed.requires_reconciliation());
        assert!(!ReconciledOrderStatus::Filled.requires_reconciliation());
        assert!(!ReconciledOrderStatus::Canceled.requires_reconciliation());
        assert!(!ReconciledOrderStatus::Rejected.requires_reconciliation());

        assert!(ReconciledOrderStatus::Sent.requires_reconciliation());
        assert!(ReconciledOrderStatus::Submitted.requires_reconciliation());
        assert!(ReconciledOrderStatus::Accepted.requires_reconciliation());
        assert!(ReconciledOrderStatus::Pending.requires_reconciliation());
        assert!(ReconciledOrderStatus::Timeout.requires_reconciliation());
        assert!(ReconciledOrderStatus::PartiallyFilled.requires_reconciliation());
        assert!(ReconciledOrderStatus::CancelPending.requires_reconciliation());
        assert!(ReconciledOrderStatus::CancelRejected.requires_reconciliation());
    }

    #[test]
    fn reconciler_reports_orders_requiring_reconciliation() {
        let mut reconciler = ExecutionReconciler::default();
        reconciler.record_order_status_for_test("OPEN", ReconciledOrderStatus::Accepted);
        reconciler.record_order_status_for_test("DONE", ReconciledOrderStatus::Canceled);

        assert!(reconciler.has_reconciliation_risk());
        let risky = reconciler.orders_requiring_reconciliation();
        assert_eq!(risky, vec![("OPEN".to_string(), ReconciledOrderStatus::Accepted)]);
    }
}

fn is_regressive(previous: ReconciledOrderStatus, next: ReconciledOrderStatus) -> bool {
    match previous {
        ReconciledOrderStatus::Filled
        | ReconciledOrderStatus::Canceled
        | ReconciledOrderStatus::Rejected => {
            !matches!(previous, ReconciledOrderStatus::CancelRejected) && previous != next
        }
        ReconciledOrderStatus::PartiallyFilled => matches!(
            next,
            ReconciledOrderStatus::Sent
                | ReconciledOrderStatus::Submitted
                | ReconciledOrderStatus::Accepted
                | ReconciledOrderStatus::Pending
                | ReconciledOrderStatus::Timeout
        ),
        ReconciledOrderStatus::CancelRejected => matches!(
            next,
            ReconciledOrderStatus::Sent
                | ReconciledOrderStatus::Submitted
                | ReconciledOrderStatus::CancelPending
        ),
        ReconciledOrderStatus::CancelPending => matches!(
            next,
            ReconciledOrderStatus::Sent | ReconciledOrderStatus::Submitted
        ),
        ReconciledOrderStatus::Timeout => matches!(next, ReconciledOrderStatus::Sent),
        ReconciledOrderStatus::Pending => matches!(next, ReconciledOrderStatus::Sent),
        ReconciledOrderStatus::Executed => matches!(
            next,
            ReconciledOrderStatus::Sent
                | ReconciledOrderStatus::Submitted
                | ReconciledOrderStatus::Accepted
                | ReconciledOrderStatus::Pending
                | ReconciledOrderStatus::Timeout
        ),
        ReconciledOrderStatus::Accepted => matches!(
            next,
            ReconciledOrderStatus::Sent | ReconciledOrderStatus::Submitted
        ),
        ReconciledOrderStatus::Submitted => matches!(next, ReconciledOrderStatus::Sent),
        ReconciledOrderStatus::Sent => false,
    }
}
