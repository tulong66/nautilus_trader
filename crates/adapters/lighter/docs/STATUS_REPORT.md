# Lighter DEX Adapter - 开发状态报告

> **最后更新**: 2026-05-12
> **状态**: ⚠️ Rust 骨架可编译/测试通过；Nautilus Python/Live 接入与执行回报闭环未完成

---

## 2026-05-12 缺口审计与 TODO

### 一句话结论

当前 `nautilus-lighter` 已是可编译的 Rust adapter 原型，但还不是 NautilusTrader 标准可配置 live adapter；下一阶段优先补齐 Python/PyO3/factory 接入和数据/执行闭环，不推进任何实盘资金路径。

### P0 — 接入层最小闭环

- [x] 在 workspace dependencies 中注册 `nautilus-lighter`，并让 `crates/pyo3` 依赖它。
- [x] 在 `crates/pyo3/src/lib.rs` 注册 `nautilus_lighter::python::lighter` 子模块。
- [x] 新增 `crates/adapters/lighter/src/factories.rs`，实现 `LighterDataClientFactory` / `LighterExecutionClientFactory`。
- [x] 新增 `LighterExecFactoryConfig`，显式承载 `TraderId` / `AccountId` / `LighterExecClientConfig`。
- [x] 扩展 `crates/adapters/lighter/src/python/`，暴露 config + factories，并注册 Nautilus 全局 factory/config extractor。
- [x] 新增 `nautilus_trader/adapters/lighter/` Python 包装层，至少包含 `__init__.py`、`config.py`、`factories.py`、`constants.py`。
- [x] 新增只导入的 Python smoke test：`tests/integration_tests/adapters/lighter/test_imports.py`。
- [x] 运行 Python runtime smoke test：`uv run --project ... --group test pytest tests/integration_tests/adapters/lighter/test_imports.py -q`，结果 3 passed；stable Rust 已更新到 1.95.0。

### P0 — Public data / paper recorder 前置

- [x] 修复 `LighterDataClient::request_bars()`：candlestick 响应已转换为 Nautilus `Bar`，并覆盖 1m/5m/15m/1h/4h/1d interval 映射。
- [x] 从 Lighter market metadata 派生真实 `price_decimals` / `size_decimals`：`tickSize` -> price precision，`stepSize` -> size precision，并用于 bar/orderbook/trade/ticker 转换。
- [x] 修复 ticker 到 `QuoteTick` 的简化逻辑：不再用 last price 伪造 bid/ask；只有 ticker 携带真实 best bid/ask 和 size 时才生成 `QuoteTick`。
- [x] 增加 public WS 数据路径测试：orderbook snapshot/update、trade、ticker，以及重连后订阅恢复消息构造。

### P1 — Execution 闭环任务板

> 目标：先完成 mock/offline execution event/report 闭环，让 adapter 能被 paper/replay 验证；不推进任何真实资金路径。
>
> 安全边界：不读取 `.env`、wallet、key 文件；不调用 Lighter mainnet 下单；不使用真实 private WS credentials；不运行任何真实资金命令。
>
> 并行策略：A 是共同前置；B/C/D/E 可在 A 后并行；F 依赖 B/C/D/E；G/H/I 可与 B/C/D/E 并行推进；完成一项就勾掉一项，若发现新缺口可在本节动态增删改。

