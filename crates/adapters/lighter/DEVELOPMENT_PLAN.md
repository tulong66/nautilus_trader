# Lighter DEX Connector - 并行开发计划

> 创建日期: 2025-12-14
> 项目: NautilusTrader Lighter Adapter
> 基于: hftbacktest 团队开发经验 + NautilusTrader 适配器架构

---

## 1. 项目概述

### 1.1 目标
为 NautilusTrader 开发 Lighter DEX 连接器，支持：
- 市场数据订阅（WebSocket）
- 订单执行（REST + WebSocket）
- 账户信息查询

### 1.2 技术决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 实现语言 | Rust + Python 绑定 | 遵循 NautilusTrader 架构（如 Bybit/Hyperliquid） |
| 签名方案 | **FFI 调用 Go 库** | hftbacktest 经验证明 lighter-rust SDK 有构建问题，Go FFI 方案成熟稳定 |
| Nonce 管理 | Per-API-Key 原子计数器 | Lighter 要求每个 API Key 独立管理 nonce |

### 1.3 Lighter 特殊要求

| 特性 | 说明 | 影响 |
|------|------|------|
| 动态价格精度 | `price_decimals` 每个市场不同 (1-6) | 需要缓存市场元数据 |
| 市场发现 | 只能通过 `order_books()` API | 启动时必须调用 |
| 双通道确认 | REST 返回 + WebSocket 推送 | 需要协调两个来源 |
| 应用层心跳 | WebSocket JSON ping/pong | 非帧级 ping |
| Auth Token | 10分钟过期 | 私有频道需要定期刷新 |

---

## 2. 模块结构

```
crates/adapters/lighter/
├── Cargo.toml
├── build.rs                    # FFI 链接配置
├── README.md
├── DEVELOPMENT_PLAN.md         # 本文档
├── libs/                       # 预编译 Go 库
│   ├── linux/amd64/
│   │   ├── liblighter-signer.so
│   │   └── liblighter-signer.h
│   ├── darwin/
│   │   ├── amd64/
│   │   └── arm64/
│   └── windows/amd64/
├── lighter-go/                 # Go 签名库源码
│   └── sharedlib/main.go
├── bin/                        # 测试/示例二进制
│   ├── http_public.rs
│   ├── http_private.rs
│   ├── ws_data.rs
│   └── ws_exec.rs
└── src/
    ├── lib.rs                  # 模块导出
    ├── config.rs               # 配置结构
    │
    ├── common/                 # 公共组件
    │   ├── mod.rs
    │   ├── consts.rs           # 常量 (VENUE, URLs)
    │   ├── enums.rs            # 枚举 (OrderType, TIF, Side)
    │   ├── types.rs            # 类型别名
    │   ├── models.rs           # API 数据模型
    │   ├── parse.rs            # 解析工具
    │   └── converters.rs       # 类型转换
    │
    ├── http/                   # REST API
    │   ├── mod.rs
    │   ├── client.rs           # HTTP 客户端
    │   ├── error.rs            # HTTP 错误
    │   ├── models.rs           # 请求/响应模型
    │   ├── parse.rs            # 响应解析
    │   └── rate_limits.rs      # 限速
    │
    ├── websocket/              # WebSocket
    │   ├── mod.rs
    │   ├── client.rs           # WS 客户端
    │   ├── codec.rs            # 消息编解码
    │   ├── messages.rs         # 消息类型
    │   ├── handler.rs          # 消息处理
    │   ├── parse.rs            # 消息解析
    │   └── error.rs            # WS 错误
    │
    ├── signing/                # 签名模块
    │   ├── mod.rs
    │   ├── ffi.rs              # Go FFI 绑定
    │   ├── signer.rs           # 签名器封装
    │   ├── nonce.rs            # Nonce 管理
    │   └── types.rs            # 签名相关类型
    │
    ├── data/                   # 数据客户端
    │   └── mod.rs              # DataClient 实现
    │
    ├── execution/              # 执行客户端
    │   └── mod.rs              # ExecutionClient 实现
    │
    └── python/                 # Python 绑定
        ├── mod.rs
        ├── http.rs
        ├── websocket.rs
        ├── enums.rs
        └── urls.rs
```

---

## 3. 并行开发工作流

### 3.1 工作流图

