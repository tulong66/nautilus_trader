# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
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

from nautilus_trader.adapters.lighter import LIGHTER
from nautilus_trader.adapters.lighter.config import LighterDataClientConfig
from nautilus_trader.adapters.lighter.constants import LIGHTER_CLIENT_ID
from nautilus_trader.adapters.lighter.constants import LIGHTER_VENUE
from nautilus_trader.adapters.lighter.factories import LighterLiveDataClientFactory
from nautilus_trader.adapters.lighter.factories import LighterLiveExecClientFactory
from nautilus_trader.core import nautilus_pyo3


def test_lighter_pyo3_bindings_are_importable() -> None:
    assert nautilus_pyo3.LighterEnvironment.MAINNET is not None
    assert nautilus_pyo3.get_lighter_http_url(False).startswith("https://")
    assert nautilus_pyo3.get_lighter_ws_url(False).startswith("wss://")
    assert nautilus_pyo3.LighterDataClientFactory().name() == "LIGHTER"
    assert nautilus_pyo3.LighterExecutionClientFactory().name() == "LIGHTER"


def test_lighter_python_package_exports_are_importable() -> None:
    assert LIGHTER == "LIGHTER"
    assert str(LIGHTER_CLIENT_ID) == "LIGHTER"
    assert str(LIGHTER_VENUE) == "LIGHTER"
    assert LighterLiveDataClientFactory().name() == "LIGHTER"
    assert LighterLiveExecClientFactory().name() == "LIGHTER"


def test_lighter_python_data_config_can_be_constructed() -> None:
    config = LighterDataClientConfig()
    assert config.environment is None