| ID | 任务 | 状态 | 可并行性 | 验收标准 |
|----|------|------|----------|----------|
| A | 建立 execution fixture 基础层：定义 offline order/account/fill/position fixtures，覆盖 accepted/rejected/partial fill/filled/canceled/cancel rejected/account update | [x] | 前置 | 已新增 `execution::fixtures` 与 `tests/execution_fixtures.rs`；fixture 不含真实凭证；`cargo +1.95.0 test -p nautilus-lighter` 通过 |
| B | WebSocket order update dispatch：把 private `OrderUpdate` 映射为 Nautilus accepted/rejected/filled/canceled/cancel-rejected 等执行事件候选 | [x] | A 后可并行 | 已新增 `execution::dispatch` 与 `tests/execution_dispatch.rs`；mock WS message 能生成确定性 dispatch outcome；不连接真实 private WS |
| C | WebSocket account update dispatch：把 private `AccountUpdate` 映射为账户余额/状态更新候选，而不是只写 log | [x] | A 后可并行 | 已新增 account dispatch outcome；mock account update 能生成 account state/report outcome；不使用真实 token |
| D | Order status reports：实现 `generate_order_status_report(s)` 的 fixture-backed 转换、过滤和空结果语义 | [x] | A 后可并行 | 已新增 `execution::reports` 与 `tests/execution_reports.rs`；open/filled/canceled/rejected fixtures 可转 Nautilus `OrderStatusReport` |
| E | Fill / position / mass reports：实现 `generate_fill_reports`、`generate_position_status_reports`、`generate_mass_status` | [x] | A 后可并行 | fill/position fixtures 可转 Nautilus reports；mass status 汇总 orders/fills/positions |
| F | Execution client wiring：把 B/C/D/E 的纯转换层接入 `LighterExecutionClient`，保留 mock/offline 可测路径 | [x] | 依赖 B/C/D/E | `process_ws_message` 已接入 `dispatch_private_message`；report 方法仅在显式 offline fixture 参数/哨兵下返回 fixture-backed 报告，默认 live 路径仍为空并 warning；`cargo +1.95.0 check -p nautilus-lighter --features python` 通过；不新增 live 默认路径 |
| G | Market cache / precision cleanup：执行侧从真实 market cache 获取 `market_index`、`price_decimals`、`size_decimals`，移除 ETH/BTC/SOL/DOGE 硬编码 | [x] | A 后可并行 | `get_market_index` 已改为 cache-only；`market_index_lookup_uses_cache_not_symbol_heuristics` 覆盖未知市场；execution client 不再含 ETH/BTC/SOL/DOGE symbol fallback |
| H | Safety surface audit：确认 signing surface 只暴露 create order / cancel order / cancel all / auth token；withdraw/transfer/leverage/margin 不进入策略路径 | [x] | 可并行 | `signing_surface` 测试证明 strategy surface 只列 create/auth/cancel/cancel-all；代码搜索未发现 withdraw/transfer/leverage/margin 签名路径 |
| I | Verification + docs：运行完整验证并更新本状态报告、Track B plan 指针 | [x] | 收尾 | `cargo +1.95.0 test -p nautilus-lighter` 通过（129 passed, 2 ignored）；`cargo +1.95.0 check -p nautilus-lighter --features python` 通过；Python import smoke test 3 passed；本任务板已按实际完成状态更新 |

### P2 — 下一阶段任务板：live 前安全门与生产接入准备

> 目标：在不进入真实资金路径的前提下，把 adapter 从 mock/offline execution 闭环推进到 testnet/paper-ready 的受控 live-prep 状态。
>
> 安全边界：继续保持 no-withdraw/no-transfer signing surface；不读取 `.env`、wallet、key 文件；不调用 Lighter mainnet 下单；不使用真实 private WS credentials；不运行任何真实资金命令。
>
> 并行策略：J 是安全前置；K/L/M/N/O/P/Q 可在 J 后并行推进；R 依赖 J-Q 的验证结果收尾。若某项发现必须触碰认证、私钥、真实账号、真实下单、提现、授权或部署，立即停止并升级为人工决策。

