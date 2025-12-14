# Lighter DEX Connector - 开发计划 (经理人B版本)

> 作者: 经理人B
> 日期: 2025-12-14
> 方法论: **风险驱动开发 (Risk-Driven Development)**

---

## 1. 核心理念

与传统的"功能驱动"开发不同，本计划采用**风险驱动**方法：
- 先验证高风险技术点
- 快速失败，快速调整
- 增量交付可工作的代码

---

## 2. 风险矩阵

| 风险项 | 影响 | 概率 | 优先级 | 缓解策略 |
|--------|------|------|--------|----------|
| **R1**: FFI 签名在 NautilusTrader 构建系统中不工作 | 致命 | 中 | **P0** | 先做 PoC |
| **R2**: Lighter API 与文档不一致 | 高 | 中 | **P1** | 先测试公开 API |
| **R3**: Nonce 并发问题 | 高 | 中 | **P1** | 参考 Hyperliquid 实现 |
| **R4**: WebSocket 重连状态丢失 | 中 | 高 | **P2** | 设计状态恢复机制 |
| **R5**: 价格精度处理错误 | 中 | 中 | **P2** | 完整的单元测试 |

---

## 3. 开发阶段 (风险优先)

### Phase 0: 技术验证 (Day 1) ⚠️ 阻塞点

**目标**: 验证 FFI 签名方案在 NautilusTrader 中可行

```
验证清单:
□ lighter-go 能否在当前环境编译
□ Rust FFI 能否正确链接 .so/.dylib
□ 签名结果能否被 Lighter testnet 接受
□ 与 NautilusTrader 构建系统兼容
```

**交付物**: `signing/` 模块 PoC + 测试脚本

**决策点**: 如果 FFI 方案失败，需要:
1. 研究纯 Rust 实现可行性
2. 或者考虑 subprocess 调用 Go 二进制

### Phase 1: 公开 API (Day 2-3)

**目标**: 实现无需签名的功能，可独立测试

```rust
// 可独立测试的功能
impl LighterHttpClient {
    pub async fn get_order_books() -> Result<Vec<Market>>;
    pub async fn get_trades(market_id: u32) -> Result<Vec<Trade>>;
    pub async fn get_candlesticks(...) -> Result<Vec<Candle>>;
}

impl LighterWebSocketClient {
    pub async fn subscribe_orderbook(market_id: u32);
    pub async fn subscribe_trades(market_id: u32);
}
```

**验证**: 连接 Lighter testnet，获取真实数据

### Phase 2: 签名集成 (Day 4-5)

**目标**: 将 Phase 0 的签名 PoC 集成到 HTTP 客户端

```rust
impl LighterHttpClient {
    pub async fn send_order(&self, order: CreateOrderRequest) -> Result<OrderResponse>;
    pub async fn cancel_order(&self, cancel: CancelOrderRequest) -> Result<()>;
    pub async fn get_account_info(&self) -> Result<AccountInfo>;
}
```

### Phase 3: NautilusTrader 集成 (Day 6-8)

**目标**: 实现 DataClient 和 ExecutionClient trait

### Phase 4: Python 绑定 + 测试 (Day 9-10)

---

## 4. 代码组织建议

### 4.1 模块依赖图

```
                    ┌─────────────┐
                    │   config    │
                    └──────┬──────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   common    │    │   signing   │    │    error    │
│  (无依赖)   │    │  (FFI/Go)   │    │  (无依赖)   │
└──────┬──────┘    └──────┬──────┘    └──────┬──────┘
       │                  │                  │
       └─────────────────┬┴──────────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│    http     │  │  websocket  │  │   models    │
└──────┬──────┘  └──────┬──────┘  └─────────────┘
       │                │
       └────────┬───────┘
                │
         ┌──────┴──────┐
         │             │
         ▼             ▼
┌─────────────┐ ┌─────────────┐
│    data     │ │  execution  │
│   client    │ │   client    │
└─────────────┘ └─────────────┘
```

### 4.2 错误处理策略

