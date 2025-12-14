# Lighter DEX Connector - 综合开发计划

> **整合自**: PM-A (功能驱动) + PM-B (风险驱动)
> **日期**: 2025-12-14
> **方法论**: **风险感知的功能驱动开发 (Risk-Aware Feature-Driven Development)**

---

## 执行摘要

| 项目 | 详情 |
|------|------|
| **目标** | Rust 核心实现 + PyO3 Python 绑定 |
| **功能** | 完整的市场数据、订单执行、账户信息支持 |
| **预计工期** | 10-12 工作日（并行开发 + 风险前置） |
| **方法论** | 风险前置验证 + 功能并行开发 + 逐阶段审核 |

---

## 1. 方案对比与整合决策

### 1.1 两个方案的核心差异

| 维度 | PM-A (功能驱动) | PM-B (风险驱动) | **整合决策** |
|------|----------------|----------------|-------------|
| **阶段划分** | 9 阶段按功能 | 5 阶段按风险 | 6 阶段（风险前置 + 功能并行） |
| **首要任务** | 签名调研 + 基础架构 | FFI PoC 验证 | **Phase 0: FFI PoC** (采纳 PM-B) |
| **工期估算** | 20 工作日 | 10 工作日 | **12 工作日**（保守估算） |
| **并行策略** | Week 1-2 并行 | Dev A/B/C 分工 | **模块并行 + 接口先行** |
| **测试策略** | 最后阶段 | 测试先行 | **测试先行** (采纳 PM-B) |
| **配置设计** | Data/Exec 分离 | Public/Private 分离 | **Public/Private** (更清晰) |

### 1.2 采纳的关键决策

#### ✅ 从 PM-B 采纳
1. **Phase 0 FFI 验证** - 作为阻塞性风险必须先验证
2. **风险矩阵** - 明确风险优先级
3. **Public/Private 配置分离** - 更清晰的关注点分离
4. **测试先行** - Mock 策略 + 测试金字塔
5. **HttpTransport trait** - 便于测试的抽象
6. **DashMap 用于缓存** - 线程安全的并发 HashMap

#### ✅ 从 PM-A 采纳
1. **详细的代码结构** - 每个模块的文件组织
2. **动态价格精度的详细实现** - 公式和示例
3. **双通道确认的 GC 机制** - 5分钟超时清理
4. **Nonce rollback 机制** - 签名失败恢复
5. **Python 绑定的详细规划** - PyO3 + nautilus_trader 包

---

## 2. 风险矩阵（采纳自 PM-B，补充自 PM-A）

| 风险项 | 影响 | 概率 | 优先级 | 缓解策略 | 验证阶段 |
|--------|------|------|--------|----------|----------|
| **R1**: FFI 签名不工作 | 致命 | 中 | **P0** | Phase 0 PoC | Day 1 |
| **R2**: API 与文档不一致 | 高 | 中 | **P1** | 先测试公开 API | Day 2-3 |
| **R3**: Nonce 并发问题 | 高 | 中 | **P1** | AtomicU64 + rollback | Day 4-5 |
| **R4**: 动态价格精度错误 | 中 | 中 | **P2** | 完整单元测试 | Day 2 |
| **R5**: WebSocket 重连状态丢失 | 中 | 高 | **P2** | 状态恢复机制 | Day 3-4 |
| **R6**: 429 限流 | 低 | 中 | **P3** | 市场缓存 + 指数退避 | Day 3 |

---

## 3. 整合后的开发阶段

### 时间线概览

