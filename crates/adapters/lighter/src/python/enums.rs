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

//! Python bindings for Lighter enums.

use pyo3::prelude::*;

use crate::common::LighterEnvironment;

#[pymethods]
impl LighterEnvironment {
    /// Returns the string representation.
    fn __repr__(&self) -> String {
        format!("{self:?}")
    }

    /// Returns the string value.
    fn __str__(&self) -> String {
        match self {
            Self::Mainnet => "mainnet".to_string(),
            Self::Testnet => "testnet".to_string(),
        }
    }

    /// Returns `true` if this is mainnet.
    #[getter]
    fn is_mainnet(&self) -> bool {
        matches!(self, Self::Mainnet)
    }

    /// Returns `true` if this is testnet.
    #[getter]
    fn is_testnet(&self) -> bool {
        matches!(self, Self::Testnet)
    }
}