```rust
// error.rs - 统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum LighterError {
    // 网络层
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    // 签名层
    #[error("Signing error: {0}")]
    Signing(String),

    #[error("Nonce error: expected {expected}, got {actual}")]
    Nonce { expected: u64, actual: u64 },

    // 业务层
    #[error("Order rejected: {code} - {message}")]
    OrderRejected { code: i32, message: String },

    #[error("Market not found: {0}")]
    MarketNotFound(u32),

    // 解析层
    #[error("Parse error: {0}")]
    Parse(String),
}
```

### 4.3 配置分离

```rust
// config.rs
/// 公开 API 配置 (无需凭证)
#[derive(Clone, Debug)]
pub struct LighterPublicConfig {
    pub chain_id: ChainId,
    pub http_base_url: Option<String>,
    pub ws_base_url: Option<String>,
    pub http_timeout_secs: u64,
    pub ws_ping_interval_secs: u64,
}

/// 私有 API 配置 (需要凭证)
#[derive(Clone, Debug)]
pub struct LighterPrivateConfig {
    pub public: LighterPublicConfig,
    pub private_key: String,
    pub api_key_index: u8,
    pub account_index: i64,
}

/// 链 ID
#[derive(Clone, Copy, Debug, Default)]
pub enum ChainId {
    #[default]
    Testnet = 300,
    Mainnet = 304,
}
```

---

## 5. 测试策略

### 5.1 测试金字塔

```
                    ┌─────────────┐
                    │  E2E Tests  │  <- Testnet 集成
                    │   (少量)    │
                    ├─────────────┤
                    │ Integration │  <- Mock Server
                    │   Tests     │
                    ├─────────────┤
                    │    Unit     │  <- 纯逻辑测试
                    │   Tests     │
                    └─────────────┘
```

### 5.2 Mock 策略

```rust
// 为 HTTP 客户端定义 trait，便于 mock
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T>;
    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T>;
}

// 生产实现
pub struct ReqwestTransport { ... }

// 测试实现
pub struct MockTransport {
    responses: HashMap<String, serde_json::Value>,
}
```

### 5.3 测试数据管理

```
tests/
├── fixtures/
│   ├── order_books.json      # 市场列表响应
│   ├── trades.json           # 成交响应
│   ├── ws_orderbook.json     # WS 订单簿消息
│   └── ws_order_update.json  # WS 订单更新
├── integration/
│   ├── test_http_public.rs
│   ├── test_http_private.rs
│   └── test_websocket.rs
└── unit/
    ├── test_parsing.rs
    ├── test_nonce.rs
    └── test_signing.rs
```

---

## 6. 与 hftbacktest 实现对比

| 方面 | hftbacktest | NautilusTrader (建议) |
|------|-------------|----------------------|
| 架构 | Connector trait | DataClient + ExecutionClient |
| 异步运行时 | tokio | tokio (一致) |
| HTTP 客户端 | reqwest | reqwest (一致) |
| WebSocket | tokio-tungstenite | tokio-tungstenite (一致) |
| 签名 | FFI Go | FFI Go (复用) |
| Nonce | AtomicU64 | 参考 Hyperliquid NonceManager |
| 错误处理 | 自定义 Error | thiserror (标准化) |

### 可复用代码

1. **签名 FFI 绑定** - 直接复用 `ffi.rs` 结构
2. **lighter-go 库** - 直接复用预编译二进制
3. **消息类型定义** - 参考但需适配 NautilusTrader 模型

### 需要重写

1. **HTTP 客户端** - 适配 NautilusTrader 风格
2. **WebSocket 客户端** - 集成 NautilusTrader 事件系统
3. **数据模型** - 转换为 NautilusTrader 类型

---

## 7. 关键代码片段

### 7.1 市场元数据缓存

