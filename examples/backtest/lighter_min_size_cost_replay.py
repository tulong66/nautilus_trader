#!/usr/bin/env python3
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
"""
Paper replay for Lighter low-spread minimum-size execution-cost experiments.

Safety boundaries:
- reads local cross-venue execution_view CSV only
- no .env / API key / wallet / private key / keyring / KMS access
- no authenticated requests, private WebSocket, order placement, cancel, transfer, or withdrawal

This script is intentionally not a live Nautilus node. It is the first paper/dry-run step before any
future testnet/live credential design. It answers: "if we wait for a Lighter fill-feasible low-spread
window, how long do we wait and what execution-cost proxy do we get versus a Hyperliquid taker
baseline?"
"""
from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from dataclasses import dataclass
from datetime import UTC
from datetime import datetime
from pathlib import Path
from typing import Any

import pandas as pd


REQUIRED_COLUMNS = {
    "timestamp",
    "reference_mid",
    "reference_bid",
    "reference_ask",
    "executable_bid",
    "executable_ask",
    "execution_spread",
}
OPTIONAL_SIZE_COLUMNS = ["lighter_bid_sz", "lighter_ask_sz"]


@dataclass(frozen=True)
class PaperRoundTripEvent:
    threshold_usd: float
    decision_timestamp: str
    entry_signal_timestamp: str
    entry_timestamp: str
    exit_target_timestamp: str
    exit_signal_timestamp: str
    exit_timestamp: str
    decision_to_entry_wait_sec: float
    entry_latency_sec: float
    exit_wait_sec: float
    exit_latency_sec: float
    total_elapsed_sec: float
    quantity_btc: float
    entry_reference_mid: float
    exit_reference_mid: float
    entry_lighter_bid: float
    entry_lighter_ask: float
    exit_lighter_bid: float
    exit_lighter_ask: float
    entry_lighter_bid_size_btc: float
    entry_lighter_ask_size_btc: float
    exit_lighter_bid_size_btc: float
    exit_lighter_ask_size_btc: float
    entry_lighter_spread: float
    exit_lighter_spread: float
    lighter_fee_bps: float
    lighter_fee_cost_usd_per_btc: float
    lighter_buy_cost_vs_ref_usd_per_btc: float
    lighter_sell_cost_vs_ref_usd_per_btc: float
    lighter_execution_cost_usd_per_btc: float
    lighter_execution_cost_usd_total: float
    lighter_realized_round_trip_cost_usd_per_btc: float
    lighter_realized_round_trip_cost_usd_total: float
    inventory_markout_usd_per_btc: float
    hyperliquid_baseline_cost_usd_per_btc: float
    hyperliquid_baseline_cost_usd_total: float
    cost_saving_vs_hyperliquid_usd_total: float


def load_execution_view(path: Path) -> pd.DataFrame:
    df = pd.read_csv(path)
    missing = REQUIRED_COLUMNS - set(df.columns)
    if missing:
        raise ValueError(f"execution_view missing required columns: {sorted(missing)}")

    df = df.copy()
    df["timestamp"] = pd.to_datetime(df["timestamp"], utc=True)
    df = df.sort_values("timestamp").reset_index(drop=True)

    numeric_columns = [
        "reference_mid",
        "reference_bid",
        "reference_ask",
        "executable_bid",
        "executable_ask",
        "execution_spread",
    ]
    for column in OPTIONAL_SIZE_COLUMNS:
        if column not in df.columns:
            df[column] = float("inf")
        numeric_columns.append(column)

    for column in numeric_columns:
        df[column] = pd.to_numeric(df[column], errors="coerce")

    df[OPTIONAL_SIZE_COLUMNS] = df[OPTIONAL_SIZE_COLUMNS].fillna(0.0)
    df = df.dropna(subset=["timestamp", *REQUIRED_COLUMNS.difference({"timestamp"})]).reset_index(drop=True)
    if df.empty:
        raise ValueError("execution_view has no usable rows after cleaning")
    return df


