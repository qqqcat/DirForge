# DirOtter 工程改进计划与实施计划（2026-04-03）

## 1. 背景

本轮复审后的结论是：

- Workspace 分层方向正确，`core / scan / platform / actions / cache / report` 已具备可维护基础。
- 项目尚未达到“高质量 Rust 代码”标准，主要短板集中在：
  - `dirotter-ui` 单文件/单对象膨胀
  - 扫描快照链路的重复全量计算
  - 扫描事件流中的字符串复制偏多
  - 工程质量门槛未完全收口到 `clippy -D warnings`
- 当前最合适的策略不是立刻做大爆炸式重写，而是按“先降风险、再提效率、最后重构 UI 架构”的顺序推进。

## 2. 改进计划

### 2.1 目标

1. 把代码质量从“中上但有技术债”提升到“可持续迭代的高质量工程实现”。
2. 让 Rust 优势从“类型安全和并发安全”继续延伸到“增量算法、少拷贝数据流和更清晰的模块边界”。
3. 在不中断现有主链路的前提下，控制 UI 膨胀、扫描成本和工程漂移。

### 2.2 改进原则

- 优先处理会持续放大成本的问题：单文件 UI、重复全量计算、事件复制。
- 优先做低风险高收益修改：静态检查清零、明显实现错误修正、文档和代码对齐。
- 避免一次性大规模重写；所有中大型重构都拆成可验证、可回退的小阶段。
- 所有阶段都绑定工程验证：`fmt + build/check + test + clippy`。

### 2.3 重点改进主题

#### A. UI 架构降耦

目标：

- 拆解 `dirotter-ui` 当前的 God File / God Object。
- 将状态、调度、分析、页面渲染和组件样式分离。

范围：

- `DirOtterNativeApp`
- 页面函数 `ui_dashboard / ui_current_scan / ui_treemap / ui_diagnostics / ui_settings`
- 扫描/删除/内存释放的 relay 与 controller
- cleanup analysis 与 diagnostics 逻辑

完成标准：

- `crates/dirotter-ui/src/lib.rs` 降到仅保留 app 装配和少量入口逻辑。
- 页面与业务规则分离到独立模块。
- 关键行为存在 UI 回归测试。

#### B. 扫描链路增量化

目标：

- 减少快照期的重复全量计算。
- 把扫描完成前的 UI 刷新成本限制在稳定可控范围内。

范围：

- `NodeStore::rollup`
- `top_n_largest_files / largest_dirs`
- `Aggregator::make_snapshot_data`
- 实时扫描的 summary / ranking 生成策略

完成标准：

- 快照阶段不再每次都对全树做完整 `rollup + full heap top-k`。
- 引入 dirty-ancestor / incremental ranking 或等效增量机制。
- 增加性能基准或阈值回归。

#### C. 扫描事件流少拷贝化

目标：

- 降低 walker -> aggregator -> publisher -> UI 之间的字符串分配和复制。

范围：

- `EntryEvent`
- `BatchEntry`
- `SnapshotView`
- 相关 path/name 生命周期管理

完成标准：

- 优先传递 interned id、`NodeId` 或共享字符串，而不是层层 owned `String`。
- UI 只在最终显示前做必要字符串物化。

#### D. 缓存与持久化稳态化

目标：

- 提升快照写入的一致性和可恢复性。
- 减少每次落盘的同步重操作。

范围：

- `CacheStore::save_snapshot`
- 历史/错误/审计写入
- 手动诊断导出流程

完成标准：

- 快照保存具备事务性，避免“删旧失败新写也失败”造成空窗。
- WAL checkpoint 从默认热路径退出，转入维护动作或空闲动作。

#### E. 工程质量门槛收口

目标：

- 把“能跑”提升到“工具链默认不过就不能合入”。

范围：