```
                    ┌──────────────────┐
                    │   Phase 0: 准备   │
                    │  目录结构 + 配置   │
                    └────────┬─────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   Stream A      │ │   Stream B      │ │   Stream C      │
│   签名模块       │ │   HTTP 客户端    │ │   WebSocket     │
│                 │ │                 │ │                 │
│ • FFI 绑定      │ │ • 基础客户端     │ │ • 连接管理      │
│ • Nonce 管理    │ │ • 公开 API      │ │ • 公开频道      │
│ • 单元测试      │ │ • 解析器        │ │ • 消息解析      │
└────────┬────────┘ └────────┬────────┘ └────────┬────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
                             │
                    ┌────────▼─────────┐
                    │   Phase 2: 集成   │
                    │  私有 API + 执行  │
                    └────────┬─────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   DataClient    │ │ ExecutionClient │ │   Python 绑定   │
└─────────────────┘ └─────────────────┘ └─────────────────┘
                             │
                    ┌────────▼─────────┐
                    │   Phase 3: 测试   │
                    │  集成 + 验收测试  │
                    └──────────────────┘
```

### 3.2 并行任务分配

#### Stream A: 签名模块 (独立开发)
**前置条件**: 无
**输出**: `signing/` 模块

| 任务 | 优先级 | 估计工时 | 可并行 |
|------|--------|----------|--------|
| A1: 设置 lighter-go 源码和构建脚本 | P0 | 2h | ✓ |
| A2: 实现 FFI 绑定 (`ffi.rs`) | P0 | 4h | ✓ |
| A3: 实现 NonceManager | P0 | 2h | ✓ |
| A4: 实现 Signer 封装 | P0 | 2h | - |
| A5: 单元测试 | P1 | 2h | - |

#### Stream B: HTTP 客户端 (独立开发)
**前置条件**: 无
**输出**: `http/` 模块

| 任务 | 优先级 | 估计工时 | 可并行 |
|------|--------|----------|--------|
| B1: 定义 HTTP 错误类型 | P0 | 1h | ✓ |
| B2: 实现基础 HTTP 客户端 | P0 | 3h | ✓ |
| B3: 实现公开 API (order_books, trades) | P0 | 3h | - |
| B4: 实现响应解析器 | P0 | 2h | - |
| B5: 实现 Instrument 缓存 | P1 | 2h | - |

#### Stream C: WebSocket 客户端 (独立开发)
**前置条件**: 无
**输出**: `websocket/` 模块

| 任务 | 优先级 | 估计工时 | 可并行 |
|------|--------|----------|--------|
| C1: 定义消息类型 | P0 | 2h | ✓ |
| C2: 实现连接管理 | P0 | 3h | ✓ |
| C3: 实现公开频道订阅 | P0 | 2h | - |
| C4: 实现应用层心跳 | P0 | 1h | - |
| C5: 实现消息解析器 | P0 | 3h | - |

#### Phase 2: 集成 (需要 A+B+C 完成)

| 任务 | 依赖 | 估计工时 |
|------|------|----------|
| D1: HTTP 私有 API (sendTx, nextNonce) | A4, B2 | 3h |
| D2: WebSocket 私有频道 (auth, orders) | A4, C2 | 3h |
| D3: DataClient 实现 | B3, C3 | 4h |
| D4: ExecutionClient 实现 | D1, D2 | 6h |
| D5: Python 绑定 | D3, D4 | 4h |

#### Phase 3: 测试与文档

| 任务 | 估计工时 |
|------|----------|
| E1: 集成测试 | 4h |
| E2: 示例二进制 | 2h |
| E3: 文档 | 2h |

---

## 4. API 映射

### 4.1 REST API

| Lighter API | NautilusTrader 用途 | 模块 |
|-------------|-------------------|------|
| `GET /api/v1/order_books` | 获取市场列表 + 元数据 | http/client.rs |
| `GET /api/v1/trades` | 历史成交 | http/client.rs |
| `GET /api/v1/candlesticks` | K线数据 | http/client.rs |
| `GET /api/v1/account_info` | 账户余额 | http/client.rs |
| `GET /api/v1/open_orders` | 当前挂单 | http/client.rs |
| `GET /api/v1/nextNonce` | Nonce 同步 | http/client.rs |
| `POST /api/v1/sendTx` | 发送签名交易 | http/client.rs |

### 4.2 WebSocket Channels

| Channel | 类型 | 用途 |
|---------|------|------|
| `orderbook:{market_id}` | 公开 | 订单簿深度 |
| `trades:{market_id}` | 公开 | 实时成交 |
| `candlesticks:{market_id}:{interval}` | 公开 | K线更新 |
| `account` | 私有 | 账户更新 |
| `orders` | 私有 | 订单状态更新 |
| `fills` | 私有 | 成交通知 |

### 4.3 NautilusTrader Trait 映射

```rust
// DataClient
impl DataClient for LighterDataClient {
    fn subscribe_quotes()      // -> orderbook channel
    fn subscribe_trades()      // -> trades channel
    fn subscribe_bars()        // -> candlesticks channel
    fn request_instruments()   // -> order_books REST API
}

// ExecutionClient
impl ExecutionClient for LighterExecutionClient {
    fn submit_order()          // -> sendTx (CreateOrder)
    fn cancel_order()          // -> sendTx (CancelOrder)
    fn cancel_all_orders()     // -> sendTx (CancelAllOrders)
    fn modify_order()          // -> sendTx (ModifyOrder)
    fn query_order()           // -> open_orders REST
    fn query_account()         // -> account_info REST
}
```