def _seconds_between(later: pd.Timestamp, earlier: pd.Timestamp) -> float:
    return float((later - earlier).total_seconds())


def _first_index_at_or_after(df: pd.DataFrame, start_idx: int, target_ts: pd.Timestamp) -> int | None:
    for idx in range(start_idx, len(df)):
        if df.at[idx, "timestamp"] >= target_ts:
            return idx
    return None


def _has_min_size(row: pd.Series, side: str, quantity_btc: float) -> bool:
    if side == "buy":
        return float(row["lighter_ask_sz"]) >= quantity_btc
    if side == "sell":
        return float(row["lighter_bid_sz"]) >= quantity_btc
    raise ValueError(f"unsupported side: {side}")


def _is_fill_feasible(row: pd.Series, side: str, threshold_usd: float, quantity_btc: float) -> bool:
    return float(row["execution_spread"]) <= threshold_usd and _has_min_size(row, side, quantity_btc)


def _find_next_fill_feasible_index(
    df: pd.DataFrame,
    start_idx: int,
    threshold_usd: float,
    max_wait_sec: float,
    latency_sec: float,
    quantity_btc: float,
    side: str,
) -> tuple[int, int] | None:
    if start_idx >= len(df):
        return None

    start_ts = df.at[start_idx, "timestamp"]
    for signal_idx in range(start_idx, len(df)):
        signal_ts = df.at[signal_idx, "timestamp"]
        signal_wait_sec = _seconds_between(signal_ts, start_ts)
        if signal_wait_sec > max_wait_sec:
            return None
        if float(df.at[signal_idx, "execution_spread"]) > threshold_usd:
            continue

        fill_target_ts = signal_ts + pd.Timedelta(seconds=latency_sec)
        fill_idx = _first_index_at_or_after(df, signal_idx, fill_target_ts)
        if fill_idx is None:
            return None
        fill_wait_sec = _seconds_between(df.at[fill_idx, "timestamp"], start_ts)
        if fill_wait_sec > max_wait_sec:
            return None
        if _is_fill_feasible(df.iloc[fill_idx], side, threshold_usd, quantity_btc):
            return signal_idx, fill_idx
    return None


def hyperliquid_taker_baseline_usd_per_btc(row: pd.Series, taker_fee_bps: float) -> float:
    reference_spread = float(row["reference_ask"] - row["reference_bid"])
    fee_cost = 2.0 * float(row["reference_mid"]) * taker_fee_bps / 10_000.0
    return reference_spread + fee_cost


def build_low_spread_runs(df: pd.DataFrame, threshold_usd: float) -> list[dict[str, Any]]:
    runs: list[dict[str, Any]] = []
    in_run = False
    start_idx = 0

    for idx, spread in enumerate(df["execution_spread"].astype(float)):
        is_low = spread <= threshold_usd
        if is_low and not in_run:
            in_run = True
            start_idx = idx
        elif not is_low and in_run:
            end_idx = idx - 1
            runs.append(_run_record(df, start_idx, end_idx, threshold_usd))
            in_run = False

    if in_run:
        runs.append(_run_record(df, start_idx, len(df) - 1, threshold_usd))
    return runs


def _run_record(df: pd.DataFrame, start_idx: int, end_idx: int, threshold_usd: float) -> dict[str, Any]:
    start_ts = df.at[start_idx, "timestamp"]
    end_ts = df.at[end_idx, "timestamp"]
    return {
        "threshold_usd": threshold_usd,
        "start_timestamp": start_ts.isoformat(),
        "end_timestamp": end_ts.isoformat(),
        "duration_sec": _seconds_between(end_ts, start_ts),
        "rows": int(end_idx - start_idx + 1),
        "min_spread_usd": float(df.loc[start_idx:end_idx, "execution_spread"].min()),
        "median_spread_usd": float(df.loc[start_idx:end_idx, "execution_spread"].median()),
    }