| ID | 任务 | 状态 | 可并行性 | 验收标准 |
|----|------|------|----------|----------|
| J | Signer wrapper / capability whitelist / stage gate：把 create order / cancel order / cancel all / auth token 之外的签名能力从策略路径硬隔离 | [x] | 前置 | `LighterExecClientConfig.enable_live_signing` 默认关闭；`LighterExecutionClient` 只持有 `LighterStrategySigner` wrapper；`connect()` 在任何 HTTP/private WS/auth token 前拒绝默认配置；tests 覆盖 denied surface、disabled gate、explicit opt-in；无 env/key/secret 读取 |
| K | Private WS/auth dry-run harness：建立可注入 token/auth stub 与 private channel 订阅重放测试 | [ ] | J 后可并行 | 使用 fixture/stub 验证 auth/subscription/order/account message flow；不读取 `.env`；不连接真实 private WS |
| L | Live report API 设计与 mock server 接入：为 order/fill/position/mass report 设计真实数据来源接口，但只用 mock server 验证 | [ ] | J 后可并行 | 默认 live 路径仍受 stage gate 保护；mock REST/WS 能返回确定性 reports；空结果/错误/分页语义有测试 |
| M | Execution state reconciliation：建立 send/order update/account update/fill/cancel/cancel-reject 的状态机与去重规则 | [ ] | J 后可并行 | fixture/replay 覆盖 partial fill、filled、canceled、cancel rejected、重复消息、乱序消息；不产生真实订单 |
| N | Sequencer 与 `sendTx` 语义建模：区分 accepted/submitted/executed/rejected，处理 `code=200` 但未 executed 的状态 | [ ] | J 后可并行 | mock response 覆盖 sequencer reject、timeout、pending、executed、`code=200 != executed`；不会把 submitted 误报为 filled |
| O | Latency / retry / rate-limit 模型：整理 Standard 200/300ms latency、超时、重试、退避和限流策略 | [ ] | J 后可并行 | 单元测试覆盖 retry budget、timeout、rate-limit backoff；文档明确哪些路径可重试、哪些必须 fail-fast |
| P | Funding / margin / liquidation 风险输入：解析 funding endpoint/history，并建模 IMR/MMR/CMR/liquidation 前置数据 | [ ] | J 后可并行 | fixture-backed parser 覆盖 funding history、margin ratios、liquidation thresholds；不接入真实账户风险动作 |
| Q | Paper/replay soak 验证：用录制 public/private fixture 长时间回放，验证 execution/account/report 一致性 | [ ] | J 后可并行 | replay 不需要认证；覆盖断线重连、订阅恢复、重复消息、空账户/空订单；输出可复现实验记录 |
| R | Verification + docs：完成下一阶段验证并更新状态报告与 Track B 指针 | [ ] | 依赖 J-Q | `cargo +1.95.0 test -p nautilus-lighter`、`cargo +1.95.0 check -p nautilus-lighter --features python`、Python smoke test 通过；本任务板按实际结果更新 |

### P3 — 延后研究/增强

- [ ] maker-vs-taking / quote skew 研究仅在 signal-driven execution 明确需要后推进。
- [ ] 真实 testnet/private credential 流程只在 J-R 全部完成并人工批准后另起安全审计任务。

---

## 项目进度总览

```
████████████████████████████████████████ 100%
```

| 阶段 | 状态 | 完成日期 |
|------|------|----------|
| Phase 1: 签名方案调研 | ✅ 完成 | 2025-12-14 |
| Phase 2: 基础架构 | ✅ 完成 | 2025-12-14 |
| Phase 3: HTTP 客户端 | ✅ 完成 | 2025-12-14 |
| Phase 4: WebSocket 客户端 | ✅ 完成 | 2025-12-14 |
| Phase 5: 签名模块 | ✅ 完成 | 2025-12-14 |
| Phase 6: Data Client | ✅ 完成 | 2025-12-15 |
| Phase 7: Execution Client | ✅ 完成 | 2025-12-15 |
| Phase 8: Python 绑定 | ⏳ 待开发 | - |
| Phase 9: 测试文档 | ✅ 完成 | 2025-12-15 |

---

## 技术决策记录

### 签名方案：纯 Rust 实现 ✅

**最终决策**: 采用纯 Rust 实现，不使用 FFI

**选用库**:
- `goldilocks-crypto` v0.1.1 - ECgFp5 曲线 + Schnorr 签名
- `poseidon-hash` v0.1.3 - Poseidon2 哈希函数
- `num-bigint` v0.4 - 大整数运算

**理由**:
1. 无外部依赖，简化部署
2. 跨平台编译更容易
3. 完全控制签名流程
4. 经过验证测试通过

### WebSocket 端点

**正确的 URL 格式**:
- Mainnet: `wss://mainnet.zklighter.elliot.ai/stream`
- Testnet: `wss://testnet.zklighter.elliot.ai/stream`

**订阅频道格式**:
- `order_book:{market_index}` - 订单簿深度
- `trade:{market_index}` - 实时成交
- `market_stats:{market_index}` - 市场统计

---

## 测试结果

### 测试套件统计