```
┌────────────────────────────────────────────────────────────────────────┐
│  Day 1: Phase 0 - FFI PoC [阻塞点]                                      │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • lighter-go 编译验证                                            │   │
│  │ • Rust FFI 链接测试                                              │   │
│  │ • Testnet 签名验证                                               │   │
│  │ • 决策点: FFI vs 纯 Rust vs subprocess                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
├────────────────────────────────────────────────────────────────────────┤
│  Day 2-3: Phase 1 - 基础架构 + 公开 API [并行]                          │
│  ┌──────────────────────┐  ┌──────────────────────┐                    │
│  │ Dev A: 基础架构       │  │ Dev B: HTTP 公开 API  │                    │
│  │ • config (Public)    │  │ • get_order_books    │                    │
│  │ • error.rs           │  │ • get_trades         │                    │
│  │ • common/            │  │ • MarketCache        │                    │
│  │ • parse.rs (精度)    │  │ • 单元测试           │                    │
│  └──────────────────────┘  └──────────────────────┘                    │
├────────────────────────────────────────────────────────────────────────┤
│  Day 4-5: Phase 2 - 签名集成 + WebSocket [并行]                         │
│  ┌──────────────────────┐  ┌──────────────────────┐                    │
│  │ Dev A: 签名模块       │  │ Dev B: WebSocket      │                    │
│  │ • NonceManager       │  │ • 公共频道订阅        │                    │
│  │ • LighterSigner      │  │ • 应用层心跳          │                    │
│  │ • HTTP 私有 API      │  │ • 消息解析            │                    │
│  │ • 签名端到端测试     │  │ • 重连机制            │                    │
│  └──────────────────────┘  └──────────────────────┘                    │
├────────────────────────────────────────────────────────────────────────┤
│  Day 6-7: Phase 3 - NautilusTrader 集成                                │
│  ┌──────────────────────┐  ┌──────────────────────┐                    │
│  │ Dev A: DataClient    │  │ Dev B: ExecutionClient│                    │
│  │ • subscribe_*        │  │ • submit_order        │                    │
│  │ • 数据流处理         │  │ • 双通道确认          │                    │
│  └──────────────────────┘  └──────────────────────┘                    │
├────────────────────────────────────────────────────────────────────────┤
│  Day 8-9: Phase 4 - Python 绑定 + 集成测试                              │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • PyO3 绑定                                                      │   │
│  │ • nautilus_trader/adapters/lighter/                              │   │
│  │ • Testnet 端到端测试                                             │   │
│  └─────────────────────────────────────────────────────────────────┘   │
├────────────────────────────────────────────────────────────────────────┤
│  Day 10-12: Phase 5 - 测试完善 + 文档                                   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ • 边界测试                                                       │   │
│  │ • 错误恢复测试                                                   │   │
│  │ • 文档编写                                                       │   │
│  │ • Code Review                                                   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 0: FFI PoC 验证 (Day 1) ⚠️ 阻塞点

**来源**: PM-B（新增）
**目标**: 验证 FFI 签名方案可行性

#### 验证清单
```
□ lighter-go 能否在当前环境编译 (Linux/macOS)
□ Rust FFI 能否正确链接 .so/.dylib
□ 签名结果能否被 Lighter testnet 接受
□ 与 NautilusTrader 构建系统 (maturin/PyO3) 兼容
```

#### 决策树
```
FFI PoC 结果
    │
    ├─► 成功 ──► 继续 Phase 1
    │
    └─► 失败
         │
         ├─► 纯 Rust 可行 ──► 调整 Phase 2 工期 (+2天)
         │
         └─► 不可行 ──► subprocess 调用 Go 二进制
                       ──► 或寻求 Lighter 团队支持
```

#### 交付物
- `signing/ffi_poc.rs` - FFI 绑定 PoC
- `tests/test_signing_poc.rs` - 签名验证测试
- `DECISION.md` - 技术决策文档

---

### Phase 1: 基础架构 + 公开 API (Day 2-3)

**并行工作流**:

#### Dev A: 基础架构
```
src/
├── lib.rs
├── config.rs          # Public/Private 分离
├── error.rs           # thiserror 统一错误
└── common/
    ├── mod.rs
    ├── enums.rs
    ├── types.rs
    ├── urls.rs
    ├── consts.rs
    └── parse.rs       # 动态价格精度
```

#### Dev B: HTTP 公开 API
```
src/http/
├── mod.rs
├── transport.rs       # HttpTransport trait (便于 mock)
├── client.rs          # LighterHttpClient
├── models.rs
└── cache.rs           # MarketCache (DashMap)
```

#### 配置设计（采纳 PM-B）
```rust
/// 公开 API 配置 (无需凭证)
#[derive(Clone, Debug)]
pub struct LighterPublicConfig {
    pub chain_id: ChainId,
    pub http_base_url: Option<String>,
    pub ws_base_url: Option<String>,
    pub http_timeout_secs: u64,
    pub ws_ping_interval_secs: u64,  // 默认 60s
}

/// 私有 API 配置 (继承公开配置)
#[derive(Clone, Debug)]
pub struct LighterPrivateConfig {
    pub public: LighterPublicConfig,
    pub private_key: String,
    pub api_key_index: u8,         // 2-254
    pub account_index: u32,
}
```

#### HttpTransport trait（采纳 PM-B）
```rust
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T>;
    async fn post<T: DeserializeOwned, B: Serialize + Send + Sync>(
        &self, path: &str, body: &B
    ) -> Result<T>;
}