def build_fill_feasible_runs(
    df: pd.DataFrame,
    threshold_usd: float,
    quantity_btc: float,
    side: str,
) -> list[dict[str, Any]]:
    feasible = df.apply(lambda row: _is_fill_feasible(row, side, threshold_usd, quantity_btc), axis=1)
    runs: list[dict[str, Any]] = []
    in_run = False
    start_idx = 0
    for idx, is_feasible in enumerate(feasible):
        if is_feasible and not in_run:
            in_run = True
            start_idx = idx
        elif not is_feasible and in_run:
            runs.append(_run_record(df, start_idx, idx - 1, threshold_usd))
            in_run = False
    if in_run:
        runs.append(_run_record(df, start_idx, len(df) - 1, threshold_usd))
    return runs


def fill_feasible_ratio(df: pd.DataFrame, threshold_usd: float, quantity_btc: float, side: str) -> float:
    feasible = df.apply(lambda row: _is_fill_feasible(row, side, threshold_usd, quantity_btc), axis=1)
    return float(feasible.mean())


def simulate_round_trips(
    df: pd.DataFrame,
    *,
    threshold_usd: float,
    quantity_btc: float,
    hold_sec: float,
    max_wait_sec: float,
    hyperliquid_taker_fee_bps: float,
    lighter_fee_bps: float = 0.0,
    latency_sec: float = 0.0,
) -> list[PaperRoundTripEvent]:
    events: list[PaperRoundTripEvent] = []
    decision_idx = 0

    while decision_idx < len(df):
        entry = _find_next_fill_feasible_index(
            df,
            decision_idx,
            threshold_usd,
            max_wait_sec,
            latency_sec,
            quantity_btc,
            "buy",
        )
        if entry is None:
            break
        entry_signal_idx, entry_fill_idx = entry

        entry_ts = df.at[entry_fill_idx, "timestamp"]
        exit_target_ts = entry_ts + pd.Timedelta(seconds=hold_sec)
        exit_search_start = _first_index_at_or_after(df, entry_fill_idx + 1, exit_target_ts)
        if exit_search_start is None:
            break

        exit_ = _find_next_fill_feasible_index(
            df,
            exit_search_start,
            threshold_usd,
            max_wait_sec,
            latency_sec,
            quantity_btc,
            "sell",
        )
        if exit_ is None:
            decision_idx = entry_signal_idx + 1
            continue
        exit_signal_idx, exit_fill_idx = exit_

        event = _build_event(
            threshold_usd=threshold_usd,
            decision_row=df.iloc[decision_idx],
            entry_signal_row=df.iloc[entry_signal_idx],
            entry_row=df.iloc[entry_fill_idx],
            exit_signal_row=df.iloc[exit_signal_idx],
            exit_row=df.iloc[exit_fill_idx],
            exit_target_ts=exit_target_ts,
            quantity_btc=quantity_btc,
            hyperliquid_taker_fee_bps=hyperliquid_taker_fee_bps,
            lighter_fee_bps=lighter_fee_bps,
        )
        events.append(event)
        decision_idx = exit_fill_idx + 1

    return events