```rust
// Lighter 特有: 动态价格精度
pub struct MarketCache {
    markets: DashMap<u32, MarketInfo>,
}

#[derive(Clone, Debug)]
pub struct MarketInfo {
    pub market_id: u32,
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub price_decimals: u8,      // 关键: 每个市场不同
    pub size_decimals: u8,
    pub min_base_amount: i64,
    pub instrument_id: InstrumentId,  // NautilusTrader 类型
}

impl MarketCache {
    /// 启动时必须调用，填充缓存
    pub async fn initialize(&self, http: &LighterHttpClient) -> Result<()> {
        let order_books = http.get_order_books().await?;
        for ob in order_books {
            let info = MarketInfo::from_order_book(&ob);
            self.markets.insert(info.market_id, info);
        }
        Ok(())
    }

    /// 获取市场信息，用于价格/数量转换
    pub fn get(&self, market_id: u32) -> Option<MarketInfo> {
        self.markets.get(&market_id).map(|r| r.clone())
    }
}
```

### 7.2 双通道订单确认

```rust
// 订单状态管理器
pub struct OrderStateManager {
    // client_order_id -> 订单状态
    pending_orders: DashMap<ClientOrderId, PendingOrder>,
}

#[derive(Debug)]
struct PendingOrder {
    client_order_id: ClientOrderId,
    rest_confirmed: bool,
    ws_confirmed: bool,
    venue_order_id: Option<VenueOrderId>,
    created_at: Instant,
}

impl OrderStateManager {
    /// REST 响应到达
    pub fn on_rest_response(&self, client_order_id: &ClientOrderId, venue_order_id: VenueOrderId) {
        if let Some(mut order) = self.pending_orders.get_mut(client_order_id) {
            order.rest_confirmed = true;
            order.venue_order_id = Some(venue_order_id);
            self.try_emit_accepted(&order);
        }
    }

    /// WebSocket 消息到达
    pub fn on_ws_order_update(&self, update: &WsOrderUpdate) {
        if let Some(mut order) = self.pending_orders.get_mut(&update.client_order_id) {
            order.ws_confirmed = true;
            self.try_emit_accepted(&order);
        }
    }

    /// 两个通道都确认后才发出 Accepted 事件
    fn try_emit_accepted(&self, order: &PendingOrder) {
        if order.rest_confirmed && order.ws_confirmed {
            // 发送 OrderAccepted 事件到 NautilusTrader
        }
    }
}
```

### 7.3 应用层心跳

```rust
// WebSocket 心跳 (JSON 级别，非帧级别)
impl LighterWebSocketClient {
    async fn heartbeat_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(15));

        loop {
            interval.tick().await;

            // Lighter 使用 JSON 消息作为心跳
            let ping = json!({
                "method": "ping",
                "id": self.next_request_id()
            });

            if let Err(e) = self.send_json(&ping).await {
                tracing::warn!("Heartbeat failed: {}", e);
                self.reconnect().await;
            }
        }
    }
}
```

---

## 8. 并行开发建议

### 8.1 工作分配

| 开发者 | 任务 | 依赖 |
|--------|------|------|
| Dev A | Phase 0: FFI 签名验证 | 无 |
| Dev B | Phase 1a: HTTP 公开 API | 无 |
| Dev C | Phase 1b: WebSocket 公开 | 无 |
| Dev A | Phase 2: 签名集成 | Phase 0, 1a |
| Dev B | Phase 3a: DataClient | Phase 1a, 1b |
| Dev C | Phase 3b: ExecutionClient | Phase 2 |

### 8.2 每日同步点

```
Day 1 EOD: Phase 0 结果 -> 决定是否继续 FFI 方案
Day 3 EOD: Phase 1 完成 -> 公开 API 可测试
Day 5 EOD: Phase 2 完成 -> 签名流程端到端验证
Day 8 EOD: Phase 3 完成 -> NautilusTrader 集成可测试
```

---

## 9. 我的建议优先级

1. **先做 FFI PoC** - 这是最大风险点，1天内验证
2. **公开 API 并行开发** - 无阻塞依赖
3. **复用 hftbacktest 代码** - 签名部分直接复用
4. **测试先行** - 每个模块完成后立即写测试

---

## 10. 与经理人A计划对比要点

期待看到经理人A的计划，我们可以讨论：

1. **Phase 0 是否必要?** - 我认为 FFI 验证是阻塞点
2. **模块划分方式** - 我按依赖关系划分
3. **测试策略** - 我强调 Mock 和单元测试
4. **风险评估** - 我列出了具体风险矩阵

让我们合并两个计划的优点！