// 生产实现
pub struct ReqwestTransport { /* ... */ }

// 测试实现
pub struct MockTransport {
    responses: HashMap<String, serde_json::Value>,
}
```

---

### Phase 2: 签名集成 + WebSocket (Day 4-5)

#### Dev A: 签名模块
```
src/signing/
├── mod.rs
├── nonce.rs           # NonceManager (AtomicU64 + rollback)
├── signer.rs          # LighterSigner trait + FFI 实现
├── types.rs           # SignedTransaction
└── ffi.rs             # FFI 绑定
```

#### Nonce 管理（整合两方案）
```rust
pub struct NonceManager {
    nonce: AtomicU64,
    account_index: u32,
    api_key_index: u8,
}

impl NonceManager {
    pub fn next(&self) -> u64 {
        self.nonce.fetch_add(1, Ordering::SeqCst)
    }

    /// PM-A: 签名失败时回滚
    pub fn rollback(&self) {
        self.nonce.fetch_sub(1, Ordering::SeqCst);
    }

    /// PM-B: 启动时从 API 同步
    pub async fn sync_from_api(&self, http: &LighterHttpClient) -> Result<()> {
        let next = http.get_next_nonce(self.account_index, self.api_key_index).await?;
        self.nonce.store(next, Ordering::SeqCst);
        Ok(())
    }
}
```

#### Dev B: WebSocket
```
src/websocket/
├── mod.rs
├── client.rs          # LighterWebSocketClient
├── messages.rs        # WsMessage 枚举
├── handler.rs         # 消息处理
└── reconnect.rs       # 重连 + 状态恢复
```

#### 应用层心跳（两方案一致）
```rust
impl LighterWebSocketClient {
    async fn handle_message(&mut self, msg: &str) -> Result<Option<WsEvent>> {
        let parsed: WsMessage = serde_json::from_str(msg)?;

        match parsed {
            WsMessage::Ping => {
                // ⚠️ 必须响应应用层 pong (120s 超时)
                self.send(r#"{"type": "pong"}"#).await?;
                Ok(None)
            }
            // ...
        }
    }
}
```

---

### Phase 3: NautilusTrader 集成 (Day 6-7)

#### Dev A: DataClient
```rust
pub struct LighterDataClient {
    core: DataClientCore,
    config: LighterPublicConfig,
    http_client: LighterHttpClient,
    ws_client: LighterWebSocketClient,
    market_cache: Arc<MarketCache>,
}

impl DataClient for LighterDataClient {
    fn subscribe_order_book_deltas(&self, instrument_id: InstrumentId) -> Result<()>;
    fn subscribe_trades(&self, instrument_id: InstrumentId) -> Result<()>;
    fn subscribe_quote_ticks(&self, instrument_id: InstrumentId) -> Result<()>;
}
```

#### Dev B: ExecutionClient + 双通道确认
```rust
pub struct LighterExecutionClient {
    core: ExecutionClientCore,
    config: LighterPrivateConfig,
    http_client: LighterHttpClient,
    ws_client: LighterWebSocketClient,
    signer: Arc<dyn LighterSigner>,
    nonce_manager: NonceManager,
    order_state: OrderStateManager,  // PM-B 的设计
}

/// 双通道订单状态管理（采纳 PM-B）
pub struct OrderStateManager {
    pending_orders: DashMap<ClientOrderId, PendingOrder>,
}

impl OrderStateManager {
    /// REST 响应到达
    pub fn on_rest_response(&self, client_order_id: &ClientOrderId, venue_order_id: VenueOrderId);

    /// WebSocket 消息到达
    pub fn on_ws_order_update(&self, update: &WsOrderUpdate);