| 测试类型 | 数量 | 状态 |
|---------|------|------|
| 单元测试 | 105 | ✅ 全部通过 |
| HTTP 集成测试 | 11 | ✅ 全部通过 |
| 签名集成测试 | 3 | ✅ 全部通过 |
| 文档测试 | 1 | ✅ 通过 |
| **总计** | **120** | ✅ **全部通过** |

### 功能验证

```
✅ HTTP 客户端
   - GET /order_books - 市场列表
   - GET /next_nonce - Nonce 获取
   - POST /send_tx - 交易发送
   - GET /candlesticks - K 线数据
   - GET /account - 账户信息
   - GET /account_active_orders - 活跃订单
   - GET /recent_trades - 最近成交
   - GET /order_book_orders - 订单簿深度

✅ WebSocket 客户端
   - 连接: wss://testnet.zklighter.elliot.ai/stream
   - 订阅: order_book:1, trade:1, market_stats:1
   - 心跳: JSON ping/pong 机制
   - 重连: 指数退避策略

✅ 签名模块
   - Schnorr 签名生成
   - Poseidon2 哈希
   - 签名验证
   - 确定性签名
```

### 编译状态

```
cargo build -p nautilus-lighter
   Compiling nautilus-lighter v0.52.0
    Finished `dev` profile [unoptimized] target(s)

⚠️  Warnings: 0
❌ Errors: 0
```

---

## 已实现模块

### 目录结构

```
crates/adapters/lighter/
├── Cargo.toml              ✅
├── src/
│   ├── lib.rs              ✅ 模块导出
│   ├── error.rs            ✅ 错误类型
│   │
│   ├── common/             ✅ 公共组件
│   │   ├── mod.rs
│   │   ├── enums.rs        ✅ 环境、订单类型、TIF
│   │   ├── types.rs        ✅ 类型别名
│   │   └── urls.rs         ✅ URL 构建器
│   │
│   ├── http/               ✅ REST API
│   │   ├── mod.rs
│   │   ├── client.rs       ✅ HTTP 客户端
│   │   ├── endpoints.rs    ✅ API 端点常量
│   │   ├── types.rs        ✅ 请求/响应模型
│   │   └── parse.rs        ✅ 响应解析
│   │
│   ├── websocket/          ✅ WebSocket
│   │   ├── mod.rs
│   │   ├── client.rs       ✅ WS 客户端
│   │   └── messages.rs     ✅ 消息类型
│   │
│   ├── signing/            ✅ 签名模块
│   │   ├── mod.rs
│   │   ├── signer.rs       ✅ Schnorr 签名器
│   │   └── nonce.rs        ✅ Nonce 管理
│   │
│   ├── data/               ✅ 数据客户端
│   │   ├── mod.rs
│   │   ├── client.rs       ✅ DataClient 实现
│   │   └── types.rs        ✅ 数据类型转换
│   │
│   └── execution/          ✅ 执行客户端
│       ├── mod.rs
│       └── client.rs       ✅ ExecutionClient 实现
│
├── tests/
│   ├── http.rs             ✅ HTTP 集成测试 (11 tests)
│   └── test_signing.rs     ✅ 签名集成测试 (3 tests)
│
├── examples/
│   ├── websocket_example.rs    ✅ WebSocket 示例
│   └── test_signing_standalone.rs  ✅ 签名独立测试
│
├── test_data/              ✅ 测试数据
│   ├── order_books.json
│   └── send_tx.json
│
└── docs/
    ├── IMPLEMENTATION_PLAN.md  📄 原始计划
    ├── INTEGRATED_PLAN.md      📄 整合计划
    └── STATUS_REPORT.md        📄 本文档
```

---

## API 实现状态

### HTTP 端点

| 端点 | 方法 | 状态 | 测试 |
|------|------|------|------|
| `/api/v1/order_books` | GET | ✅ | ✅ |
| `/api/v1/order_book_details` | GET | ✅ | - |
| `/api/v1/order_book_orders` | GET | ✅ | ✅ |
| `/api/v1/recent_trades` | GET | ✅ | ✅ |
| `/api/v1/candlesticks` | GET | ✅ | ✅ |
| `/api/v1/account` | GET | ✅ | ✅ |
| `/api/v1/account_active_orders` | GET | ✅ | ✅ |
| `/api/v1/next_nonce` | GET | ✅ | ✅ |
| `/api/v1/send_tx` | POST | ✅ | ✅ |
| `/api/v1/send_tx_batch` | POST | ✅ | - |

