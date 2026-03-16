# DirForge 系统设计说明书（SDD）

## 1. 目标与范围

DirForge 是一款 **本地磁盘分析器工程化原型**，当前目标是稳定“扫描 -> 聚合 -> 展示 -> 去重候选 -> 操作计划”的主链路，并在此基础上逐步补齐平台能力、可观测性与执行安全性。

## 当前实现状态（与代码对齐）

- 已实现：批量扫描事件、周期性 snapshot delta 事件、扫描 profile、取消扫描。
- 已实现：`NodeStore` 扁平结构、`rollup` 聚合、Top-N 查询。
- 已实现：`eframe/egui` UI 壳、历史快照缓存、基础 treemap 与列表展示。
- 已实现：去重候选、删除计划与模拟执行、文本报告与诊断导出。
- 进行中：平台深度能力（回收站、卷信息、Explorer 选择）、可观测指标、扫描性能优化。

> 当前成熟度：pre-alpha（可运行原型，非生产级）。

技术基线（当前依赖）：

- Rust
- `egui/eframe`（原生桌面 UI）
- `walkdir`（目录遍历）
- `rusqlite`（缓存）
- `blake3`（哈希）
- `tracing`（可观测初始化）

## 2. 总体架构

核心原则：解耦“发现文件、元数据采集、聚合、去重、缓存、UI 渲染”。

```text
Desktop UI (egui/eframe)
  -> UI State / ViewModel
  -> Event & Snapshot Bus
  -> { Scan Engine | Aggregate Engine | Dup Engine | Cache Engine }
  -> Storage + OS Integration
```

### 模块职责

- **Scan Engine**：遍历与轻元数据采集，产出 batch/progress/snapshot delta。
- **Aggregate Engine**：目录回卷、Top-N 聚合。
- **Dup Engine**：独立去重流水线，避免阻塞基础扫描。
- **Cache Engine**：快照持久化、历史与设置管理。
- **UI**：消费事件流并进行局部重建。

## 3. 当前线程模型（现状）

当前实现以单后台扫描线程 + UI 主线程为主，具备基础可用性，但尚未落地 IO/CPU 分池。

后续目标：

1. **UI Main Thread**：输入/渲染/命令分发。
2. **IO Scan Pool（有界）**：目录枚举、metadata 读取。
3. **CPU Pool**：聚合、排序、哈希。
4. **Background Service**：缓存落盘、日志与诊断打包。

## 4. 扫描流水线

```text
Stage 0 Root Planning
Stage 1 Directory Enumeration
Stage 2 Metadata Acquisition
Stage 3 Local Aggregation
Stage 4 Tree Rollup
Stage 5 Snapshot Delta Publish
Stage 6 Finished Publish
```

设计要点：

- 事件批量化发出，避免逐文件刷 UI。
- 快照事件只发送 delta，避免频繁克隆整个 `NodeStore`。
- UI 侧按 batch 做局部重建；最终结果由 Finished 事件携带完整 store。

## 5. 缓存架构

三层缓存规划：

- **L1 内存热缓存**：可见列表/treemap 数据。
- **L2 会话缓存**：当前扫描状态、增量事件缓存。
- **L3 持久化缓存**：SQLite 历史快照与设置。

## 6. 重复文件检测架构

四阶段去重：

1. `size -> files[]` 初筛
2. partial hash 重分桶
3. strong hash 最终确认
4. 结果整形（keeper 推荐、风险级别、可回收空间）

## 7. UI 刷新策略

- 快照合并（coalescing）
- 局部失效（扫描列表优先）
- 大列表虚拟化
- 交互优先（拖拽/滚动时降低后台合并频率）

## 8. 关键数据结构

当前使用扁平 `NodeStore`：

```text
NodeId -> Node { parent, kind, size_self, size_subtree, ... }
Vec<Node>
HashMap<Path, NodeId>
```

## 9. 平台能力现状与计划

现状：

- Explorer 打开/选择
- 回收站删除入口
- reparse point/symlink 检测
- 卷信息查询
- 统一平台错误模型

计划：

- Windows 权限与系统目录保护策略
- 稳定文件身份字段
- 更完整的失败恢复与审计记录

## 10. 里程碑建议

1. 扫描性能与数据流：delta 事件、局部重建、压力基准。
2. 平台能力：回收站、卷信息、错误分类一致性。
3. 观测与诊断：吞吐/错误/操作审计指标。
4. 执行安全：预校验、批执行、部分失败策略。
5. 回归质量：边界 fixture、取消/错误/symlink 测试。
