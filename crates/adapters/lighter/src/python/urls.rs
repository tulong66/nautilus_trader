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

//! Python wrapper functions for Lighter URL helpers.

use pyo3::prelude::*;

use crate::common::{
    LighterEnvironment,
    consts::{
        LIGHTER_MAINNET_HTTP_URL, LIGHTER_MAINNET_WS_URL,
        LIGHTER_TESTNET_HTTP_URL, LIGHTER_TESTNET_WS_URL,
    },
};

/// Get the HTTP base URL for Lighter based on environment.
#[pyfunction]
#[pyo3(name = "get_lighter_http_url")]
#[must_use]
pub fn py_get_lighter_http_url(is_testnet: bool) -> String {
    if is_testnet {
        LIGHTER_TESTNET_HTTP_URL.to_string()
    } else {
        LIGHTER_MAINNET_HTTP_URL.to_string()
    }
}

/// Get the WebSocket URL for Lighter based on environment.
#[pyfunction]
#[pyo3(name = "get_lighter_ws_url")]
#[must_use]
pub fn py_get_lighter_ws_url(is_testnet: bool) -> String {
    if is_testnet {
        LIGHTER_TESTNET_WS_URL.to_string()
    } else {
        LIGHTER_MAINNET_WS_URL.to_string()
    }
}

/// Get the environment enum from a boolean flag.
#[pyfunction]
#[pyo3(name = "get_lighter_environment")]
#[must_use]
pub fn py_get_lighter_environment(is_testnet: bool) -> LighterEnvironment {
    if is_testnet {
        LighterEnvironment::Testnet
    } else {
        LighterEnvironment::Mainnet
    }
}
