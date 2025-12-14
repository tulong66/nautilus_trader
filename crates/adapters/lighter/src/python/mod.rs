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

//! Python bindings for the Lighter adapter.
//!
//! This module provides PyO3 bindings for:
//! - `LighterEnvironment` enum for network selection (mainnet/testnet)
//! - URL helper functions for HTTP and WebSocket connections

pub mod enums;
pub mod urls;

use pyo3::prelude::*;

/// Loaded as `nautilus_pyo3.lighter`.
///
/// # Errors
///
/// Returns an error if any bindings fail to register with the Python module.
#[pymodule]
pub fn lighter(_: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__package__", "nautilus_trader.core.nautilus_pyo3.lighter")?;

    // Enums
    m.add_class::<crate::common::enums::LighterEnvironment>()?;

    // URL helper functions
    m.add_function(wrap_pyfunction!(urls::py_get_lighter_http_url, m)?)?;
    m.add_function(wrap_pyfunction!(urls::py_get_lighter_ws_url, m)?)?;
    m.add_function(wrap_pyfunction!(urls::py_get_lighter_environment, m)?)?;

    Ok(())
}