def _build_event(
    *,
    threshold_usd: float,
    decision_row: pd.Series,
    entry_signal_row: pd.Series,
    entry_row: pd.Series,
    exit_signal_row: pd.Series,
    exit_row: pd.Series,
    exit_target_ts: pd.Timestamp,
    quantity_btc: float,
    hyperliquid_taker_fee_bps: float,
    lighter_fee_bps: float,
) -> PaperRoundTripEvent:
    entry_ref = float(entry_row["reference_mid"])
    exit_ref = float(exit_row["reference_mid"])
    entry_ask = float(entry_row["executable_ask"])
    entry_bid = float(entry_row["executable_bid"])
    exit_ask = float(exit_row["executable_ask"])
    exit_bid = float(exit_row["executable_bid"])

    lighter_buy_cost = entry_ask - entry_ref
    lighter_sell_cost = exit_ref - exit_bid
    lighter_fee_cost = (entry_ask + exit_bid) * lighter_fee_bps / 10_000.0
    lighter_execution_cost = lighter_buy_cost + lighter_sell_cost + lighter_fee_cost
    realized_round_trip_cost = entry_ask - exit_bid + lighter_fee_cost
    inventory_markout = exit_ref - entry_ref
    hyperliquid_baseline = hyperliquid_taker_baseline_usd_per_btc(entry_row, hyperliquid_taker_fee_bps)

    decision_ts = decision_row["timestamp"]
    entry_signal_ts = entry_signal_row["timestamp"]
    entry_ts = entry_row["timestamp"]
    exit_signal_ts = exit_signal_row["timestamp"]
    exit_ts = exit_row["timestamp"]

    return PaperRoundTripEvent(
        threshold_usd=threshold_usd,
        decision_timestamp=decision_ts.isoformat(),
        entry_signal_timestamp=entry_signal_ts.isoformat(),
        entry_timestamp=entry_ts.isoformat(),
        exit_target_timestamp=exit_target_ts.isoformat(),
        exit_signal_timestamp=exit_signal_ts.isoformat(),
        exit_timestamp=exit_ts.isoformat(),
        decision_to_entry_wait_sec=_seconds_between(entry_ts, decision_ts),
        entry_latency_sec=_seconds_between(entry_ts, entry_signal_ts),
        exit_wait_sec=_seconds_between(exit_ts, exit_target_ts),
        exit_latency_sec=_seconds_between(exit_ts, exit_signal_ts),
        total_elapsed_sec=_seconds_between(exit_ts, decision_ts),
        quantity_btc=quantity_btc,
        entry_reference_mid=entry_ref,
        exit_reference_mid=exit_ref,
        entry_lighter_bid=entry_bid,
        entry_lighter_ask=entry_ask,
        exit_lighter_bid=exit_bid,
        exit_lighter_ask=exit_ask,
        entry_lighter_bid_size_btc=float(entry_row["lighter_bid_sz"]),
        entry_lighter_ask_size_btc=float(entry_row["lighter_ask_sz"]),
        exit_lighter_bid_size_btc=float(exit_row["lighter_bid_sz"]),
        exit_lighter_ask_size_btc=float(exit_row["lighter_ask_sz"]),
        entry_lighter_spread=float(entry_row["execution_spread"]),
        exit_lighter_spread=float(exit_row["execution_spread"]),
        lighter_fee_bps=lighter_fee_bps,
        lighter_fee_cost_usd_per_btc=lighter_fee_cost,
        lighter_buy_cost_vs_ref_usd_per_btc=lighter_buy_cost,
        lighter_sell_cost_vs_ref_usd_per_btc=lighter_sell_cost,
        lighter_execution_cost_usd_per_btc=lighter_execution_cost,
        lighter_execution_cost_usd_total=lighter_execution_cost * quantity_btc,
        lighter_realized_round_trip_cost_usd_per_btc=realized_round_trip_cost,
        lighter_realized_round_trip_cost_usd_total=realized_round_trip_cost * quantity_btc,
        inventory_markout_usd_per_btc=inventory_markout,
        hyperliquid_baseline_cost_usd_per_btc=hyperliquid_baseline,
        hyperliquid_baseline_cost_usd_total=hyperliquid_baseline * quantity_btc,
        cost_saving_vs_hyperliquid_usd_total=(hyperliquid_baseline - lighter_execution_cost) * quantity_btc,
    )


