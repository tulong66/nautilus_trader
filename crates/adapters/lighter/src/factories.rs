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

//! Factory functions for creating Lighter clients and components.

use std::{any::Any, cell::RefCell, rc::Rc};

use nautilus_common::{
    cache::Cache,
    clients::{DataClient, ExecutionClient},
    clock::Clock,
    factories::{ClientConfig, DataClientFactory, ExecutionClientFactory},
};
use nautilus_live::ExecutionClientCore;
use nautilus_model::{
    enums::{AccountType, OmsType},
    identifiers::{AccountId, ClientId, TraderId},
};

use crate::{
    common::consts::LIGHTER_VENUE,
    config::{LighterDataClientConfig, LighterExecClientConfig},
    data::LighterDataClient,
    execution::LighterExecutionClient,
};

impl ClientConfig for LighterDataClientConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ClientConfig for LighterExecClientConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Factory for creating Lighter data clients.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.lighter", from_py_object)
)]
#[cfg_attr(
    feature = "python",
    pyo3_stub_gen::derive::gen_stub_pyclass(module = "nautilus_trader.lighter")
)]
pub struct LighterDataClientFactory;

impl LighterDataClientFactory {
    /// Creates a new [`LighterDataClientFactory`] instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LighterDataClientFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl DataClientFactory for LighterDataClientFactory {
    fn create(
        &self,
        name: &str,
        config: &dyn ClientConfig,
        _cache: Rc<RefCell<Cache>>,
        _clock: Rc<RefCell<dyn Clock>>,
    ) -> anyhow::Result<Box<dyn DataClient>> {
        let lighter_config = config
            .as_any()
            .downcast_ref::<LighterDataClientConfig>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid config type for LighterDataClientFactory. Expected LighterDataClientConfig, was {config:?}",
                )
            })?
            .clone();

        let client_id = ClientId::from(name);
        let client = LighterDataClient::new(client_id, lighter_config)?;
        Ok(Box::new(client))
    }

    fn name(&self) -> &'static str {
        "LIGHTER"
    }

    fn config_type(&self) -> &'static str {
        "LighterDataClientConfig"
    }
}

/// Configuration for creating Lighter execution clients via factory.
#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.lighter", from_py_object)
)]
#[cfg_attr(
    feature = "python",
    pyo3_stub_gen::derive::gen_stub_pyclass(module = "nautilus_trader.lighter")
)]
pub struct LighterExecFactoryConfig {
    /// The trader ID for the execution client.
    pub trader_id: TraderId,
    /// The account ID for the execution client.
    pub account_id: AccountId,
    /// The underlying execution client configuration.
    pub config: LighterExecClientConfig,
}

impl ClientConfig for LighterExecFactoryConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Factory for creating Lighter execution clients.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.lighter", from_py_object)
)]
#[cfg_attr(
    feature = "python",
    pyo3_stub_gen::derive::gen_stub_pyclass(module = "nautilus_trader.lighter")
)]
pub struct LighterExecutionClientFactory;

impl LighterExecutionClientFactory {
    /// Creates a new [`LighterExecutionClientFactory`] instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LighterExecutionClientFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionClientFactory for LighterExecutionClientFactory {
    fn create(
        &self,
        name: &str,
        config: &dyn ClientConfig,
        cache: Rc<RefCell<Cache>>,
    ) -> anyhow::Result<Box<dyn ExecutionClient>> {
        let factory_config = config
            .as_any()
            .downcast_ref::<LighterExecFactoryConfig>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid config type for LighterExecutionClientFactory. Expected LighterExecFactoryConfig, was {config:?}",
                )
            })?
            .clone();

        let core = ExecutionClientCore::new(
            factory_config.trader_id,
            ClientId::from(name),
            *LIGHTER_VENUE,
            OmsType::Netting,
            factory_config.account_id,
            AccountType::Margin,
            None,
            cache,
        );

        let client = LighterExecutionClient::new(core, factory_config.config)?;
        Ok(Box::new(client))
    }

    fn name(&self) -> &'static str {
        "LIGHTER"
    }

    fn config_type(&self) -> &'static str {
        "LighterExecFactoryConfig"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lighter_data_client_factory_creation() {
        let factory = LighterDataClientFactory::new();
        assert_eq!(factory.name(), "LIGHTER");
        assert_eq!(factory.config_type(), "LighterDataClientConfig");
    }

    #[test]
    fn test_lighter_data_client_factory_default() {
        let factory = LighterDataClientFactory;
        assert_eq!(factory.name(), "LIGHTER");
    }

    #[test]
    fn test_lighter_execution_client_factory_creation() {
        let factory = LighterExecutionClientFactory::new();
        assert_eq!(factory.name(), "LIGHTER");
        assert_eq!(factory.config_type(), "LighterExecFactoryConfig");
    }

    #[test]
    fn test_lighter_execution_client_factory_default() {
        let factory = LighterExecutionClientFactory;
        assert_eq!(factory.name(), "LIGHTER");
    }
}
