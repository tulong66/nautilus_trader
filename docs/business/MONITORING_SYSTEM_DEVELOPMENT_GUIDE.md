# DEX 价格监控系统开发指南

> **适用场景**: 仅使用 MetaMask 钱包进行 DEX 套利交易
>
> 创建时间: 2025-12-14
> 版本: v1.0

---

## 📋 目录

1. [系统架构设计](#系统架构设计)
2. [环境准备](#环境准备)
3. [第一阶段：基础价格监控](#第一阶段基础价格监控)
4. [第二阶段：套利检测引擎](#第二阶段套利检测引擎)
5. [第三阶段：交易执行模块](#第三阶段交易执行模块)
6. [第四阶段：集成 NautilusTrader](#第四阶段集成-nautilustrader)
7. [测试与部署](#测试与部署)
8. [常见问题](#常见问题)

---

## 🏗️ 系统架构设计

### 整体架构

```
┌─────────────────────────────────────────────────┐
│         监控系统 (Monitoring System)              │
│                                                  │
│  ┌──────────────┐      ┌──────────────┐        │
│  │ 价格抓取层   │──────│ 套利检测层    │        │
│  │ Price Fetch  │      │ Opportunity   │        │
│  │              │      │ Detection     │        │
│  └──────┬───────┘      └──────┬───────┘        │
│         │                     │                  │
│         │    ┌────────────────▼────────┐        │
│         │    │  数据存储层              │        │
│         │    │  Data Storage            │        │
│         │    └────────────────┬────────┘        │
│         │                     │                  │
│  ┌──────▼──────────────────────▼───────┐        │
│  │       执行引擎 (Execution)            │        │
│  │  - MetaMask 集成                      │        │
│  │  - DEX-Router 调用                    │        │
│  └──────────────────────────────────────┘        │
└─────────────────────────────────────────────────┘
         │                              │
         ▼                              ▼
   ┌─────────┐                    ┌─────────┐
   │Uniswap  │                    │ Lighter │
   │Curve    │                    │         │
   │Balancer │                    │         │
   └─────────┘                    └─────────┘
```

### 技术栈选择

**核心语言**: Python 3.12+

**依赖库**:
```python
web3==7.6.0              # 以太坊交互
requests==2.32.3         # HTTP 请求
websocket-client==1.8.0  # WebSocket 连接
pandas==2.2.3            # 数据分析
python-dotenv==1.0.1     # 环境变量管理
redis==5.2.0             # 缓存(可选)
```

---

## 💻 环境准备

### 第 1 步：安装 Python 和依赖

```bash
# 1. 创建项目目录
mkdir dex-arbitrage-monitor
cd dex-arbitrage-monitor

# 2. 创建虚拟环境
python3.12 -m venv venv
source venv/bin/activate  # Linux/macOS
# 或 venv\Scripts\activate  # Windows

# 3. 安装依赖
pip install web3 requests websocket-client pandas python-dotenv redis
```

### 第 2 步：申请 RPC 节点

**推荐使用 Alchemy（免费层）**:

1. 访问 https://www.alchemy.com/
2. 注册账号
3. 创建新应用：
   - 选择 "Ethereum"
   - 选择 "Mainnet"
   - 复制 HTTPS URL（如 `https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY`）

**备用方案**: Infura, QuickNode

### 第 3 步：配置环境变量

创建 `.env` 文件：

```bash
# .env
ALCHEMY_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
INFURA_RPC_URL=https://mainnet.infura.io/v3/YOUR_KEY  # 备用
METAMASK_ADDRESS=0xYourWalletAddress
METAMASK_PRIVATE_KEY=your_private_key_here  # ⚠️ 谨慎保管
```

**⚠️ 安全警告**:
```bash
# 添加到 .gitignore
echo ".env" >> .gitignore
echo "*.key" >> .gitignore
```

### 第 4 步：准备 MetaMask

1. **安装 MetaMask**: https://metamask.io/
2. **获取私钥**:
   - 打开 MetaMask
   - 点击账户 → 账户详情 → 导出私钥
   - 输入密码 → 复制私钥
3. **充值**:
   - 至少 0.05 ETH（用于 Gas 费）
   - 至少 $1,000 USDC（用于套利交易）

---

## 📊 第一阶段：基础价格监控

### 目标

构建一个能够从多个 DEX 获取实时价格的监控器。

### 步骤 1.1：连接以太坊节点

创建 `src/blockchain_client.py`:

```python
# src/blockchain_client.py
from web3 import Web3
from dotenv import load_dotenv
import os

load_dotenv()

class BlockchainClient:
    """以太坊区块链客户端"""

    def __init__(self, rpc_url=None):
        self.rpc_url = rpc_url or os.getenv("ALCHEMY_RPC_URL")
        self.w3 = Web3(Web3.HTTPProvider(self.rpc_url))

        # 验证连接
        if not self.w3.is_connected():
            raise Exception(f"无法连接到以太坊节点: {self.rpc_url}")

        print(f"✅ 已连接到以太坊节点")
        print(f"   当前区块高度: {self.w3.eth.block_number}")

    def get_block_number(self):
        """获取当前区块高度"""
        return self.w3.eth.block_number

    def get_eth_balance(self, address):
        """获取 ETH 余额（单位: ETH）"""
        balance_wei = self.w3.eth.get_balance(address)
        return self.w3.from_wei(balance_wei, 'ether')

    def get_gas_price(self):
        """获取当前 Gas 价格（单位: Gwei）"""
        gas_price_wei = self.w3.eth.gas_price
        return self.w3.from_wei(gas_price_wei, 'gwei')

# 测试代码
if __name__ == "__main__":
    client = BlockchainClient()
    print(f"当前区块: {client.get_block_number()}")
    print(f"Gas 价格: {client.get_gas_price()} Gwei")
```

**测试运行**:
```bash
python src/blockchain_client.py
```

**预期输出**:
```
✅ 已连接到以太坊节点
   当前区块高度: 21234567
当前区块: 21234567
Gas 价格: 12.5 Gwei
```

---

### 步骤 1.2：定义 DEX 配置

创建 `src/dex_config.py`:

```python
# src/dex_config.py
"""DEX 配置文件：合约地址、ABI、交易对"""

# 主要 DEX 配置
DEXES = {
    "uniswap_v3": {
        "name": "Uniswap V3",
        "quoter": "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6",
        "router": "0xE592427A0AEce92De3Edee1F18E0157C05861564",
        "fee_tiers": [500, 3000, 10000],  # 0.05%, 0.3%, 1%
    },
    "curve": {
        "name": "Curve Finance",
        # Curve 的池子地址是动态的，需要查询注册表
        "registry": "0x90E00ACe148ca3b23Ac1bC8C240C2a7Dd9c2d7f5",
    },
    "balancer": {
        "name": "Balancer V2",
        "vault": "0xBA12222222228d8Ba445958a75a0704d566BF2C8",
    },
    "lighter": {
        "name": "Lighter",
        # ⚠️ 需要根据实际情况填写 Lighter 的合约地址
        "router": "0x...",  # 待补充
        "factory": "0x...",  # 待补充
    }
}

# 主要代币地址
TOKENS = {
    "WETH": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    "USDC": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    "USDT": "0xdAC17F958D2ee523a2206206994597C13D831ec7",
    "DAI": "0x6B175474E89094C44Da98b954EedeAC495271d0F",
    "WBTC": "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
}

# 监控的交易对
TRADING_PAIRS = [
    ("USDC", "USDT"),  # 稳定币套利
    ("DAI", "USDC"),   # 稳定币套利
    ("WETH", "USDC"),  # 主流币对
    ("WBTC", "WETH"),  # 主流币对
]

# Uniswap V3 Quoter ABI（简化版，仅包含 quoteExactInputSingle）
UNISWAP_V3_QUOTER_ABI = [
    {
        "inputs": [
            {"internalType": "address", "name": "tokenIn", "type": "address"},
            {"internalType": "address", "name": "tokenOut", "type": "address"},
            {"internalType": "uint24", "name": "fee", "type": "uint24"},
            {"internalType": "uint256", "name": "amountIn", "type": "uint256"},
            {"internalType": "uint160", "name": "sqrtPriceLimitX96", "type": "uint160"}
        ],
        "name": "quoteExactInputSingle",
        "outputs": [
            {"internalType": "uint256", "name": "amountOut", "type": "uint256"}
        ],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]

# ERC20 ABI（简化版，仅包含 balanceOf 和 decimals）
ERC20_ABI = [
    {
        "constant": True,
        "inputs": [{"name": "_owner", "type": "address"}],
        "name": "balanceOf",
        "outputs": [{"name": "balance", "type": "uint256"}],
        "type": "function"
    },
    {
        "constant": True,
        "inputs": [],
        "name": "decimals",
        "outputs": [{"name": "", "type": "uint8"}],
        "type": "function"
    }
]
```

---

### 步骤 1.3：实现 Uniswap V3 价格抓取

创建 `src/price_fetcher.py`:

```python
# src/price_fetcher.py
from web3 import Web3
from decimal import Decimal
from typing import Optional, Tuple
import time

from blockchain_client import BlockchainClient
from dex_config import DEXES, TOKENS, UNISWAP_V3_QUOTER_ABI, ERC20_ABI

class UniswapV3PriceFetcher:
    """Uniswap V3 价格抓取器"""

    def __init__(self, blockchain_client: BlockchainClient):
        self.client = blockchain_client
        self.w3 = blockchain_client.w3

        # 初始化 Quoter 合约
        quoter_address = DEXES["uniswap_v3"]["quoter"]
        self.quoter = self.w3.eth.contract(
            address=quoter_address,
            abi=UNISWAP_V3_QUOTER_ABI
        )

        print(f"✅ Uniswap V3 Quoter 已初始化: {quoter_address}")

    def get_token_decimals(self, token_symbol: str) -> int:
        """获取代币精度"""
        token_address = TOKENS[token_symbol]
        token_contract = self.w3.eth.contract(
            address=token_address,
            abi=ERC20_ABI
        )
        return token_contract.functions.decimals().call()

    def get_price(
        self,
        token_in_symbol: str,
        token_out_symbol: str,
        amount: float = 1.0,
        fee_tier: int = 3000
    ) -> Optional[Tuple[Decimal, Decimal]]:
        """
        获取价格

        参数:
            token_in_symbol: 输入代币符号（如 "WETH"）
            token_out_symbol: 输出代币符号（如 "USDC"）
            amount: 输入数量（默认 1.0）
            fee_tier: 手续费等级（500/3000/10000）

        返回:
            (amount_out, price): 输出数量和价格
        """
        try:
            # 获取代币地址
            token_in = TOKENS[token_in_symbol]
            token_out = TOKENS[token_out_symbol]

            # 获取精度
            decimals_in = self.get_token_decimals(token_in_symbol)
            decimals_out = self.get_token_decimals(token_out_symbol)

            # 转换输入数量为 wei
            amount_in_wei = int(amount * 10 ** decimals_in)

            # 调用 Quoter 合约
            amount_out_wei = self.quoter.functions.quoteExactInputSingle(
                token_in,
                token_out,
                fee_tier,
                amount_in_wei,
                0  # sqrtPriceLimitX96 = 0 表示无限制
            ).call()

            # 转换为人类可读格式
            amount_out = Decimal(amount_out_wei) / Decimal(10 ** decimals_out)
            price = amount_out / Decimal(amount)

            return amount_out, price

        except Exception as e:
            print(f"❌ 获取价格失败 ({token_in_symbol}/{token_out_symbol}): {e}")
            return None, None

# 测试代码
if __name__ == "__main__":
    # 初始化
    blockchain = BlockchainClient()
    fetcher = UniswapV3PriceFetcher(blockchain)

    # 测试：查询 1 WETH 能换多少 USDC
    print("\n🔍 查询价格...")
    amount_out, price = fetcher.get_price("WETH", "USDC", amount=1.0)

    if price:
        print(f"✅ Uniswap V3 (0.3% fee):")
        print(f"   1 WETH = {amount_out:.2f} USDC")
        print(f"   价格: ${price:.2f}")

    # 测试稳定币
    print("\n🔍 查询稳定币价格...")
    amount_out, price = fetcher.get_price("USDC", "USDT", amount=1000.0)

    if price:
        print(f"✅ USDC/USDT:")
        print(f"   1000 USDC = {amount_out:.2f} USDT")
        print(f"   价格: {price:.6f}")
```

**测试运行**:
```bash
python src/price_fetcher.py
```

**预期输出**:
```
✅ 已连接到以太坊节点
   当前区块高度: 21234567
✅ Uniswap V3 Quoter 已初始化: 0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6

🔍 查询价格...
✅ Uniswap V3 (0.3% fee):
   1 WETH = 3250.45 USDC
   价格: $3250.45

🔍 查询稳定币价格...
✅ USDC/USDT:
   1000 USDC = 999.85 USDT
   价格: 0.999850
```

---

### 步骤 1.4：多 DEX 价格聚合

创建 `src/multi_dex_monitor.py`:

```python
# src/multi_dex_monitor.py
import time
from decimal import Decimal
from typing import Dict, List
from dataclasses import dataclass
from datetime import datetime

from blockchain_client import BlockchainClient
from price_fetcher import UniswapV3PriceFetcher
from dex_config import TRADING_PAIRS

@dataclass
class PriceQuote:
    """价格报价"""
    dex: str
    pair: str
    price: Decimal
    amount_out: Decimal
    timestamp: datetime
    fee_tier: int = None

class MultiDexMonitor:
    """多 DEX 价格监控器"""

    def __init__(self, blockchain_client: BlockchainClient):
        self.client = blockchain_client
        self.uniswap_fetcher = UniswapV3PriceFetcher(blockchain_client)

        # 价格缓存
        self.price_cache: Dict[str, List[PriceQuote]] = {}

    def fetch_all_prices(self) -> Dict[str, List[PriceQuote]]:
        """获取所有交易对在所有 DEX 上的价格"""
        results = {}

        for token_in, token_out in TRADING_PAIRS:
            pair_key = f"{token_in}/{token_out}"
            results[pair_key] = []

            print(f"\n📊 查询 {pair_key}...")

            # 1. Uniswap V3（多个手续费等级）
            for fee in [500, 3000, 10000]:
                amount_out, price = self.uniswap_fetcher.get_price(
                    token_in, token_out, amount=1.0, fee_tier=fee
                )

                if price:
                    quote = PriceQuote(
                        dex=f"Uniswap V3 ({fee/10000}%)",
                        pair=pair_key,
                        price=price,
                        amount_out=amount_out,
                        timestamp=datetime.now(),
                        fee_tier=fee
                    )
                    results[pair_key].append(quote)
                    print(f"   ✅ {quote.dex}: {price:.6f}")

                time.sleep(0.5)  # 避免请求过快

            # 2. Curve（如果是稳定币对）
            # TODO: 实现 Curve 价格抓取

            # 3. Balancer
            # TODO: 实现 Balancer 价格抓取

            # 4. Lighter
            # TODO: 实现 Lighter 价格抓取

        self.price_cache = results
        return results

    def find_best_prices(self, pair: str) -> tuple:
        """找出最佳买入和卖出价格"""
        if pair not in self.price_cache:
            return None, None

        quotes = self.price_cache[pair]
        if not quotes:
            return None, None

        # 找最低价（买入）和最高价（卖出）
        best_buy = min(quotes, key=lambda q: q.price)
        best_sell = max(quotes, key=lambda q: q.price)

        return best_buy, best_sell

    def display_summary(self):
        """显示价格摘要"""
        print("\n" + "="*60)
        print("📊 价格监控摘要")
        print("="*60)

        for pair, quotes in self.price_cache.items():
            if not quotes:
                continue

            best_buy, best_sell = self.find_best_prices(pair)

            print(f"\n💱 {pair}")
            print(f"   最佳买入: {best_buy.dex} @ {best_buy.price:.6f}")
            print(f"   最佳卖出: {best_sell.dex} @ {best_sell.price:.6f}")

            if best_buy != best_sell:
                spread = best_sell.price - best_buy.price
                spread_pct = (spread / best_buy.price) * 100
                print(f"   💰 价差: {spread:.6f} ({spread_pct:.3f}%)")

# 测试代码
if __name__ == "__main__":
    print("🚀 启动多 DEX 价格监控...")

    blockchain = BlockchainClient()
    monitor = MultiDexMonitor(blockchain)

    # 获取价格
    monitor.fetch_all_prices()

    # 显示摘要
    monitor.display_summary()
```

**测试运行**:
```bash
python src/multi_dex_monitor.py
```

**预期输出**:
```
🚀 启动多 DEX 价格监控...
✅ 已连接到以太坊节点
   当前区块高度: 21234567
✅ Uniswap V3 Quoter 已初始化: 0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6

📊 查询 USDC/USDT...
   ✅ Uniswap V3 (0.05%): 0.999920
   ✅ Uniswap V3 (0.3%): 0.999850
   ✅ Uniswap V3 (1%): 0.999650

📊 查询 DAI/USDC...
   ✅ Uniswap V3 (0.05%): 1.000100
   ✅ Uniswap V3 (0.3%): 1.000050
   ✅ Uniswap V3 (1%): 0.999950

============================================================
📊 价格监控摘要
============================================================

💱 USDC/USDT
   最佳买入: Uniswap V3 (1%) @ 0.999650
   最佳卖出: Uniswap V3 (0.05%) @ 0.999920
   💰 价差: 0.000270 (0.027%)

💱 DAI/USDC
   最佳买入: Uniswap V3 (1%) @ 0.999950
   最佳卖出: Uniswap V3 (0.05%) @ 1.000100
   💰 价差: 0.000150 (0.015%)
```

---

## 🎯 第二阶段：套利检测引擎

### 目标

基于价格数据,自动检测盈利机会并计算预期收益。

### 步骤 2.1：套利机会检测

创建 `src/arbitrage_detector.py`:

```python
# src/arbitrage_detector.py
from decimal import Decimal
from dataclasses import dataclass
from typing import Optional
from datetime import datetime

from multi_dex_monitor import PriceQuote

@dataclass
class ArbitrageOpportunity:
    """套利机会"""
    pair: str
    buy_dex: str
    sell_dex: str
    buy_price: Decimal
    sell_price: Decimal
    spread: Decimal
    spread_percentage: Decimal
    estimated_gas_cost: Decimal
    estimated_profit: Decimal
    timestamp: datetime

    def __str__(self):
        return (
            f"💰 套利机会: {self.pair}\n"
            f"   买入: {self.buy_dex} @ {self.buy_price:.6f}\n"
            f"   卖出: {self.sell_dex} @ {self.sell_price:.6f}\n"
            f"   价差: {self.spread:.6f} ({self.spread_percentage:.3f}%)\n"
            f"   预估 Gas: ${self.estimated_gas_cost:.2f}\n"
            f"   预估利润: ${self.estimated_profit:.2f}"
        )

class ArbitrageDetector:
    """套利检测引擎"""

    def __init__(
        self,
        min_spread_percentage: Decimal = Decimal("0.2"),  # 最小价差 0.2%
        min_profit_usd: Decimal = Decimal("1.0"),         # 最小利润 $1
        trade_size_usd: Decimal = Decimal("1000.0"),      # 交易规模 $1000
    ):
        self.min_spread_percentage = min_spread_percentage
        self.min_profit_usd = min_profit_usd
        self.trade_size_usd = trade_size_usd

    def estimate_gas_cost(self, num_swaps: int = 2) -> Decimal:
        """
        估算 Gas 成本

        参数:
            num_swaps: 交换次数（跨 DEX 套利通常需要 2 次）

        返回:
            Gas 成本（USD）
        """
        # 假设当前 Gas 价格 12 Gwei
        gas_price_gwei = Decimal("12")

        # 每次 swap 消耗约 150,000 gas
        gas_per_swap = Decimal("150000")

        # ETH 价格假设 $3250
        eth_price_usd = Decimal("3250")

        # 计算总成本
        total_gas = gas_price_gwei * gas_per_swap * num_swaps
        gas_cost_eth = total_gas / Decimal("1e9")  # Gwei → ETH
        gas_cost_usd = gas_cost_eth * eth_price_usd

        return gas_cost_usd

    def detect(
        self,
        best_buy: PriceQuote,
        best_sell: PriceQuote
    ) -> Optional[ArbitrageOpportunity]:
        """
        检测套利机会

        参数:
            best_buy: 最佳买入报价
            best_sell: 最佳卖出报价

        返回:
            ArbitrageOpportunity 或 None
        """
        # 计算价差
        spread = best_sell.price - best_buy.price
        spread_pct = (spread / best_buy.price) * 100

        # 检查是否满足最小价差要求
        if spread_pct < self.min_spread_percentage:
            return None

        # 估算 Gas 成本
        gas_cost = self.estimate_gas_cost(num_swaps=2)

        # 计算预估利润
        # 利润 = 交易规模 × 价差% - Gas 成本
        profit = (self.trade_size_usd * spread_pct / 100) - gas_cost

        # 检查是否满足最小利润要求
        if profit < self.min_profit_usd:
            return None

        # 创建套利机会对象
        opportunity = ArbitrageOpportunity(
            pair=best_buy.pair,
            buy_dex=best_buy.dex,
            sell_dex=best_sell.dex,
            buy_price=best_buy.price,
            sell_price=best_sell.price,
            spread=spread,
            spread_percentage=spread_pct,
            estimated_gas_cost=gas_cost,
            estimated_profit=profit,
            timestamp=datetime.now()
        )

        return opportunity

# 测试代码
if __name__ == "__main__":
    from blockchain_client import BlockchainClient
    from multi_dex_monitor import MultiDexMonitor

    print("🚀 启动套利检测引擎...")

    blockchain = BlockchainClient()
    monitor = MultiDexMonitor(blockchain)
    detector = ArbitrageDetector(
        min_spread_percentage=Decimal("0.1"),  # 降低门槛用于测试
        min_profit_usd=Decimal("0.5")
    )

    # 获取价格
    monitor.fetch_all_prices()

    # 检测套利机会
    print("\n" + "="*60)
    print("🎯 套利机会检测")
    print("="*60)

    opportunities = []
    for pair in monitor.price_cache:
        best_buy, best_sell = monitor.find_best_prices(pair)

        if best_buy and best_sell:
            opp = detector.detect(best_buy, best_sell)

            if opp:
                opportunities.append(opp)
                print(f"\n{opp}")

    if not opportunities:
        print("\n❌ 未发现符合条件的套利机会")
    else:
        print(f"\n✅ 发现 {len(opportunities)} 个套利机会!")
```

**测试运行**:
```bash
python src/arbitrage_detector.py
```

---

### 步骤 2.2：实时监控循环

创建 `src/live_monitor.py`:

```python
# src/live_monitor.py
import time
from decimal import Decimal
from datetime import datetime

from blockchain_client import BlockchainClient
from multi_dex_monitor import MultiDexMonitor
from arbitrage_detector import ArbitrageDetector

class LiveMonitor:
    """实时监控系统"""

    def __init__(
        self,
        interval_seconds: int = 60,
        min_spread: Decimal = Decimal("0.2"),
        min_profit: Decimal = Decimal("1.0")
    ):
        self.interval = interval_seconds

        # 初始化组件
        self.blockchain = BlockchainClient()
        self.monitor = MultiDexMonitor(self.blockchain)
        self.detector = ArbitrageDetector(
            min_spread_percentage=min_spread,
            min_profit_usd=min_profit
        )

        # 统计数据
        self.total_checks = 0
        self.total_opportunities = 0

    def run_single_check(self):
        """执行单次检测"""
        self.total_checks += 1

        print(f"\n{'='*60}")
        print(f"🔍 检测 #{self.total_checks} - {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        print(f"{'='*60}")

        # 1. 获取价格
        self.monitor.fetch_all_prices()

        # 2. 检测套利机会
        opportunities = []
        for pair in self.monitor.price_cache:
            best_buy, best_sell = self.monitor.find_best_prices(pair)

            if best_buy and best_sell:
                opp = self.detector.detect(best_buy, best_sell)

                if opp:
                    opportunities.append(opp)
                    print(f"\n{opp}")

        # 3. 统计
        if opportunities:
            self.total_opportunities += len(opportunities)
            print(f"\n✅ 本次发现 {len(opportunities)} 个机会")
        else:
            print(f"\n❌ 本次未发现机会")

        print(f"\n📊 累计统计:")
        print(f"   总检测次数: {self.total_checks}")
        print(f"   总机会数: {self.total_opportunities}")
        print(f"   成功率: {self.total_opportunities / self.total_checks * 100:.2f}%")

    def run(self):
        """持续运行监控"""
        print("🚀 启动实时监控系统...")
        print(f"⏱️  检测间隔: {self.interval} 秒")
        print(f"📊 最小价差: {self.detector.min_spread_percentage}%")
        print(f"💰 最小利润: ${self.detector.min_profit_usd}")
        print("\n按 Ctrl+C 停止...\n")

        try:
            while True:
                self.run_single_check()

                # 等待下次检测
                print(f"\n⏳ 等待 {self.interval} 秒...")
                time.sleep(self.interval)

        except KeyboardInterrupt:
            print("\n\n⏹️  监控已停止")
            print(f"📊 最终统计:")
            print(f"   总检测次数: {self.total_checks}")
            print(f"   总机会数: {self.total_opportunities}")
            print(f"   成功率: {self.total_opportunities / self.total_checks * 100:.2f}%")

# 启动监控
if __name__ == "__main__":
    monitor = LiveMonitor(
        interval_seconds=60,           # 每 60 秒检测一次
        min_spread=Decimal("0.15"),    # 最小价差 0.15%
        min_profit=Decimal("0.5")      # 最小利润 $0.5
    )

    monitor.run()
```

**启动监控**:
```bash
python src/live_monitor.py
```

**预期输出**:
```
🚀 启动实时监控系统...
⏱️  检测间隔: 60 秒
📊 最小价差: 0.15%
💰 最小利润: $0.5

按 Ctrl+C 停止...

============================================================
🔍 检测 #1 - 2025-12-14 15:30:00
============================================================

📊 查询 USDC/USDT...
   ✅ Uniswap V3 (0.05%): 0.999920
   ✅ Uniswap V3 (0.3%): 0.999850
   ✅ Uniswap V3 (1%): 0.999650

💰 套利机会: USDC/USDT
   买入: Uniswap V3 (1%) @ 0.999650
   卖出: Uniswap V3 (0.05%) @ 0.999920
   价差: 0.000270 (0.027%)
   预估 Gas: $5.85
   预估利润: $-5.58

❌ 本次未发现机会

📊 累计统计:
   总检测次数: 1
   总机会数: 0
   成功率: 0.00%

⏳ 等待 60 秒...
```

---

## ⚡ 第三阶段：交易执行模块

### 目标

集成 MetaMask,实现自动化交易执行。

### 步骤 3.1：MetaMask 钱包集成

创建 `src/wallet.py`:

```python
# src/wallet.py
from web3 import Web3
from eth_account import Account
from decimal import Decimal
import os
from dotenv import load_dotenv

from blockchain_client import BlockchainClient
from dex_config import TOKENS, ERC20_ABI

load_dotenv()

class MetaMaskWallet:
    """MetaMask 钱包集成"""

    def __init__(self, blockchain_client: BlockchainClient):
        self.client = blockchain_client
        self.w3 = blockchain_client.w3

        # 加载私钥
        private_key = os.getenv("METAMASK_PRIVATE_KEY")
        if not private_key:
            raise Exception("请在 .env 文件中设置 METAMASK_PRIVATE_KEY")

        # 创建账户对象
        self.account = Account.from_key(private_key)
        self.address = self.account.address

        print(f"✅ 钱包已加载: {self.address}")

    def get_eth_balance(self) -> Decimal:
        """获取 ETH 余额"""
        balance_wei = self.w3.eth.get_balance(self.address)
        return Decimal(self.w3.from_wei(balance_wei, 'ether'))

    def get_token_balance(self, token_symbol: str) -> Decimal:
        """获取代币余额"""
        token_address = TOKENS[token_symbol]
        token_contract = self.w3.eth.contract(
            address=token_address,
            abi=ERC20_ABI
        )

        # 获取余额
        balance_wei = token_contract.functions.balanceOf(self.address).call()

        # 获取精度
        decimals = token_contract.functions.decimals().call()

        return Decimal(balance_wei) / Decimal(10 ** decimals)

    def display_balances(self):
        """显示所有余额"""
        print(f"\n💼 钱包余额 ({self.address}):")
        print(f"   ETH: {self.get_eth_balance():.6f}")

        for token_symbol in ["USDC", "USDT", "DAI", "WETH", "WBTC"]:
            try:
                balance = self.get_token_balance(token_symbol)
                if balance > 0:
                    print(f"   {token_symbol}: {balance:.6f}")
            except Exception as e:
                print(f"   {token_symbol}: 查询失败 ({e})")

# 测试代码
if __name__ == "__main__":
    blockchain = BlockchainClient()
    wallet = MetaMaskWallet(blockchain)
    wallet.display_balances()
```

**测试运行**:
```bash
python src/wallet.py
```

---

### 步骤 3.2：Uniswap 交易执行

创建 `src/executor.py`:

```python
# src/executor.py
from web3 import Web3
from decimal import Decimal
from typing import Optional
import time

from wallet import MetaMaskWallet
from dex_config import DEXES, TOKENS, ERC20_ABI

# Uniswap V3 Router ABI（简化版）
UNISWAP_V3_ROUTER_ABI = [
    {
        "inputs": [
            {
                "components": [
                    {"internalType": "address", "name": "tokenIn", "type": "address"},
                    {"internalType": "address", "name": "tokenOut", "type": "address"},
                    {"internalType": "uint24", "name": "fee", "type": "uint24"},
                    {"internalType": "address", "name": "recipient", "type": "address"},
                    {"internalType": "uint256", "name": "deadline", "type": "uint256"},
                    {"internalType": "uint256", "name": "amountIn", "type": "uint256"},
                    {"internalType": "uint256", "name": "amountOutMinimum", "type": "uint256"},
                    {"internalType": "uint160", "name": "sqrtPriceLimitX96", "type": "uint160"}
                ],
                "internalType": "struct ISwapRouter.ExactInputSingleParams",
                "name": "params",
                "type": "tuple"
            }
        ],
        "name": "exactInputSingle",
        "outputs": [
            {"internalType": "uint256", "name": "amountOut", "type": "uint256"}
        ],
        "stateMutability": "payable",
        "type": "function"
    }
]

class UniswapV3Executor:
    """Uniswap V3 交易执行器"""

    def __init__(self, wallet: MetaMaskWallet):
        self.wallet = wallet
        self.w3 = wallet.w3

        # 初始化 Router 合约
        router_address = DEXES["uniswap_v3"]["router"]
        self.router = self.w3.eth.contract(
            address=router_address,
            abi=UNISWAP_V3_ROUTER_ABI
        )

        print(f"✅ Uniswap V3 Router 已初始化: {router_address}")

    def approve_token(
        self,
        token_symbol: str,
        amount: Decimal,
        spender_address: str = None
    ) -> str:
        """
        授权代币给 Router 合约

        参数:
            token_symbol: 代币符号
            amount: 授权数量
            spender_address: 授权地址（默认为 Router）

        返回:
            交易哈希
        """
        if spender_address is None:
            spender_address = self.router.address

        token_address = TOKENS[token_symbol]
        token_contract = self.w3.eth.contract(
            address=token_address,
            abi=ERC20_ABI
        )

        # 获取精度
        decimals = token_contract.functions.decimals().call()
        amount_wei = int(amount * Decimal(10 ** decimals))

        # 构建交易
        approve_tx = token_contract.functions.approve(
            spender_address,
            amount_wei
        ).build_transaction({
            'from': self.wallet.address,
            'gas': 100000,
            'gasPrice': self.w3.eth.gas_price,
            'nonce': self.w3.eth.get_transaction_count(self.wallet.address),
        })

        # 签名并发送
        signed_tx = self.w3.eth.account.sign_transaction(
            approve_tx,
            self.wallet.account.key
        )
        tx_hash = self.w3.eth.send_raw_transaction(signed_tx.raw_transaction)

        print(f"✅ 授权交易已发送: {tx_hash.hex()}")

        # 等待确认
        receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash)
        print(f"✅ 授权已确认 (区块 {receipt['blockNumber']})")

        return tx_hash.hex()

    def swap(
        self,
        token_in_symbol: str,
        token_out_symbol: str,
        amount_in: Decimal,
        slippage_tolerance: Decimal = Decimal("0.5"),  # 0.5%
        fee_tier: int = 3000
    ) -> Optional[str]:
        """
        执行交换

        参数:
            token_in_symbol: 输入代币
            token_out_symbol: 输出代币
            amount_in: 输入数量
            slippage_tolerance: 滑点容忍度（百分比）
            fee_tier: 手续费等级

        返回:
            交易哈希或 None
        """
        try:
            # 1. 授权（如果需要）
            print(f"\n🔒 授权 {amount_in} {token_in_symbol}...")
            self.approve_token(token_in_symbol, amount_in)

            # 2. 准备交换参数
            token_in = TOKENS[token_in_symbol]
            token_out = TOKENS[token_out_symbol]

            # 获取精度
            token_in_contract = self.w3.eth.contract(address=token_in, abi=ERC20_ABI)
            decimals_in = token_in_contract.functions.decimals().call()

            amount_in_wei = int(amount_in * Decimal(10 ** decimals_in))

            # 计算最小输出（考虑滑点）
            # 这里简化处理，实际应先调用 Quoter 获取预期输出
            amount_out_minimum = 0  # ⚠️ 生产环境必须设置合理值

            # 设置交易截止时间（当前时间 + 20 分钟）
            deadline = int(time.time()) + 1200

            # 构建交换参数
            swap_params = {
                'tokenIn': token_in,
                'tokenOut': token_out,
                'fee': fee_tier,
                'recipient': self.wallet.address,
                'deadline': deadline,
                'amountIn': amount_in_wei,
                'amountOutMinimum': amount_out_minimum,
                'sqrtPriceLimitX96': 0
            }

            # 3. 构建交易
            swap_tx = self.router.functions.exactInputSingle(
                swap_params
            ).build_transaction({
                'from': self.wallet.address,
                'gas': 300000,
                'gasPrice': self.w3.eth.gas_price,
                'nonce': self.w3.eth.get_transaction_count(self.wallet.address),
                'value': 0
            })

            # 4. 签名并发送
            print(f"\n🔄 执行交换: {amount_in} {token_in_symbol} → {token_out_symbol}")
            signed_tx = self.w3.eth.account.sign_transaction(
                swap_tx,
                self.wallet.account.key
            )
            tx_hash = self.w3.eth.send_raw_transaction(signed_tx.raw_transaction)

            print(f"✅ 交换交易已发送: {tx_hash.hex()}")

            # 5. 等待确认
            receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash)

            if receipt['status'] == 1:
                print(f"✅ 交换成功! (区块 {receipt['blockNumber']})")
                return tx_hash.hex()
            else:
                print(f"❌ 交换失败!")
                return None

        except Exception as e:
            print(f"❌ 交换失败: {e}")
            return None

# 测试代码
if __name__ == "__main__":
    from blockchain_client import BlockchainClient

    print("⚠️  警告: 这将执行真实交易!")
    print("⚠️  请确保:")
    print("   1. 你的钱包有足够的 ETH 和代币")
    print("   2. 你了解交易风险")
    print("   3. 建议先在测试网测试")
    print("\n是否继续? (yes/no): ", end='')

    if input().lower() != 'yes':
        print("已取消")
        exit()

    blockchain = BlockchainClient()
    wallet = MetaMaskWallet(blockchain)
    executor = UniswapV3Executor(wallet)

    # 显示余额
    wallet.display_balances()

    # 执行小额交换（测试）
    # ⚠️ 请根据实际情况修改数量
    executor.swap(
        token_in_symbol="USDC",
        token_out_symbol="USDT",
        amount_in=Decimal("10"),  # 仅 $10 用于测试
        fee_tier=3000
    )

    # 显示交易后余额
    print("\n交易后余额:")
    wallet.display_balances()
```

---

## 🔗 第四阶段：集成 NautilusTrader

### 步骤 4.1：创建自定义适配器

创建 `src/nautilus_adapter.py`:

```python
# src/nautilus_adapter.py
"""
NautilusTrader 自定义 DEX 适配器

这个适配器将我们的价格监控系统集成到 NautilusTrader 框架中
"""

from decimal import Decimal
from nautilus_trader.adapters.base import DataClientAdapter
from nautilus_trader.model.data import QuoteTick
from nautilus_trader.model.identifiers import InstrumentId, Venue
from nautilus_trader.model.objects import Price, Quantity
from nautilus_trader.common.component import MessageBus

from multi_dex_monitor import MultiDexMonitor
from blockchain_client import BlockchainClient

class DEXDataAdapter(DataClientAdapter):
    """DEX 数据适配器"""

    def __init__(
        self,
        msgbus: MessageBus,
        venue: Venue,
        blockchain_client: BlockchainClient = None
    ):
        super().__init__(
            client_id=f"DEX-{venue}",
            msgbus=msgbus,
            venue=venue
        )

        # 初始化监控器
        if blockchain_client is None:
            blockchain_client = BlockchainClient()

        self.monitor = MultiDexMonitor(blockchain_client)

    async def _connect(self):
        """连接到数据源"""
        self._log.info("Connecting to DEX data source...")
        # 这里可以初始化 WebSocket 连接等
        self._log.info("Connected to DEX")

    async def _disconnect(self):
        """断开连接"""
        self._log.info("Disconnecting from DEX...")

    async def subscribe_quote_ticks(self, instrument_id: InstrumentId):
        """订阅报价数据"""
        self._log.info(f"Subscribing to quote ticks for {instrument_id}")

        # 启动价格监控循环
        # 这里应该在后台线程中运行
        pass

    def _handle_price_update(self, pair: str, buy_quote, sell_quote):
        """处理价格更新"""
        # 将价格数据转换为 NautilusTrader 的 QuoteTick
        instrument_id = InstrumentId.from_str(f"{pair}.{self._venue}")

        quote_tick = QuoteTick(
            instrument_id=instrument_id,
            bid_price=Price(buy_quote.price, precision=6),
            ask_price=Price(sell_quote.price, precision=6),
            bid_size=Quantity(1000, precision=2),
            ask_size=Quantity(1000, precision=2),
            ts_event=buy_quote.timestamp,
            ts_init=buy_quote.timestamp
        )

        # 发送到消息总线
        self._msgbus.send(endpoint="DataEngine.process", msg=quote_tick)

# 集成示例
if __name__ == "__main__":
    print("ℹ️  NautilusTrader 集成示例")
    print("   完整集成需要在 NautilusTrader 环境中运行")
    print("   这里仅展示适配器结构")
```

---

## 🧪 测试与部署

### 测试清单

```bash
# 1. 单元测试
pytest tests/

# 2. 集成测试
python src/blockchain_client.py
python src/price_fetcher.py
python src/multi_dex_monitor.py
python src/arbitrage_detector.py

# 3. 纸盘测试（不执行真实交易）
python src/live_monitor.py

# 4. 小额实盘测试（$10-100）
python src/executor.py
```

### 部署到服务器

```bash
# 1. 安装 tmux（用于后台运行）
sudo apt install tmux

# 2. 创建新会话
tmux new -s dex-monitor

# 3. 启动监控
python src/live_monitor.py

# 4. 分离会话（Ctrl+B, D）
# 5. 重新连接：tmux attach -t dex-monitor
```

---

## 🔧 常见问题

### Q1: RPC 节点请求限制怎么办?

**A**: 使用多个 RPC 节点轮换:

```python
# src/blockchain_client.py
class BlockchainClient:
    def __init__(self):
        self.rpc_urls = [
            os.getenv("ALCHEMY_RPC_URL"),
            os.getenv("INFURA_RPC_URL"),
            os.getenv("QUICKNODE_RPC_URL")
        ]
        self.current_index = 0

    def switch_rpc(self):
        """切换 RPC 节点"""
        self.current_index = (self.current_index + 1) % len(self.rpc_urls)
        self.w3 = Web3(Web3.HTTPProvider(self.rpc_urls[self.current_index]))
```

### Q2: Gas 费突然飙升怎么办?

**A**: 动态调整 Gas 价格和利润阈值:

```python
# src/arbitrage_detector.py
class ArbitrageDetector:
    def estimate_gas_cost(self):
        # 实时获取 Gas 价格
        gas_price = self.blockchain.w3.eth.gas_price

        # 如果 Gas 价格过高，提高利润阈值
        if gas_price > threshold:
            self.min_profit_usd = Decimal("5.0")
        else:
            self.min_profit_usd = Decimal("1.0")
```

### Q3: 如何添加 Lighter 交易所?

**A**:

1. 获取 Lighter 的合约地址和 ABI
2. 在 `dex_config.py` 中添加配置
3. 实现 `LighterPriceFetcher` 类（参考 `UniswapV3PriceFetcher`）
4. 在 `MultiDexMonitor` 中集成

```python
# src/dex_config.py
DEXES = {
    # ...
    "lighter": {
        "name": "Lighter",
        "router": "0x...",  # 填写实际地址
        "quoter": "0x...",  # 填写实际地址
    }
}

# src/lighter_fetcher.py
class LighterPriceFetcher:
    def __init__(self, blockchain_client):
        # 初始化 Lighter 合约
        pass

    def get_price(self, token_in, token_out, amount):
        # 调用 Lighter 的价格查询接口
        pass
```

### Q4: 如何优化监控速度?

**A**: 使用多线程并行查询:

```python
# src/multi_dex_monitor.py
import concurrent.futures

class MultiDexMonitor:
    def fetch_all_prices(self):
        with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
            futures = []

            for pair in TRADING_PAIRS:
                future = executor.submit(self.fetch_pair_price, pair)
                futures.append(future)

            for future in concurrent.futures.as_completed(futures):
                result = future.result()
                # 处理结果
```

---

## 📚 下一步

1. ✅ **完成基础监控系统** - 已完成
2. ⏳ **添加更多 DEX** - Curve, Balancer, Lighter
3. ⏳ **集成 NautilusTrader** - 完整回测和风控
4. ⏳ **部署到云服务器** - AWS/GCP
5. ⏳ **实现自动化交易** - 智能执行引擎

---

## 📞 支持

遇到问题?

1. 检查 `.env` 配置
2. 验证钱包余额
3. 查看 RPC 节点状态
4. 参考 [NautilusTrader 文档](https://nautilustrader.io/docs/)

---

**⚠️ 免责声明**: 本指南仅用于教育目的。加密货币交易存在高风险,可能导致本金损失。在投入真实资金前,请充分评估风险并咨询专业人士。

---

*Document End*