- `cargo fmt`
- `cargo build` / `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 文档同步规则

完成标准：

- 默认分支保持 clippy clean。
- 新一轮架构调整必须更新计划、进展和综合评估文档。

## 3. 实施计划

### Phase 0：立即收口当前问题

目标：

- 修掉已经确认的静态检查失败和低成本实现问题。

任务：

1. 修复 `clippy -D warnings` 当前失败项。
2. 修正明显错误或误导实现：
   - 无效/误导的 telemetry 采样
   - 粗糙的路径类型判断
3. 完成一次全量构建、测试、静态检查。
4. 回写评估、计划和进展文档。

退出条件：

- `cargo fmt --all`
- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

### Phase 1：UI 模块拆分

目标：

- 先拆“最容易独立”的部分，避免直接切主循环。

任务：

1. 抽离 `cleanup_analysis` 到独立模块。
2. 抽离 `diagnostics` 和导出逻辑。
3. 抽离 `scan_controller / delete_controller / memory_controller`。
4. 抽离 `pages/dashboard.rs`、`pages/current_scan.rs`、`pages/treemap.rs`。

实施方式：

- 每次只搬一类职责，不做顺手大改。
- 保持 `DirOtterNativeApp` 对外行为不变。
- 每完成一个模块就补最小回归测试。

退出条件：

- `lib.rs` 大幅缩短，页面和业务规则不再混在一起。

### Phase 2：扫描快照增量化

目标：

- 解决当前扫描期最主要的潜在性能热点。

任务：

1. 让 dirty 标记真正参与祖先路径增量更新。
2. 将 top-k 计算从“全量建堆”改为“固定容量候选结构”。
3. 将 summary / ranking 的更新策略从 snapshot-time 全量计算改为 entry-time 增量维护。
4. 为 sample tree / massive tree 增加快照耗时阈值测试。

退出条件：

- 扫描快照成本与节点总量的耦合显著下降。

### Phase 3：消息与数据结构瘦身

目标：

- 用 Rust 的共享所有权和轻量数据传递替换目前的重复字符串拥有权模型。

任务：

1. 重新审视 `EntryEvent / BatchEntry / SnapshotView` 的字段。
2. 把 path/name 从 owned string 改为共享数据或延迟解析。
3. 确保 UI 只拿到它真正需要的展示数据。
4. 评估 `NodeStore` 的索引结构是否继续压缩。

退出条件：

- 消息链路中的大对象复制显著减少。

### Phase 4：缓存和持久化收口

目标：

- 把持久化从“能存”提升为“稳态可维护”。

任务：

1. 为快照写入加入事务边界。
2. 把 checkpoint 从热路径移除。
3. 梳理手动快照、历史、错误导出的职责。
4. 为失败恢复和旧数据兼容补回归测试。

退出条件：

- 快照落库既稳定又不会干扰主流程。

### Phase 5：发布门槛固化

目标：

- 让未来迭代不再轻易退回“先堆功能后补质量”。

任务：

1. CI 默认执行 `fmt / build / test / clippy`。
2. 建立文档更新检查清单。
3. 为关键页面和关键链路补最小回归矩阵。

退出条件：

- 质量门槛前置，而不是靠事后评估兜底。

## 4. 优先级

### P0

- clippy 清零
- 当前错误实现修复
- 改进/实施计划文档落地

### P1

- UI 模块拆分
- 扫描快照增量化设计与第一阶段实现

### P2

- 事件流少拷贝化
- 缓存事务与 checkpoint 策略重做

### P3

- CI 与视觉/性能回归体系补齐

## 5. 本轮已立即执行的动作

- 已将本计划文档落地。
- 已对当前已确认的静态检查问题和低成本实现问题做修复。
- 已准备重新执行构建、测试和 clippy 校验，并把结果同步到相关文档。

## 6. 阶段进展

### 2026-04-03 Phase 1 Start

- 已启动 UI 模块拆分。
- 第一块已抽离为独立模块：
  - `crates/dirotter-ui/src/cleanup.rs`
- 第二块已抽离为独立模块：
  - `crates/dirotter-ui/src/controller.rs`
- 当前策略是：
  - 先抽“纯规则 / 纯分析”逻辑
  - 再抽“后台线程 / relay / controller”逻辑
  - 保持 `DirOtterNativeApp` 的对外行为和现有测试不变
  - 再继续拆 controller 和页面模块

### 当前状态

- Phase 0：完成
- Phase 1：进行中
- Phase 2-5：待执行

### 2026-04-03 Phase 1 Update

- 页面层拆分已继续向前推进：
  - `crates/dirotter-ui/src/cleanup.rs`
  - `crates/dirotter-ui/src/controller.rs`
  - `crates/dirotter-ui/src/dashboard.rs`
  - `crates/dirotter-ui/src/dashboard_impl.rs`
- `dashboard` 相关方法已从 `lib.rs` 移出：
  - `ui_dashboard`
  - `render_overview_hero`
  - `render_live_overview_hero`
  - `render_overview_metrics_strip`
  - `render_scan_target_card`
- 这说明 Phase 1 的页面拆分路径已经验证可行，下一步应继续沿 `current_scan / treemap / diagnostics` 逐页拆分，而不是改回“大文件继续加方法”。
- 本轮执行中出现过一次模块文案编码污染，已在同轮修复并重新收口到 `src/` 内部实现文件，没有留下额外运行时问题。
- 当前验证结果：
  - `cargo fmt --all`
  - `cargo test -p dirotter-ui`
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