    /// PM-A: 5分钟 GC 清理超时订单
    pub fn gc_stale_orders(&self, timeout: Duration);
}
```

---

### Phase 4: Python 绑定 + 集成测试 (Day 8-9)

#### Rust PyO3 绑定
```
src/python/
├── mod.rs
├── config.rs
├── enums.rs
├── http.rs
└── websocket.rs
```

#### Python 包
```
nautilus_trader/adapters/lighter/
├── __init__.py
├── config.py          # msgspec 配置类
├── factories.py       # LiveDataClientFactory, LiveExecClientFactory
├── data.py
├── execution.py
└── providers.py
```

---

### Phase 5: 测试完善 + 文档 (Day 10-12)

#### 测试结构（采纳 PM-B）
```
tests/
├── fixtures/
│   ├── order_books.json
│   ├── trades.json
│   └── ws_messages.json
├── unit/
│   ├── test_parse.rs        # 价格精度
│   ├── test_nonce.rs        # Nonce 管理
│   └── test_signing.rs      # 签名验证
├── integration/
│   ├── test_http_public.rs
│   ├── test_http_private.rs
│   └── test_websocket.rs
└── e2e/
    └── test_testnet.rs      # Testnet 端到端
```

#### 文档
- `README.md` - 使用指南
- `CLAUDE.md` - AI 上下文
- `docs/integrations/lighter.md` - 官方文档

---

## 4. 模块依赖图（采纳 PM-B）

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
│ (transport) │  │ (reconnect) │  │             │
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

---

## 5. 每日同步点（采纳 PM-B）

| Day | 里程碑 | 决策点 |
|-----|--------|--------|
| Day 1 EOD | Phase 0 完成 | FFI 方案是否可行？ |
| Day 3 EOD | Phase 1 完成 | 公开 API 可测试 |
| Day 5 EOD | Phase 2 完成 | 签名端到端验证 |
| Day 7 EOD | Phase 3 完成 | NautilusTrader 集成可测试 |
| Day 9 EOD | Phase 4 完成 | Python 绑定可用 |
| Day 12 EOD | Phase 5 完成 | 可发布 |

---

## 6. 可复用代码（来自 hftbacktest）

| 模块 | 复用程度 | 说明 |
|------|----------|------|
| `ffi.rs` | 直接复用 | FFI 绑定结构 |
| `lighter-go.so` | 直接复用 | 预编译签名库 |
| `msg.rs` | 参考 | 消息类型需适配 |
| `ordermanager.rs` | 参考 | 双通道确认逻辑 |
| `nonce.rs` | 参考 | Nonce 管理模式 |

---

## 7. 关键技术要点汇总

### 7.1 动态价格精度（PM-A 详细 + PM-B 缓存）
```rust
// 每个市场精度不同，必须从 MarketCache 获取
pub fn price_to_int(price: f64, price_decimals: u8) -> u64 {
    (price * 10f64.powi(price_decimals as i32)).round() as u64
}

// 使用 DashMap 实现线程安全缓存
pub struct MarketCache {
    markets: DashMap<u32, MarketInfo>,
}
```

### 7.2 Nonce 管理（整合）
```rust
impl NonceManager {
    pub fn next(&self) -> u64;           // 获取下一个
    pub fn rollback(&self);              // PM-A: 签名失败回滚
    pub async fn sync_from_api(&self);   // PM-B: 启动时同步
}
```

### 7.3 双通道确认（整合）
```rust
impl OrderStateManager {
    pub fn on_rest_response(&self, ...);   // REST 确认
    pub fn on_ws_order_update(&self, ...); // WS 确认
    fn try_emit_accepted(&self, ...);      // 两者都确认才发事件
    pub fn gc_stale_orders(&self, ...);    // PM-A: 5分钟清理
}
```

### 7.4 应用层心跳（两方案一致）
- 使用 JSON `{"type": "pong"}` 响应
- 120 秒超时
- 禁用 WebSocket 协议级 ping

---

## 8. 最终工期估算

| 阶段 | 工作日 | 并行 | 人力 |
|------|--------|------|------|
| Phase 0: FFI PoC | 1 | - | 1人 |
| Phase 1: 基础+公开API | 2 | ✓ | 2人 |
| Phase 2: 签名+WS | 2 | ✓ | 2人 |
| Phase 3: NT 集成 | 2 | ✓ | 2人 |
| Phase 4: Python+测试 | 2 | ✓ | 2人 |
| Phase 5: 完善+文档 | 3 | - | 1-2人 |
| **总计** | **12** | | |

**乐观估计**: 10 工作日
**保守估计**: 12 工作日
**风险缓冲**: +2 工作日（如 FFI 方案需调整）

---

## 9. 下一步行动

1. **立即启动 Phase 0** - FFI PoC 验证（阻塞点）
2. **准备 Phase 1 并行工作** - 基础架构 + 公开 API
3. **建立每日同步机制** - 根据上面的里程碑检查进度

---

**文档版本**: v1.0 (整合版)
**最后更新**: 2025-12-14
**整合者**: PM-A + PM-B 协作