### WebSocket 频道

| 频道 | 类型 | 状态 | 测试 |
|------|------|------|------|
| `order_book:{id}` | 公开 | ✅ | ✅ 实时验证 |
| `trade:{id}` | 公开 | ✅ | ✅ 实时验证 |
| `market_stats:{id}` | 公开 | ✅ | ✅ 实时验证 |
| `account_all:{account}` | 私有 | ✅ | - |
| `account_all_orders:{account}` | 私有 | ✅ | - |
| `user_stats:{account}` | 私有 | ✅ | - |

---

## 待完成工作

### Phase 8: Python 绑定 (待开发)

```
src/python/
├── mod.rs          ⏳ PyO3 模块入口
├── config.rs       ⏳ 配置类绑定
├── enums.rs        ⏳ 枚举导出
├── http.rs         ⏳ HTTP 客户端绑定
└── websocket.rs    ⏳ WebSocket 绑定

nautilus_trader/adapters/lighter/
├── __init__.py     ⏳
├── config.py       ⏳ Python 配置类
├── factories.py    ⏳ 客户端工厂
├── data.py         ⏳ DataClient 包装
├── execution.py    ⏳ ExecutionClient 包装
└── providers.py    ⏳ InstrumentProvider
```

### 优化建议

1. **性能优化**
   - [ ] 添加连接池
   - [ ] 实现请求批处理
   - [ ] 优化序列化/反序列化

2. **可靠性增强**
   - [ ] 添加断路器模式
   - [ ] 实现更完善的重试策略
   - [ ] 添加健康检查机制

3. **监控能力**
   - [ ] 添加 metrics 收集
   - [ ] 实现延迟追踪
   - [ ] 日志级别优化

---

## 依赖清单

### 核心依赖

```toml
[dependencies]
nautilus-common = { workspace = true, features = ["live"] }
nautilus-core = { workspace = true }
nautilus-model = { workspace = true }
nautilus-network = { workspace = true }
nautilus-live = { workspace = true }

# 签名库
goldilocks-crypto = "0.1.1"
poseidon-hash = "0.1.3"
num-bigint = "0.4"

# 异步运行时
tokio = { workspace = true }
tokio-tungstenite = { workspace = true }

# 序列化
serde = { workspace = true }
serde_json = { workspace = true }
```

### 开发依赖

```toml
[dev-dependencies]
nautilus-testkit = { workspace = true }
axum = { workspace = true }  # Mock 服务器
rstest = { workspace = true }
```

---

## 运行说明

### 构建

```bash
# 开发构建
cargo build -p nautilus-lighter

# 发布构建
cargo build -p nautilus-lighter --release
```

### 测试

```bash
# 运行所有测试
cargo test -p nautilus-lighter

# 仅单元测试
cargo test -p nautilus-lighter --lib

# HTTP 集成测试
cargo test -p nautilus-lighter --test http

# 签名测试
cargo test -p nautilus-lighter --test test_signing
```

### 示例

```bash
# WebSocket 连接示例
cargo run --example websocket_example -p nautilus-lighter

# 签名功能测试
cargo run --example test_signing_standalone -p nautilus-lighter
```

---

## 参考资源

- [Lighter API 文档](https://apidocs.lighter.xyz/)
- [NautilusTrader 文档](https://nautilustrader.io/docs/)
- [Hyperliquid 适配器参考](../hyperliquid/)
- [Bybit 适配器参考](../bybit/)

---

## 变更日志

### 2025-12-15

- ✅ 完成所有功能测试验证
- ✅ 修复 WebSocket URL 路径问题
- ✅ 清除所有编译警告
- ✅ 更新状态报告文档

### 2025-12-14

- ✅ 完成基础架构搭建
- ✅ 实现 HTTP 客户端和所有端点
- ✅ 实现 WebSocket 客户端
- ✅ 实现纯 Rust 签名模块
- ✅ 实现 DataClient 和 ExecutionClient
- ✅ 添加集成测试套件

---

**文档版本**: v2.0
**作者**: Claude Code Agent
