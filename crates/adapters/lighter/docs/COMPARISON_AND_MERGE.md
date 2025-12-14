# Lighter Connector 开发计划对比与整合

> 日期: 2025-12-14
> 参与者: PM-A, PM-B

---

## 1. 方案对比

| 维度 | PM-A 方案 | PM-B 方案 | 评估 |
|------|-----------|-----------|------|
| **方法论** | 功能驱动、阶段分解 | 风险驱动、快速验证 | 互补 |
| **时间估算** | 20 工作日 (4周) | ~10 工作日 (2周) | A更保守 |
| **Phase 划分** | 9 个 Phase | 4 个 Phase + 验证 | A更细 |
| **风险评估** | 有风险表 | **详细风险矩阵** | B更强 |
| **代码示例** | **非常详细** | 关键点示例 | A更强 |
| **测试策略** | 测试文件列表 | **Mock策略+金字塔** | B更强 |
| **并行策略** | Week维度 | Stream维度 | 各有优势 |

---

## 2. 各自亮点

### PM-A 亮点 ✨
1. **完整的代码示例** - 几乎可以直接复制使用
2. **详细的文件结构** - 每个模块都有明确路径
3. **检查清单** - 每个Phase有完成标准
4. **Mermaid依赖图** - 清晰的阶段依赖
5. **Python绑定细节** - 包含msgspec配置示例

### PM-B 亮点 ✨
1. **风险优先** - Phase 0 PoC 阻塞验证
2. **风险矩阵** - 量化风险影响和概率
3. **Mock 测试策略** - `HttpTransport` trait 设计
4. **模块依赖图** - 代码级别的依赖关系
5. **hftbacktest对比** - 明确可复用代码

---

## 3. 🚨 重大技术发现 (2025-12-14 更新)

### Lighter 签名算法研究结果

经过深入研究，发现 **Lighter 不使用 EVM 标准签名 (EIP-712/ECDSA)**，而是使用：

| 组件 | Lighter 协议 | Ethereum 标准 |
|------|-------------|---------------|
| **椭圆曲线** | ECgFp5 (Goldilocks Fp5) | secp256k1 |
| **哈希函数** | Poseidon2 | Keccak256 |
| **签名算法** | Schnorr | ECDSA |
| **私钥大小** | 40 字节 | 32 字节 |
| **ZK 友好** | ✅ 是 (Plonky2 STARK) | ❌ 否 |

### ✅ 纯 Rust 方案可行！

**好消息**: 已有现成的 Rust crate 可用：

```toml
[dependencies]
goldilocks-crypto = "0.1.1"  # ECgFp5 + Schnorr 签名
poseidon-hash = "0.1.3"       # Poseidon2 哈希
```

