# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2026 Nautech Systems Pty Ltd. All rights reserved.
#  https://nautechsystems.io
#
#  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
#  You may not use this file except in compliance with the License.
#  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
#
#  Unless required by applicable law or agreed to in writing, software
#  distributed under the License is distributed on an "AS IS" BASIS,
#  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#  See the License for the specific language governing permissions and
#  limitations under the License.
# -------------------------------------------------------------------------------------------------

from pathlib import Path

import pandas as pd
import pytest

from examples.backtest.lighter_min_size_cost_replay import build_fill_feasible_runs
from examples.backtest.lighter_min_size_cost_replay import build_low_spread_runs
from examples.backtest.lighter_min_size_cost_replay import fill_feasible_ratio
from examples.backtest.lighter_min_size_cost_replay import hyperliquid_taker_baseline_usd_per_btc
from examples.backtest.lighter_min_size_cost_replay import load_execution_view
from examples.backtest.lighter_min_size_cost_replay import simulate_round_trips


def _write_execution_view(path: Path) -> None:
    rows = [
        {
            "timestamp": "2026-05-30T00:00:00Z",
            "reference_mid": 100.0,
            "reference_bid": 99.5,
            "reference_ask": 100.5,
            "executable_bid": 99.8,
            "executable_ask": 100.2,
            "execution_spread": 0.4,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:10Z",
            "reference_mid": 101.0,
            "reference_bid": 100.5,
            "reference_ask": 101.5,
            "executable_bid": 98.0,
            "executable_ask": 104.0,
            "execution_spread": 6.0,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:30Z",
            "reference_mid": 101.0,
            "reference_bid": 100.5,
            "reference_ask": 101.5,
            "executable_bid": 100.7,
            "executable_ask": 101.2,
            "execution_spread": 0.5,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:40Z",
            "reference_mid": 101.5,
            "reference_bid": 101.0,
            "reference_ask": 102.0,
            "executable_bid": 98.0,
            "executable_ask": 104.0,
            "execution_spread": 6.0,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:01:00Z",
            "reference_mid": 102.0,
            "reference_bid": 101.5,
            "reference_ask": 102.5,
            "executable_bid": 101.7,
            "executable_ask": 102.2,
            "execution_spread": 0.5,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
    ]
    pd.DataFrame(rows).to_csv(path, index=False)


def _write_latency_view(path: Path) -> None:
    rows = [
        {
            "timestamp": "2026-05-30T00:00:00Z",
            "reference_mid": 100.0,
            "reference_bid": 99.5,
            "reference_ask": 100.5,
            "executable_bid": 99.8,
            "executable_ask": 100.2,
            "execution_spread": 0.4,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:01Z",
            "reference_mid": 100.0,
            "reference_bid": 99.5,
            "reference_ask": 100.5,
            "executable_bid": 99.7,
            "executable_ask": 100.3,
            "execution_spread": 0.6,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.005,
        },
        {
            "timestamp": "2026-05-30T00:00:02Z",
            "reference_mid": 100.0,
            "reference_bid": 99.5,
            "reference_ask": 100.5,
            "executable_bid": 99.7,
            "executable_ask": 100.4,
            "execution_spread": 0.7,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:22Z",
            "reference_mid": 101.0,
            "reference_bid": 100.5,
            "reference_ask": 101.5,
            "executable_bid": 100.8,
            "executable_ask": 101.2,
            "execution_spread": 0.4,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:23Z",
            "reference_mid": 101.0,
            "reference_bid": 100.5,
            "reference_ask": 101.5,
            "executable_bid": 100.7,
            "executable_ask": 101.3,
            "execution_spread": 0.6,
            "lighter_bid_sz": 0.005,
            "lighter_ask_sz": 0.20,
        },
        {
            "timestamp": "2026-05-30T00:00:24Z",
            "reference_mid": 101.0,
            "reference_bid": 100.5,
            "reference_ask": 101.5,
            "executable_bid": 100.6,
            "executable_ask": 101.4,
            "execution_spread": 0.8,
            "lighter_bid_sz": 0.20,
            "lighter_ask_sz": 0.20,
        },
    ]
    pd.DataFrame(rows).to_csv(path, index=False)


def test_load_execution_view_and_low_spread_runs(tmp_path: Path) -> None:
    input_path = tmp_path / "execution_view.csv"
    _write_execution_view(input_path)

    df = load_execution_view(input_path)
    runs = build_low_spread_runs(df, threshold_usd=1.0)

    assert len(df) == 5
    assert len(runs) == 3
    assert runs[0]["rows"] == 1
    assert runs[0]["min_spread_usd"] == 0.4


def test_load_execution_view_defaults_missing_sizes_to_unbounded(tmp_path: Path) -> None:
    input_path = tmp_path / "execution_view.csv"
    _write_execution_view(input_path)
    raw = pd.read_csv(input_path).drop(columns=["lighter_bid_sz", "lighter_ask_sz"])
    raw.to_csv(input_path, index=False)

    df = load_execution_view(input_path)

    assert df["lighter_bid_sz"].iloc[0] == float("inf")
    assert df["lighter_ask_sz"].iloc[0] == float("inf")


def test_fill_feasible_runs_require_side_size(tmp_path: Path) -> None:
    input_path = tmp_path / "execution_view.csv"
    _write_latency_view(input_path)
    df = load_execution_view(input_path)

    buy_runs = build_fill_feasible_runs(df, threshold_usd=1.0, quantity_btc=0.01, side="buy")
    sell_runs = build_fill_feasible_runs(df, threshold_usd=1.0, quantity_btc=0.01, side="sell")

    assert fill_feasible_ratio(df, threshold_usd=1.0, quantity_btc=0.01, side="buy") == pytest.approx(5 / 6)
    assert fill_feasible_ratio(df, threshold_usd=1.0, quantity_btc=0.01, side="sell") == pytest.approx(5 / 6)
    assert len(buy_runs) == 2
    assert len(sell_runs) == 2


def test_simulate_round_trip_waits_for_entry_and_exit_low_spread(tmp_path: Path) -> None:
    input_path = tmp_path / "execution_view.csv"
    _write_execution_view(input_path)
    df = load_execution_view(input_path)

    events = simulate_round_trips(
        df,
        threshold_usd=1.0,
        quantity_btc=0.01,
        hold_sec=20.0,
        max_wait_sec=40.0,
        hyperliquid_taker_fee_bps=2.5,
    )

    assert len(events) == 1
    first = events[0]
    assert first.decision_to_entry_wait_sec == 0.0
    assert first.exit_wait_sec == 10.0
    assert first.entry_latency_sec == 0.0
    assert first.exit_latency_sec == 0.0
    assert first.entry_lighter_spread == 0.4
    assert first.exit_lighter_spread == 0.5
    assert first.entry_lighter_ask_size_btc == 0.20
    assert first.exit_lighter_bid_size_btc == 0.20
    assert first.lighter_execution_cost_usd_per_btc == 0.5
    assert first.lighter_execution_cost_usd_total == 0.005
    assert first.inventory_markout_usd_per_btc == 1.0
    assert first.hyperliquid_baseline_cost_usd_total == 0.0105
    assert first.cost_saving_vs_hyperliquid_usd_total == pytest.approx(0.0055)


def test_simulate_round_trip_applies_latency_and_min_size(tmp_path: Path) -> None:
    input_path = tmp_path / "execution_view.csv"
    _write_latency_view(input_path)
    df = load_execution_view(input_path)

    events = simulate_round_trips(
        df,
        threshold_usd=1.0,
        quantity_btc=0.01,
        hold_sec=20.0,
        max_wait_sec=30.0,
        hyperliquid_taker_fee_bps=2.5,
        lighter_fee_bps=1.0,
        latency_sec=1.0,
    )

    assert len(events) == 1
    first = events[0]
    assert first.entry_signal_timestamp == "2026-05-30T00:00:01+00:00"
    assert first.entry_timestamp == "2026-05-30T00:00:02+00:00"
    assert first.exit_signal_timestamp == "2026-05-30T00:00:23+00:00"
    assert first.exit_timestamp == "2026-05-30T00:00:24+00:00"
    assert first.entry_latency_sec == 1.0
    assert first.exit_latency_sec == 1.0
    assert first.entry_lighter_ask_size_btc == 0.20
    assert first.exit_lighter_bid_size_btc == 0.20
    assert first.lighter_fee_cost_usd_per_btc == pytest.approx((100.4 + 100.6) * 1.0 / 10_000.0)
    assert first.lighter_execution_cost_usd_per_btc == pytest.approx(0.8201)


def test_hyperliquid_baseline_includes_spread_and_two_taker_fees() -> None:
    row = pd.Series({"reference_bid": 99.5, "reference_ask": 100.5, "reference_mid": 100.0})

    assert hyperliquid_taker_baseline_usd_per_btc(row, taker_fee_bps=2.5) == 1.05
