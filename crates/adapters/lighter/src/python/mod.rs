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

pub mod config;
pub mod enums;
pub mod factories;
pub mod urls;

use nautilus_common::factories::{ClientConfig, DataClientFactory, ExecutionClientFactory};
use nautilus_core::python::{to_pyruntime_err, to_pyvalue_err};
use nautilus_system::get_global_pyo3_registry;
use pyo3::prelude::*;

use crate::{
    common::enums::LighterEnvironment,
    config::{LighterDataClientConfig, LighterExecClientConfig},
    factories::{
        LighterDataClientFactory, LighterExecFactoryConfig, LighterExecutionClientFactory,
    },
};

#[expect(clippy::needless_pass_by_value)]
fn extract_lighter_data_factory(
    py: Python<'_>,
    factory: Py<PyAny>,
) -> PyResult<Box<dyn DataClientFactory>> {
    match factory.extract::<LighterDataClientFactory>(py) {
        Ok(f) => Ok(Box::new(f)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract LighterDataClientFactory: {e}"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn extract_lighter_exec_factory(
    py: Python<'_>,
    factory: Py<PyAny>,
) -> PyResult<Box<dyn ExecutionClientFactory>> {
    match factory.extract::<LighterExecutionClientFactory>(py) {
        Ok(f) => Ok(Box::new(f)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract LighterExecutionClientFactory: {e}"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn extract_lighter_data_config(
    py: Python<'_>,
    config: Py<PyAny>,
) -> PyResult<Box<dyn ClientConfig>> {
    match config.extract::<LighterDataClientConfig>(py) {
        Ok(c) => Ok(Box::new(c)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract LighterDataClientConfig: {e}"
        ))),
    }
}

#[expect(clippy::needless_pass_by_value)]
fn extract_lighter_exec_config(
    py: Python<'_>,
    config: Py<PyAny>,
) -> PyResult<Box<dyn ClientConfig>> {
    match config.extract::<LighterExecFactoryConfig>(py) {
        Ok(c) => Ok(Box::new(c)),
        Err(e) => Err(to_pyvalue_err(format!(
            "Failed to extract LighterExecFactoryConfig: {e}"
        ))),
    }
}

/// Loaded as `nautilus_pyo3.lighter`.
///
/// # Errors
///
/// Returns an error if any bindings fail to register with the Python module.
#[pymodule]
pub fn lighter(_: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__package__", "nautilus_trader.core.nautilus_pyo3.lighter")?;

    m.add_class::<LighterEnvironment>()?;
    m.add_class::<LighterDataClientConfig>()?;
    m.add_class::<LighterExecClientConfig>()?;
    m.add_class::<LighterExecFactoryConfig>()?;
    m.add_class::<LighterDataClientFactory>()?;
    m.add_class::<LighterExecutionClientFactory>()?;

    m.add_function(wrap_pyfunction!(urls::py_get_lighter_http_url, m)?)?;
    m.add_function(wrap_pyfunction!(urls::py_get_lighter_ws_url, m)?)?;
    m.add_function(wrap_pyfunction!(urls::py_get_lighter_environment, m)?)?;

    let registry = get_global_pyo3_registry();

    if let Err(e) =
        registry.register_factory_extractor("LIGHTER".to_string(), extract_lighter_data_factory)
    {
        return Err(to_pyruntime_err(format!(
            "Failed to register Lighter data factory extractor: {e}"
        )));
    }

    if let Err(e) = registry
        .register_exec_factory_extractor("LIGHTER".to_string(), extract_lighter_exec_factory)
    {
        return Err(to_pyruntime_err(format!(
            "Failed to register Lighter exec factory extractor: {e}"
        )));
    }

    if let Err(e) = registry.register_config_extractor(
        "LighterDataClientConfig".to_string(),
        extract_lighter_data_config,
    ) {
        return Err(to_pyruntime_err(format!(
            "Failed to register Lighter data config extractor: {e}"
        )));
    }

    if let Err(e) = registry.register_config_extractor(
        "LighterExecFactoryConfig".to_string(),
        extract_lighter_exec_config,
    ) {
        return Err(to_pyruntime_err(format!(
            "Failed to register Lighter exec config extractor: {e}"
        )));
    }

    Ok(())
}
