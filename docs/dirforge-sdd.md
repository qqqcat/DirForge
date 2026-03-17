# DirForge 系统设计说明书（SDD，2026-03-16）

## 1. 目标与范围

> 说明：本文件聚焦系统设计与架构目标；项目阶段、风险与优先级请参考 `docs/dirforge-comprehensive-assessment.md`。

DirForge 是一款本地磁盘分析器工程化原型，目标是稳定“扫描 → 聚合 → 展示 → 去重候选 → 操作计划 → 报告/诊断”主链路，并持续补齐平台能力、可观测性与执行安全。

## 2. 当前实现状态（与代码对齐）

- 已实现：扫描事件流（`Progress/Batch/Snapshot/Finished`）、取消扫描、错误分类上报。
- 已实现：`NodeStore` 扁平结构、`rollup` 聚合、Top-N 查询。
- 已实现：`eframe/egui` UI、历史快照缓存、可滚动排行榜、基础 treemap 与列表展示。
- 已实现：默认根路径选择、盘符快捷扫描、Inspector 内真实删除（回收站/永久删除确认）、删除忙碌层、目录上下文文件榜单、文本/CSV/诊断导出。
- 已实现：系统快照与指标描述、路径可达性评估、诊断归档。

> 当前成熟度：**Production Readiness（工程化验证后期）**。

技术基线：Rust + egui/eframe + walkdir + rusqlite + blake3 + tracing。

## 3. 总体架构

核心原则：解耦“发现文件、元数据采集、聚合、去重、缓存、UI 渲染”。

```text
Desktop UI (egui/eframe)
  -> UI State / ViewModel
  -> Event Bus (bounded)
  -> { Scan Engine | Aggregate Engine | Dup Engine | Cache Engine }
  -> Storage + OS Integration
```

### 模块职责

- **Scan Engine**：目录遍历与轻元数据采集，产出进度/批次/快照/完成事件。
- **Aggregate Engine**：接收 walker 事件并构建目录树，处理乱序父子到达。
- **Dup Engine**：独立去重流水线，避免阻塞基础扫描。
- **Cache Engine**：快照持久化、历史/设置/审计管理。
- **UI**：消费事件流、支持盘符快捷扫描、执行删除后的局部重建，并进行可视化。
- **Action UX**：将长耗时删除从 UI 主线程中剥离，并通过前台忙碌层、状态栏提示和页面锁定维持交互一致性。

## 4. 当前线程模型（现状）

当前实现已采用并发流水线，而非单线程扫描：

1. **UI 主线程**：输入、渲染、命令分发、事件消费合并。
2. **扫描 worker 池**：并发枚举目录与读取 metadata。
3. **聚合线程**：接收 walker 事件并维护树结构聚合状态。
4. **发布链路**：通过有界通道向 UI 发送批次/快照事件，控制背压。

设计收益：

- 提升吞吐并降低主循环阻塞风险。
- 在慢消费场景下抑制事件堆积与内存峰值。

## 5. 扫描流水线

```text
Stage 0 Root Planning
Stage 1 Concurrent Directory Enumeration
Stage 2 Metadata Acquisition
Stage 3 Aggregation / Parent-Child Reconciliation
Stage 4 Rollup & Top-N Extraction
Stage 5 Snapshot Delta Publish
Stage 6 Finished Publish
```

设计要点：

- 事件批量化发出，避免逐文件驱动 UI。
- 快照以 delta + 视图数据为主，降低大对象复制压力。
- 并发乱序输入下通过 pending 缓冲保证建树正确性。

## 6. 缓存架构

- **L1 内存热缓存**：当前可见列表/treemap 数据。
- **L2 会话缓存**：当前扫描状态与待处理事件。
- **L3 持久化缓存**：SQLite 历史快照、设置、审计。

快照 payload 采用 `zstd+bincode`，并保留历史 JSON 兼容读取。

## 7. 重复文件检测架构

四阶段去重：

1. `size -> files[]` 初筛
2. partial hash 重分桶
3. strong hash 最终确认
4. 结果整形（keeper 推荐、风险分级、可回收空间）

## 8. UI 刷新策略

- 快照合并（coalescing）
- 有界队列（`VecDeque`）+ 超限受控丢弃
- 大列表虚拟化
- 删除成功后的局部 `NodeStore` 重建与重新 `rollup`
- 选中文件夹后的上下文文件榜单切换
- 交互优先（拖拽/滚动时降低后台合并频率）

## 9. 平台能力现状

- Explorer 打开/选择
- 回收站删除入口
- Windows 回收站可见性二次校验
- 永久删除执行入口
- reparse point/symlink 检测
- 卷信息查询
- `assess_path_access(path)` 前置路径校验与边界判断
- 统一平台错误模型

## 10. 后续里程碑建议

1. 执行安全：真实删除路径在跨平台边界场景的覆盖加深。
2. 稳定性：长跑压测 + 资源曲线观测（CPU/RSS/事件滞后）。
3. 可观测性：关键链路指标与诊断导出进一步标准化。
4. UI 体验：错误恢复、重试、结果可追溯流程优化。
