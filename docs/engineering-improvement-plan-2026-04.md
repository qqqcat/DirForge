# DirOtter 工程改进计划与实施计划（2026-04-03）

> 2026-04-09 状态更新：
>
> - Phase 0 已完成
> - Phase 1 已进入“主体完成、继续局部收口”的状态
> - Phase 2 与 Phase 3 的核心目标已基本落地
> - Phase 4 已完成轻量存储、原子写入、临时会话回退与清理的主体实现
> - Phase 5 的基础设施已落地，当前剩余重点转为视觉回归、跨平台边界和正式签名发布

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
- 已完成 `fmt / build / test / clippy` 校验，并将结果同步到相关文档。

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
- Phase 1：主体完成，继续局部收口
- Phase 2：主体完成
- Phase 3：主体完成
- Phase 4：部分完成
- Phase 5：基础完成

### 2026-04-03 Phase 1 Update

- 页面层拆分已继续向前推进：
  - `crates/dirotter-ui/src/cleanup.rs`
  - `crates/dirotter-ui/src/controller.rs`
  - `crates/dirotter-ui/src/dashboard.rs`
  - `crates/dirotter-ui/src/dashboard_impl.rs`
  - `crates/dirotter-ui/src/result_pages.rs`
  - `crates/dirotter-ui/src/settings_pages.rs`
  - `crates/dirotter-ui/src/advanced_pages.rs`
- `dashboard` 相关方法已从 `lib.rs` 移出：
  - `ui_dashboard`
  - `render_overview_hero`
  - `render_live_overview_hero`
  - `render_overview_metrics_strip`
  - `render_scan_target_card`
- `current_scan / treemap` 相关方法也已从 `lib.rs` 移出：
  - `ui_current_scan`
  - `ui_treemap`
- `history / errors / diagnostics / settings` 相关方法也已从 `lib.rs` 移出：
  - `ui_history`
  - `ui_errors`
  - `ui_diagnostics`
  - `ui_settings`
