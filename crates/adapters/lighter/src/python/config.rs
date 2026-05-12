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

//! Python bindings for Lighter configuration.

use pyo3::prelude::*;

use crate::{
    common::enums::LighterEnvironment,
    config::{LighterDataClientConfig, LighterExecClientConfig},
};

#[pymethods]
#[pyo3_stub_gen::derive::gen_stub_pymethods]
impl LighterDataClientConfig {
    /// Configuration for the Lighter data client.
    #[new]
    #[pyo3(signature = (
        environment = None,
        base_url_http = None,
        base_url_ws = None,
        heartbeat_interval_secs = None,
        http_timeout_secs = None,
        max_retries = None,
    ))]
    fn py_new(
        environment: Option<LighterEnvironment>,
        base_url_http: Option<String>,
        base_url_ws: Option<String>,
        heartbeat_interval_secs: Option<u64>,
        http_timeout_secs: Option<u64>,
        max_retries: Option<u32>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            environment: environment.unwrap_or(defaults.environment),
            base_url_http,
            base_url_ws,
            heartbeat_interval_secs: heartbeat_interval_secs.or(defaults.heartbeat_interval_secs),
            http_timeout_secs: http_timeout_secs.or(defaults.http_timeout_secs),
            max_retries: max_retries.or(defaults.max_retries),
        }
    }

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
#[pyo3_stub_gen::derive::gen_stub_pymethods]
impl LighterExecClientConfig {
    /// Configuration for the Lighter execution client.
    #[new]
    #[pyo3(signature = (
        private_key,
        account_index,
        api_key_index,
        environment = None,
        order_prefix = None,
        gc_interval_secs = None,
        base_url_http = None,
        base_url_ws = None,
        heartbeat_interval_secs = None,
        http_timeout_secs = None,
        max_retries = None,
    ))]
    #[expect(clippy::too_many_arguments)]
    fn py_new(
        private_key: String,
        account_index: u32,
        api_key_index: u8,
        environment: Option<LighterEnvironment>,
        order_prefix: Option<String>,
        gc_interval_secs: Option<u64>,
        base_url_http: Option<String>,
        base_url_ws: Option<String>,
        heartbeat_interval_secs: Option<u64>,
        http_timeout_secs: Option<u64>,
        max_retries: Option<u32>,
    ) -> Self {
        let mut config = Self::new(
            private_key,
            account_index,
            api_key_index,
            environment.unwrap_or_default(),
        );
        config.order_prefix = order_prefix;
        config.gc_interval_secs = gc_interval_secs.or(config.gc_interval_secs);
        config.base_url_http = base_url_http;
        config.base_url_ws = base_url_ws;
        config.heartbeat_interval_secs = heartbeat_interval_secs.or(config.heartbeat_interval_secs);
        config.http_timeout_secs = http_timeout_secs.or(config.http_timeout_secs);
        config.max_retries = max_retries.or(config.max_retries);
        config
    }

    fn __repr__(&self) -> String {
        format!("{self:?}")
    }
}