def summarize_events(
    df: pd.DataFrame,
    events_by_threshold: dict[float, list[PaperRoundTripEvent]],
    runs_by_threshold: dict[float, list[dict[str, Any]]],
    buy_runs_by_threshold: dict[float, list[dict[str, Any]]],
    sell_runs_by_threshold: dict[float, list[dict[str, Any]]],
    args: argparse.Namespace,
) -> dict[str, Any]:
    by_threshold: dict[str, Any] = {}
    for threshold, events in events_by_threshold.items():
        event_df = pd.DataFrame([asdict(event) for event in events])
        runs = runs_by_threshold[threshold]
        run_df = pd.DataFrame(runs)
        buy_run_df = pd.DataFrame(buy_runs_by_threshold[threshold])
        sell_run_df = pd.DataFrame(sell_runs_by_threshold[threshold])
        by_threshold[str(threshold)] = {
            "round_trips": int(len(events)),
            "low_spread_time_ratio": float((df["execution_spread"].astype(float) <= threshold).mean()),
            "buy_fill_feasible_time_ratio": fill_feasible_ratio(df, threshold, args.quantity_btc, "buy"),
            "sell_fill_feasible_time_ratio": fill_feasible_ratio(df, threshold, args.quantity_btc, "sell"),
            "low_spread_runs": int(len(runs)),
            "buy_fill_feasible_runs": int(len(buy_runs_by_threshold[threshold])),
            "sell_fill_feasible_runs": int(len(sell_runs_by_threshold[threshold])),
            "low_spread_run_duration_sec": _describe_series(run_df.get("duration_sec")),
            "buy_fill_feasible_run_duration_sec": _describe_series(buy_run_df.get("duration_sec")),
            "sell_fill_feasible_run_duration_sec": _describe_series(sell_run_df.get("duration_sec")),
            "decision_to_entry_wait_sec": _describe_series(event_df.get("decision_to_entry_wait_sec")),
            "entry_latency_sec": _describe_series(event_df.get("entry_latency_sec")),
            "exit_wait_sec": _describe_series(event_df.get("exit_wait_sec")),
            "exit_latency_sec": _describe_series(event_df.get("exit_latency_sec")),
            "lighter_fee_cost_usd_per_btc": _describe_series(event_df.get("lighter_fee_cost_usd_per_btc")),
            "lighter_execution_cost_usd_total": _describe_series(event_df.get("lighter_execution_cost_usd_total")),
            "lighter_realized_round_trip_cost_usd_total": _describe_series(
                event_df.get("lighter_realized_round_trip_cost_usd_total"),
            ),
            "hyperliquid_baseline_cost_usd_total": _describe_series(
                event_df.get("hyperliquid_baseline_cost_usd_total"),
            ),
            "cost_saving_vs_hyperliquid_usd_total": _describe_series(
                event_df.get("cost_saving_vs_hyperliquid_usd_total"),
            ),
            "inventory_markout_usd_per_btc": _describe_series(event_df.get("inventory_markout_usd_per_btc")),
        }

    return {
        "created_at_utc": datetime.now(UTC).isoformat(timespec="seconds") + "Z",
        "mode": "paper_replay_only",
        "safety": {
            "reads_local_execution_view_csv_only": True,
            "uses_private_credentials": False,
            "uses_private_websocket": False,
            "places_orders": False,
            "cancels_orders": False,
            "transfers_or_withdraws": False,
        },
        "input": str(args.input),
        "rows": int(len(df)),
        "time_start": df["timestamp"].iloc[0].isoformat(),
        "time_end": df["timestamp"].iloc[-1].isoformat(),
        "quantity_btc": args.quantity_btc,
        "hold_sec": args.hold_sec,
        "max_wait_sec": args.max_wait_sec,
        "latency_sec": args.latency_sec,
        "lighter_fee_bps": args.lighter_fee_bps,
        "hyperliquid_taker_fee_bps": args.hyperliquid_taker_fee_bps,
        "thresholds_usd": args.threshold_usd,
        "by_threshold": by_threshold,
        "notes": [
            "Execution costs are paper proxies, not real fills.",
            "Fill-feasible means spread <= threshold and top-of-book size is at least quantity_btc after latency haircut.",
            "Lighter execution cost is measured against reference_mid at entry/exit and includes configurable Lighter fees.",
            "Realized round-trip cost includes inventory price movement between entry and exit.",
            "Hyperliquid baseline is reference spread plus configurable taker fees on both legs.",
        ],
    }