- 这说明 Phase 1 的页面拆分路径已经基本完成验证。下一步更适合转向共享 helper 下沉、状态结构收口，以及扫描快照链路优化，而不是改回“大文件继续加方法”。
- 本轮执行中出现过一次模块文案编码污染，已在同轮修复并重新收口到 `src/` 内部实现文件，没有留下额外运行时问题。
- 当前验证结果：
  - `cargo fmt --all`
  - `cargo test -p dirotter-ui`
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`

### 2026-04-03 Next Layer Update

- 核心扫描链路优化已开始落地：
  - `crates/dirotter-core/src/lib.rs`
  - `crates/dirotter-cache/src/lib.rs`
- 已完成的具体改造：
  - `mark_dirty()` 改为向祖先传播 dirty
  - `rollup()` 改为只重算 dirty 节点
  - `top_n_largest_files()` / `largest_dirs()` 改为固定容量候选堆
  - `save_snapshot()` 改为事务式替换
  - 去掉每次快照保存后的强制 `wal_checkpoint(TRUNCATE)`
- 这意味着计划里“减快照成本”已经不再只是文档项，而是开始进入代码主干。
- 当前这一层的下一步更适合继续处理：
  - `aggregator.make_snapshot_data()` 的 view 组装成本
  - 共享 helper / 状态分组的继续下沉
  - 针对 snapshot 节奏的更明确性能基准

### 2026-04-03 Next Layer Update 2

### 2026-04-09 Validation Update

- 默认无数据库的轻量存储模型已继续收口：
  - 启动阶段会优先使用持久 `settings.json`
  - 若持久目录不可写，则回退到临时会话存储并在设置页明确提示
  - 会话快照临时目录增加退出清理与陈旧目录回收
- 翻译生成链路已补充两项防漂移措施：
  - `scripts/generate_ui_translations.py` 会清洗零宽字符，避免生成文件再次触发 `clippy::invisible_characters`
  - `scripts/build_translation_source.py` 用于重建 `crates/dirotter-ui/src/_translation_source_all.rs`
- 当前验证结果：
  - `cargo fmt --all --check`
  - `cargo build -p dirotter-app`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `target/debug/dirotter-app.exe` 启动 smoke test

### 2026-04-09 Release Gate Update

- Phase 5 中最核心的发布门槛基础设施已落地：
  - 新增 `.github/workflows/ci.yml`
  - 新增 `.github/workflows/release-windows.yml`
  - 新增 `scripts/package-windows.ps1`
  - 新增 `scripts/sign-windows.ps1`
  - 新增 `scripts/install-windows-portable.ps1`
  - 新增 `scripts/uninstall-windows-portable.ps1`
- 当前 CI 默认执行：
  - `template validation`
  - `cargo fmt --all --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo build --release -p dirotter-app`
- 当前 Windows 发布链路默认生成：
  - `DirOtter-windows-x64-<version>-portable.zip`
  - `.sha256.txt`
  - 可选 Authenticode 签名产物
- 之前波动的 benchmark 门禁也已一起收口：
  - massive tree snapshot payload 测试的事件等待改为更稳定的 timeout 常量
  - 性能阈值断言本身保持不变，避免把真实回归放宽掉

- 扫描链路优化已继续深入到 entry-time 聚合维护：
  - `NodeStore::add_node()` 现在会即时维护祖先 `size_subtree / file_count / dir_count`
  - `Aggregator::make_snapshot_data()` 已移除补账式 `rollup()` 依赖
  - `top_files_delta / top_dirs_delta` 改为直接从命中节点导出
- 这一步的意义是把“增量化”从 snapshot-time 推进到 entry-time：
  - snapshot 不再负责为 append-only 扫描补整棵树的账
  - 扫描线程在插入节点时就把账维护到位
- 当前剩余的下一步优先项：
  1. 继续压缩 `EntryEvent / BatchEntry / SnapshotView` 中的 owned string
  2. 评估是否把 live snapshot 的 view payload 再继续收窄
  3. 增加更明确的 snapshot/perf 基线，避免后续回退到“大树一来就重新全算”

### 2026-04-03 Phase 3 Start

- “少拷贝数据流”已经开始进入代码主干，而不再只是计划：
  - `EntryEvent.path / parent_path / name` 已切到共享 `Arc<str>`
  - `BatchEntry.path` 已切到共享 `Arc<str>`
  - `Publisher.frontier` 已切到共享路径队列
  - `Aggregator.pending_by_parent` 已切到共享路径键
- 当前策略是明确分层：
  - 扫描 crate 内部尽量共享
  - UI 边界继续保留 `String`，只在显示前物化
- 这样做的好处是：
  - 不需要一次性改穿整个应用状态层
  - 先把最高频的跨线程热路径成本降下来
  - 继续保持现有 UI 与测试行为稳定

### 2026-04-03 Phase 3 Update

- 共享路径已经继续推进到扫描事件边界：
  - `ScanProgress.current_path` 已改为 `Option<Arc<str>>`
  - `SnapshotView.top_files / top_dirs` 已改为共享路径排行
  - `ScanEvent::Finished` 的 Top-N 排行也已改为共享路径
- 当前分层更清晰：
  - 扫描 crate 内部与事件边界尽量共享
  - UI 接到事件后再统一物化到自身状态
- 这一步的意义在于，实时扫描阶段最频繁的“路径转字符串”热点已经继续后移，不再混在 publisher/snapshot 组装里。

### 2026-04-03 Phase 3 Update 2

- 共享路径已经进一步推进到 UI 持有层：
  - `scan_current_path` 已改为 `Option<Arc<str>>`
  - 实时/完成态 Top-N 排行已改为共享路径状态
  - 只有排行 helper 或页面渲染真正需要文本时才 `to_string()`
- 这样做的价值是：
  - 继续压缩实时扫描期间的瞬时分配
  - 保持 UI 页面调用接口基本稳定
  - 让后续是否继续处理 `ResolvedNode` 变成一个可独立评估的问题，而不是和当前改动耦合

### 2026-04-03 Phase 3 Update 3

- `ResolvedNode` 已继续改为共享字符串结构：
  - `name / path` 使用 `Arc<str>`
  - `SnapshotView.nodes` 直接复用已有共享分配
- 这一步的效果是把实时 snapshot 里最后一块明显的节点级字符串复制也压下去。
- 当前 Phase 3 的剩余重点已经更集中：
  1. 评估页面 helper 中是否还存在不必要的批量 `to_string()`
  2. 决定是否为最终结果页引入更轻的 view model，而不是直接从共享状态即时物化
  3. 增加更明确的快照 payload / 分配基线

### 2026-04-04 Phase 3 Update 4

- 已继续去掉两块明确的冗余 payload：
  - 非 full-tree `SnapshotView` 不再携带变更节点列表，只保留 `changed_node_count`
  - `ScanEvent::Finished` 不再重复携带可由 `store` 重建的 Top-N 排行
- 这一步的意义不是“类型更漂亮”，而是直接减少：
  - 实时 snapshot 的序列化/排队体积
  - 完成态事件的重复数据传输
  - UI relay 层的无效中转数据

### 2026-04-04 Perf Baseline Update

- 性能基线已开始覆盖 snapshot 关键路径，而不再只看“整次扫描多久完成”：
  - 大树扫描下的 snapshot payload 大小阈值
  - `make_snapshot_data(false)` 的本地组装耗时阈值
- 这样做的意义是把最近几轮的优化成果固化下来：
  - 如果 payload 重新膨胀，测试先报
  - 如果 snapshot 组装重新退化，测试先报

### 2026-04-04 Runtime Observability Update

- snapshot 稀疏化现在不再只靠离线阈值测试守护，也开始进入运行时观测：
  - telemetry 新增 `avg/max snapshot changed nodes`
  - telemetry 新增 `avg/max snapshot materialized nodes`
  - telemetry 新增 `avg snapshot ranked items`
  - telemetry 新增 `avg/max snapshot text bytes`
- 当前实现明确避免了一种“为了看 payload 又把 payload 做重”的倒退：
  - 不额外序列化 live snapshot
  - 只统计节点/排行路径文本长度作为低成本估算
- 这一步的目标不是替代基准测试，而是给 diagnostics 一个能直接揭示 payload 回退的运行时锚点。

### 2026-04-04 SnapshotView Type Split

- `SnapshotView` 已进一步从“单结构兼顾 live/debug”收口成显式分层：
  - `LiveSnapshotView`
  - `FullSnapshotView`
  - `SnapshotView::{Live, Full}`
- 当前价值不是表面上的类型美化，而是把运行时边界做实：
  - live 路径只能自然接轻量视图
  - full-tree/debug 路径必须显式走 `Full`
- 这让后续继续压 live payload 时，风险从“靠人记住不要塞 nodes”变成“类型先拦住错误方向”。

### 2026-04-04 UI Selection State Update

- 当前结果树相关 UI 状态已开始从“路径字符串驱动”收口为“`NodeId` 优先”：
  - `SelectedTarget` 携带 `node_id`
  - `TreemapEntry` 携带 `node_id`
  - 新增 `select_node()`，treemap 与 cleanup 候选点击优先走节点 ID
- 这一步暂时没有追求“一次改穿所有 UI 状态”，而是先抓最值钱的当前结果树交互层。
- 这样做的好处是：
  - 降低重复 `path -> NodeId` 回查
  - 给后续继续推进 ID 化留出稳定中间态
  - 保留错误页/外部路径这类非 store-backed 路径的兼容 fallback

### 2026-04-04 UI View-Model Extraction

- `dirotter-ui` 已进一步从“状态文件里顺手拼展示数据”转向显式 view-model 模块：
  - 新增 `src/view_models.rs`
  - 下沉了摘要卡片、扫描健康文案、排行物化和上下文文件榜单
- 这一步的关键价值是职责收口：
  - `DirOtterNativeApp` 更像应用协调器
  - 页面模块继续只负责布局与交互
  - 展示数据整形开始集中到单独模块，便于后续继续压物化点

### 2026-04-04 UI String Hotspot Reduction

- UI 侧最明显的批量字符串热点已继续收口：
  - 实时/完成态排行改用共享 `RankedPath`
  - 上下文文件榜单改用共享 `RankedPath`
  - `live_files` 改为共享路径列表
  - `TreemapEntry.name / path` 改为共享 `Arc<str>`
- 这一步的重要性在于，它把“少拷贝/延迟物化”的策略继续推进到了 UI 展示层，而不是只停在 scan/core 内部。

### 2026-04-04 UI Shared Path State Reduction

- UI 内部最后两块高频路径状态也已继续收口：
  - `CleanupPanelState.selected_paths` 改为 `HashSet<Arc<str>>`
  - `treemap_focus_path` 改为 `Option<Arc<str>>`
- 这一步的价值不是“字段类型更统一”，而是把 cleanup 勾选和 treemap 聚焦这两条仍会频繁 `contains/remove/set-focus` 的路径状态也拉回共享模型。
- 当前状态下，UI 内部共享路径的覆盖面已经从：
  - scan 事件
  - 实时/完成态排行
  - `SelectedTarget / TreemapEntry`
  继续扩展到：
  - cleanup 选择集
  - treemap 当前焦点

### 2026-04-04 Inspector / Confirm View-Model Extraction

- `view_models.rs` 现在已不只处理首页和结果页的榜单/摘要，还继续接管了：
  - Inspector 目标摘要
  - 后台删除任务摘要
  - 永久删除确认摘要
  - cleanup 确认摘要
- 这一步的关键价值是继续把 `DirOtterNativeApp` 从“交互入口 + 展示字符串拼装”收口成“交互入口 + 状态协调”。
- 当前状态下，view-model 模块已经开始覆盖：
  - 概览卡片
  - 扫描健康文案
  - 排行与上下文榜单
  - Inspector / confirm 摘要

### 2026-04-04 Inspector Action-State Extraction

- Inspector 的动作可用性判断也已继续下沉：
  - 打开位置
  - 快速清理
  - 回收站删除
  - 永久删除
  - 系统内存释放
- Explorer 反馈、删除反馈和最近执行摘要的展示文本现在也由 `view_models.rs` 统一整形。
- 这一步的价值在于，Inspector 已经不只是“展示文本下沉”，而是连动作态判断和反馈态判断都开始脱离布局代码，后续调整交互状态时的改动面会更小。

### 2026-04-04 Inspector Memory Status Redesign

- Inspector 底部的 `Workspace Context` 已被系统内存状态卡替换，并继续由 `view_models.rs` 统一整形。
- 这次重做把系统可用内存、内存负载、DirOtter 占用和最近一次释放结果从底部状态栏/布局拼装中收口回 Inspector。
- 到这一步，Inspector 区域的大部分展示整形已经完成下沉，主函数主要只剩：
  - 面板布局
  - 按钮点击后的动作分发
  - 少量窗口级控制流

### 2026-04-04 Cleanup Confirm / Failure Details Refinement

- cleanup 确认窗已从“固定前几项预览”改为“完整目标列表 + 内部滚动”，用户在确认前可以完整复核待处理路径。
- Inspector 最近执行摘要在存在失败项时已改为暴露明确的失败详情入口，失败详情窗集中展示：
  - 完整路径
  - 原始失败原因
  - 针对性的处理建议
- 外层失败 banner 已不再直接暴露首条失败原因，避免在摘要区继续出现被截断的错误文本。
- 这一步的价值是把“批量删除前的可审阅性”和“批量失败后的可追溯性”补齐，而不是继续把关键信息挤在狭窄摘要区里。

### 2026-04-04 Cleanup Details Window Extraction

- cleanup 详情窗也已开始走相同路线：
  - 分类 tabs 改由 `view_models.rs` 整形
  - 统计区和按钮标签/启用态改由 `view_models.rs` 整形
  - item 行的路径、大小、风险/分类标签、unused days 和评分文案改由 `view_models.rs` 整形
- 这一步的价值是把另一个明显偏重的 UI 函数也从“边布局边拼展示文本”拉回“布局 + 状态写回”的模式。
- 当前状态下，cleanup 详情窗里还留在主函数的核心逻辑主要是：
  - 勾选状态写回
  - 切换当前选中项
  - 触发清理/打开位置等动作

### 2026-04-04 Cleanup Details Action Extraction

- cleanup 详情窗的交互控制流也已继续收口，不再依赖多组布尔旗标在窗口尾部串联。
- 当前已经改为：
  - 渲染阶段收集 `CleanupDetailsAction`
  - 渲染后统一分发到 action handler
- 这一步的意义是把 cleanup 详情窗继续从“布局、展示、控制流全混在一起”的函数，推进成更明确的：
  - view-model 负责展示整形
  - window 函数负责渲染与收集动作
  - handler 负责执行动作

### 2026-04-04 Remaining Dialog Action Extraction

- 剩余两类确认窗现在也已采用同样模式：
  - 永久删除确认窗
  - cleanup 确认窗
- 当前都已从“窗口内部直接切 confirmed/close 状态再执行”收口成：
  - 渲染阶段收集动作
  - handler 统一执行确认动作
- 这一步的价值在于，窗口级控制流已经不再是 cleanup 详情窗的局部优化，而是开始成为 `dirotter-ui` 里确认窗的统一惯例。
