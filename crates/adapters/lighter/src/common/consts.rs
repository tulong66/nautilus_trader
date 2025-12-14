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

//! Constants for the Lighter adapter.

use nautilus_model::identifiers::Venue;
use ustr::Ustr;

/// Lighter venue identifier.
pub static LIGHTER_VENUE: std::sync::LazyLock<Venue> =
    std::sync::LazyLock::new(|| Venue::new(Ustr::from("LIGHTER")));

/// Chain ID for Lighter testnet.
pub const CHAIN_ID_TESTNET: u32 = 300;

/// Chain ID for Lighter mainnet.
pub const CHAIN_ID_MAINNET: u32 = 304;

/// Default WebSocket ping interval in seconds.
pub const DEFAULT_WS_PING_INTERVAL_SECS: u64 = 60;

/// Lighter WebSocket ping timeout in seconds.
pub const WS_PING_TIMEOUT_SECS: u64 = 120;

/// Default HTTP request timeout in seconds.
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 30;

/// Default market cache TTL in seconds.
pub const DEFAULT_MARKET_CACHE_TTL_SECS: u64 = 300;

/// Default GC interval for stale orders in seconds.
pub const DEFAULT_GC_INTERVAL_SECS: u64 = 300;

/// Base decimals for quantity (Lighter uses 8 decimals for base amounts).
pub const BASE_DECIMALS: u8 = 8;

/// API key index range: minimum value.
pub const API_KEY_INDEX_MIN: u8 = 2;

/// API key index range: maximum value.
pub const API_KEY_INDEX_MAX: u8 = 254;
