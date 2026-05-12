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

from __future__ import annotations

from nautilus_trader.common.config import PositiveInt
from nautilus_trader.config import LiveDataClientConfig
from nautilus_trader.config import LiveExecClientConfig
from nautilus_trader.core.nautilus_pyo3 import LighterEnvironment


class LighterDataClientConfig(LiveDataClientConfig, frozen=True):
    """
    Configuration for ``LighterDataClient`` instances.
    """

    environment: LighterEnvironment | None = None
    base_url_http: str | None = None
    base_url_ws: str | None = None
    heartbeat_interval_secs: PositiveInt | None = None
    http_timeout_secs: PositiveInt | None = None
    max_retries: PositiveInt | None = None


class LighterExecClientConfig(LiveExecClientConfig, frozen=True):
    """
    Configuration for ``LighterExecutionClient`` instances.
    """

    private_key: str | None = None
    account_index: int | None = None
    api_key_index: int | None = None
    environment: LighterEnvironment | None = None
    order_prefix: str | None = None
    gc_interval_secs: PositiveInt | None = None
    base_url_http: str | None = None
    base_url_ws: str | None = None
    heartbeat_interval_secs: PositiveInt | None = None
    http_timeout_secs: PositiveInt | None = None
    max_retries: PositiveInt | None = None