---

## 5. 签名实现细节

### 5.1 FFI 函数签名

```rust
// ffi.rs
#[link(name = "lighter-signer")]
unsafe extern "C" {
    fn CreateClient(
        url: *mut c_char,
        private_key: *mut c_char,
        chain_id: c_int,
        api_key_index: c_int,
        account_index: c_longlong,
    ) -> *mut c_char;

    fn SignCreateOrder(
        market_index: c_int,
        client_order_index: c_longlong,
        base_amount: c_longlong,
        price: c_int,
        is_ask: c_int,
        order_type: c_int,
        time_in_force: c_int,
        reduce_only: c_int,
        trigger_price: c_int,
        order_expiry: c_longlong,
        nonce: c_longlong,
    ) -> StrOrErr;

    fn SignCancelOrder(
        market_index: c_int,
        order_index: c_longlong,
        nonce: c_longlong,
    ) -> StrOrErr;

    fn SignCancelAllOrders(
        time_in_force: c_int,
        time: c_longlong,
        nonce: c_longlong,
    ) -> StrOrErr;

    fn CreateAuthToken(deadline: c_longlong) -> StrOrErr;
}
```

### 5.2 Nonce 管理策略

```rust
// nonce.rs
pub struct NonceManager {
    nonce: AtomicU64,
}

impl NonceManager {
    pub fn next(&self) -> u64;      // 获取并递增
    pub fn current(&self) -> u64;   // 仅查看
    pub fn reset(&self, value: u64); // 同步服务器值
    pub fn rollback(&self);         // 错误回滚
}
```

### 5.3 错误恢复流程

```
发送交易 -> 失败?
           │
           ├─ Nonce 错误 (21601/21602)
           │  └─ rollback() -> sync_nonce() -> 重试
           │
           └─ 其他错误
              └─ 返回错误
```

---

## 6. 配置结构

```rust
// config.rs
#[derive(Clone, Debug)]
pub struct LighterDataClientConfig {
    pub chain_id: u32,              // 300=testnet, 304=mainnet
    pub base_url_http: Option<String>,
    pub base_url_ws: Option<String>,
    pub http_timeout_secs: Option<u64>,
    pub ws_heartbeat_secs: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct LighterExecClientConfig {
    pub chain_id: u32,
    pub private_key: String,
    pub api_key_index: u8,
    pub account_index: i64,
    pub base_url_http: Option<String>,
    pub base_url_ws: Option<String>,
    pub http_timeout_secs: Option<u64>,
    pub ws_heartbeat_secs: Option<u64>,
    pub auth_token_refresh_secs: Option<u64>,
}
```

---

## 7. 里程碑

| 里程碑 | 目标 | 预计完成 |
|--------|------|----------|
| M0 | 目录结构 + 配置 + 常量 | Day 1 |
| M1 | Stream A/B/C 完成 (可独立测试) | Day 3 |
| M2 | 私有 API + 签名集成 | Day 5 |
| M3 | DataClient + ExecutionClient | Day 7 |
| M4 | Python 绑定 + 测试 | Day 9 |
| M5 | 文档 + 代码审查 | Day 10 |

---

## 8. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| lighter-go 构建问题 | 阻塞签名模块 | 复用 hftbacktest 预编译库 |
| Nonce 同步失败 | 订单提交失败 | 实现自动重试 + 回滚机制 |
| WebSocket 断连 | 数据丢失 | 实现重连 + 状态恢复 |
| 价格精度不匹配 | 订单被拒绝 | 启动时缓存所有市场元数据 |

---

## 9. 参考资源

- [Lighter API 文档](https://docs.lighter.xyz)
- [NautilusTrader Hyperliquid 适配器](../hyperliquid/)
- [NautilusTrader Bybit 适配器](../bybit/)
- [hftbacktest Lighter 开发经验](/home/deepzen/works/hftbacktest/.serena/memories/lighter_connector_development_experience.md)

---

## 10. 下一步行动

1. **立即启动** (可并行):
   - [ ] 创建目录结构和 Cargo.toml
   - [ ] 复制 lighter-go 源码和预编译库
   - [ ] 定义 common/ 模块（常量、枚举、类型）

2. **Phase 1 并行开发**:
   - [ ] Stream A: 签名模块
   - [ ] Stream B: HTTP 客户端
   - [ ] Stream C: WebSocket 客户端

3. **决策待定**:
   - [ ] 确认是否需要探索纯 Rust 签名方案