**参考实现**: [github.com/Bvvvp009/lighter-rust](https://github.com/Bvvvp009/lighter-rust)

### 技术决策变更

| 原计划 | 新决策 | 理由 |
|--------|--------|------|
| FFI 调用 lighter-go | **纯 Rust 实现** | 无外部依赖、跨平台、可维护 |
| 复用 hftbacktest .so | 使用 crates.io 库 | 官方发布、有维护 |

---

## 4. 关键共识点 ✅

两个方案**一致认同**的关键点（签名方案已更新）：

| 共识 | 说明 |
|------|------|
| ~~FFI 签名优先~~ | **新决策**: 纯 Rust 实现 (goldilocks-crypto) |
| 动态价格精度 | `price_decimals` 是关键特性，需缓存 |
| 双通道确认 | REST + WebSocket 协调机制 |
| Nonce 管理 | Per-API-Key + AtomicU64 + rollback |
| 应用层心跳 | JSON ping/pong，非帧级别 |
| 市场发现 | 必须用 `order_books()` API |

---

## 5. 建议整合方案（已更新）

### 5.1 采用 PM-B 的风险驱动框架（已简化）

```
Phase 0: 签名验证 (Day 1)              ← 验证 goldilocks-crypto 可用
    │
    ├─ 通过 → 继续（预期结果）
    └─ 失败 → 调试或备选方案
         │
Phase 1: 基础架构 (Day 2-3)            ← 采用 PM-A 详细结构
         │
    ┌────┴────┐
    ↓         ↓
Phase 2:   Phase 3:                    ← 并行开发
HTTP       WebSocket
(Day 4-6)  (Day 4-6)
    │         │
    └────┬────┘
         │
Phase 4: 签名模块 (Day 7-8)            ← 纯 Rust 实现
         │
Phase 5: DataClient (Day 9-10)
         │
Phase 6: ExecutionClient (Day 11-12)   ← 节省1天（无 FFI 调试）
         │
Phase 7: Python 绑定 (Day 13-14)
         │
Phase 8: 测试+文档 (Day 15-16)
```

**注**: 由于改用纯 Rust，无需 FFI 调试，总工期缩短至 **16 工作日**

### 5.2 采用 PM-A 的代码模板

直接使用 PM-A 的：
- `config.rs` 结构
- `common/parse.rs` 价格转换
- `http/models.rs` 数据结构
- `websocket/messages.rs` 消息类型
- Python 绑定结构

### 5.3 采用 PM-B 的测试策略

```rust
// 采用 HttpTransport trait 便于 Mock
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T>;
    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T>;
}

// 生产实现
pub struct ReqwestTransport { /* ... */ }

// 测试实现
pub struct MockTransport { /* ... */ }
```

### 5.4 合并检查清单（已更新）

每个 Phase 增加 PM-B 的决策点：

```
Phase 0 完成标准:
  □ goldilocks-crypto 编译成功
  □ 签名函数调用成功
  □ （可选）与 lighter-rust 测试向量对比
  □ 技术决策文档更新

  决策点: 纯 Rust 可行? → 是: 继续 / 否: 回退 FFI 方案
```

---

## 6. 整合时间表（已更新）

| 天数 | 任务 | 方法 | 产出 |
|------|------|------|------|
| D1 | Phase 0: 签名 PoC | PM-B | 验证 goldilocks-crypto |
| D2-3 | Phase 1: 基础架构 | PM-A | 目录+配置+常量 |
| D4-6 | Phase 2+3: HTTP+WS (并行) | PM-A+B | 公开API |
| D7-8 | Phase 4: 签名模块 | **纯 Rust** | 签名模块 |
| D9-10 | Phase 5: DataClient | PM-A | 数据客户端 |
| D11-12 | Phase 6: ExecutionClient | PM-A | 执行客户端 |
| D13-14 | Phase 7: Python 绑定 | PM-A | PyO3绑定 |
| D15-16 | Phase 8: 测试+文档 | PM-B | Mock测试 |

**总计: 16 工作日**（比原计划节省 1 天，无需 FFI 调试）

---

## 7. 立即行动项（已更新）

### 今天可以并行启动:

| 任务 | 负责 | 产出 |
|------|------|------|
| **创建目录结构** | - | `crates/adapters/lighter/` |
| **Cargo.toml** | - | 添加 goldilocks-crypto 依赖 |
| **签名 PoC** | - | 验证 goldilocks-crypto 可用 |
| **common/ 模块** | - | 常量+枚举+类型 |

---

## 8. 待讨论问题（已解决部分）

1. ~~**时间估算**: 采用 17 天还是 20 天？~~ → **已解决: 16 天（纯 Rust）**
2. ~~**PoC 失败备选**: 如果 FFI 失败，纯 Rust 还是 subprocess？~~ → **已解决: 直接用纯 Rust**
3. **测试覆盖率目标**: 80%? 90%?
4. **Python 绑定优先级**: 是否可以延后到 Phase 8？

---

## 9. 结论（已更新）

**推荐整合方案**:
- 框架: PM-B 风险驱动
- 内容: PM-A 详细模板
- **签名: 纯 Rust 实现 (goldilocks-crypto)**
- 测试: PM-B Mock 策略
- 时间: **16 工作日**

**技术栈**:
```toml
[dependencies]
goldilocks-crypto = "0.1.1"   # Schnorr + ECgFp5
poseidon-hash = "0.1.3"       # Poseidon2 哈希
```

**下一步**: 立即开始 Phase 0 签名验证 + Phase 1 基础架构并行。