def _describe_series(series: pd.Series | None) -> dict[str, float | int | None]:
    if series is None:
        return _empty_stats()
    clean = pd.to_numeric(series, errors="coerce").dropna()
    if clean.empty:
        return _empty_stats()
    return {
        "count": int(clean.count()),
        "mean": float(clean.mean()),
        "median": float(clean.median()),
        "p75": float(clean.quantile(0.75)),
        "p90": float(clean.quantile(0.90)),
        "p95": float(clean.quantile(0.95)),
        "min": float(clean.min()),
        "max": float(clean.max()),
    }


def _empty_stats() -> dict[str, float | int | None]:
    return {
        "count": 0,
        "mean": None,
        "median": None,
        "p75": None,
        "p90": None,
        "p95": None,
        "min": None,
        "max": None,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Paper replay Lighter low-spread minimum-size cost experiment")
    parser.add_argument("--input", type=Path, required=True, help="cross_venue execution_view CSV")
    parser.add_argument("--out-root", type=Path, required=True, help="Output directory")
    parser.add_argument("--threshold-usd", type=float, nargs="+", default=[1.0, 2.0, 3.0])
    parser.add_argument("--quantity-btc", type=float, default=0.0001, help="Paper order size in BTC")
    parser.add_argument("--hold-sec", type=float, default=30.0, help="Minimum time between paper buy and sell")
    parser.add_argument("--max-wait-sec", type=float, default=30.0, help="Max wait for entry/exit fill-feasible window")
    parser.add_argument("--latency-sec", type=float, default=0.0, help="Latency haircut before checking fill feasibility")
    parser.add_argument("--lighter-fee-bps", type=float, default=0.0, help="One-way Lighter paper fee bps")
    parser.add_argument("--hyperliquid-taker-fee-bps", type=float, default=2.5, help="One-way Hyperliquid taker fee bps")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    df = load_execution_view(args.input)

    events_by_threshold: dict[float, list[PaperRoundTripEvent]] = {}
    runs_by_threshold: dict[float, list[dict[str, Any]]] = {}
    buy_runs_by_threshold: dict[float, list[dict[str, Any]]] = {}
    sell_runs_by_threshold: dict[float, list[dict[str, Any]]] = {}
    all_events: list[dict[str, Any]] = []
    for threshold in args.threshold_usd:
        events = simulate_round_trips(
            df,
            threshold_usd=threshold,
            quantity_btc=args.quantity_btc,
            hold_sec=args.hold_sec,
            max_wait_sec=args.max_wait_sec,
            hyperliquid_taker_fee_bps=args.hyperliquid_taker_fee_bps,
            lighter_fee_bps=args.lighter_fee_bps,
            latency_sec=args.latency_sec,
        )
        runs = build_low_spread_runs(df, threshold)
        buy_runs = build_fill_feasible_runs(df, threshold, args.quantity_btc, "buy")
        sell_runs = build_fill_feasible_runs(df, threshold, args.quantity_btc, "sell")
        events_by_threshold[threshold] = events
        runs_by_threshold[threshold] = runs
        buy_runs_by_threshold[threshold] = buy_runs
        sell_runs_by_threshold[threshold] = sell_runs
        all_events.extend(asdict(event) for event in events)

    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    args.out_root.mkdir(parents=True, exist_ok=True)
    events_path = args.out_root / f"lighter_min_size_cost_events_{stamp}.csv"
    summary_path = args.out_root / f"lighter_min_size_cost_summary_{stamp}.json"

    pd.DataFrame(all_events).to_csv(events_path, index=False)
    summary = summarize_events(
        df,
        events_by_threshold,
        runs_by_threshold,
        buy_runs_by_threshold,
        sell_runs_by_threshold,
        args,
    )
    summary["outputs"] = {"events_csv": str(events_path), "summary_json": str(summary_path)}
    summary_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")

    print(f"OK EVENTS CSV: {events_path}")
    print(f"OK SUMMARY JSON: {summary_path}")
    print(json.dumps(summary["by_threshold"], ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
