# Task Plan

## Goal
基于 2026-04-03 的整体代码评估结果，推动 DirOtter 从“中上质量、局部已有技术债”的状态，进入“高质量 Rust 工程实现”的下一阶段。

## Problem Statement
- `dirotter-ui` 已膨胀为单文件核心，状态、调度、业务规则和页面渲染混杂。
- 扫描快照链路仍包含明显的重复全量计算。
- 扫描事件流中存在不必要的字符串复制，Rust 的少拷贝优势发挥不充分。
- 工程门槛尚未完全收口到 `clippy -D warnings`。
- 现有计划文档停留在旧任务，不足以指导下一轮系统性改进。

## Optimization Strategy
1. 先做低风险高收益收口：
   - 修复当前静态检查失败
   - 修复已确认的低成本实现问题
   - 更新专项计划与评估文档
2. 再拆 UI 架构：
   - 分离 controller、analysis、diagnostics、pages、widgets
   - 降低 `DirOtterNativeApp` 的职责密度
3. 同步推动扫描链路增量化：
   - 降低快照期间的全量 `rollup + top-k` 成本
   - 让 dirty 标记真正参与增量更新
4. 最后收口数据流和持久化：
   - 减少字符串复制
   - 提升快照写入一致性
   - 固化 CI 质量门槛

## Plan Document
- 详细改进计划与实施计划见：
  - `docs/engineering-improvement-plan-2026-04.md`

## Execution Plan
- [x] 重新审视 workspace、核心 crate、扫描链路、平台层和 UI 主体。
- [x] 产出专项改进计划与实施计划文档。
- [x] 修复当前 `clippy -D warnings` 失败项。
- [x] 修复已确认的低成本实现问题：
  - 扫描发送阻塞时间采样
  - 删除计划中的路径类型判断
- [~] 分阶段拆解 `dirotter-ui`。
  - [x] 抽离 `cleanup analysis` 到独立模块
  - [x] 抽离 controller
  - [x] 抽离页面模块
- [~] 分阶段实现扫描快照增量化。
  - [x] dirty 祖先传播
  - [x] dirty-only rollup
  - [x] 固定容量 top-k
  - [x] `add_node` 祖先聚合值即时维护
  - [x] `aggregator` 快照阶段移除补账式 `rollup`
  - [~] 快照视图与消息链路继续瘦身
    - [x] `walker -> aggregator -> publisher` 路径共享化
    - [x] `SnapshotView / ScanProgress` 继续延迟物化
- [~] 分阶段瘦身扫描消息链路与快照持久化。
  - [x] 热路径批量事件 `path` 改为共享 `Arc<str>`
  - [x] 继续压缩完成态与实时快照的展示 payload
  - [x] 为稀疏 snapshot payload 增加运行时 telemetry

## Verification Plan
1. 工程验证：
   - `cargo fmt --all`
   - `cargo build --workspace`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
2. 文档验证：
   - 更新专项计划、综合评估、发现记录和进展记录
3. 后续阶段验证：
   - 为 UI 拆分和扫描增量化补对应回归测试

## Verification Status
- `cargo fmt --all`：已通过
- `cargo build --workspace`：已通过
- `cargo test --workspace`：已通过
- `cargo clippy --workspace --all-targets -- -D warnings`：已通过

## Result
- 本轮重点已切换为“质量收口 + 架构减债”。
- 详细计划已独立沉淀到 `docs/engineering-improvement-plan-2026-04.md`。
- 当前阶段的目标不是继续堆功能，而是为下一轮高质量重构建立边界、顺序和验证门槛。
- 当前仓库已经完成一次质量门槛收口，后续 UI 拆分和扫描增量化可以在更干净的基线上推进。
- Phase 1 已正式启动，`cleanup analysis` 已完成第一次模块拆分。
- 页面层首轮拆分已基本完成，当前重心已进入扫描核心链路的“entry-time 增量维护”。

## Follow-up: Result View Simplification
- Treemap 不再按实时扫描刷新。
- 新结果页只在扫描完成后工作，并优先读取扫描完成后的结果树 / 缓存快照。
- 结果页只展示当前目录的直接子项，支持逐层下钻与返回上级。
- 目标是保留“看目录占比”的核心价值，同时避免百万节点和重布局算法拖垮 UI。

## Follow-up: Result View Layout Optimization
- 结果页不再使用自然高度塌缩的内容卡。
- 页面结构调整为“顶部摘要 + 底部填充型结果区”。
- 条形图区必须吃满剩余高度，长列表走结果区内部滚动，而不是留下大面积未利用空白。

## Follow-up: Cleanup Suggestion System (V1)
- 目标：把首页从“只看数据”推进到“给出可执行的释放空间建议”。
- 分析层：
  - 基于扫描完成后的 `NodeStore`
  - 规则分类 `cache / downloads / video / archive / installer / image / system / other`
  - 风险分级 `Low / Medium / High`
  - 评分采用 `size + unused_days + category bias`
- UI：
  - Overview 顶部新增 `清理建议` 卡
  - 支持 `查看详情`
  - 支持 `一键清理缓存（推荐）`
- 执行：
  - 快捷清理默认走回收站
  - 详情窗里绿色默认勾选、黄色默认不勾选、红色锁定不可删
- 验证：
  - `cargo fmt --all`
  - `cargo test --workspace`
  - `cargo build -p dirotter-app`

## Follow-up: Overview / Settings Clipping Fix
- 问题：
  - 首页在新增清理建议后，首屏更紧，底部卡片容易出现“像被截断”的观感
  - Settings 页在页面级宽度约束内部又套了固定宽度容器，右侧卡片更容易贴边
- 修正：
  - 为滚动页统一补充底部安全区
  - Settings 页移除多余的二次固定宽度裁切
  - 首页卡片纵向间距做轻量压缩
  - 卡片和提示条统一走放宽 `clip rect` 的渲染路径，避免描边被子布局裁剪矩形截断
  - 首页和设置页改成纵向章节布局，不再继续依赖双列卡片矩阵
  - Settings 再进一步改为“窄内容列 + 分组设置行”，参考主流设置页模式重做
  - Overview 不再套用设置页的章节样式，改为主流 dashboard 结构：Hero 结论区、KPI 指标条、双列操作区、双列证据区
  - Overview 进一步改为独立首页宽度 + 显式双列宽度分配，修正卷空间摘要漂移、卡片重叠和左右留白不对称
  - 首页继续把专用宽度从 `1240` 收到 `1160`，外层 gutter 提高到 `64`，优先修正视觉上右侧贴 Inspector 的问题
  - KPI 指标条内的四张卡片改为强制填满分配列宽，修正卡片本身缩在左侧造成的假性不对称
  - 清理建议详情窗改为居中受控对话框，补齐关闭入口，并为右侧大小列预留固定宽度，修正长路径导致的右侧截断
  - Overview 再次收口信息架构：移除与 KPI 重复的 `卷空间摘要` 大卡，卷级信息并入首页四张指标卡与全宽扫描卡
  - 顶部四张卡调整为唯一指标：`磁盘已用 / 磁盘可用 / 已扫描体积 / 错误`
  - 首页扫描卡继续收口为“盘符优先、手动目录次要”，移除顶部解释文案，并把缩小后的路径输入框移到盘符区之后
  - 停止扫描改为真正的可退出流程，避免 worker 在条件变量等待时挂死 UI
  - SQLite 快照改为“每个根路径只保留最新一份”，并在写入后主动 checkpoint WAL，避免数据库文件随重复扫描持续膨胀
  - `一键清理缓存` 改为 `staging -> 后台 purge`，先追求秒级体感反馈
  - Windows 文件永久删除接入更低层 fast path，失败时保留现有删除回退
  - 启动时自动清理 `.dirotter-staging` 遗留项，并在扫描阶段排除该内部目录

## Follow-up: French / Spanish Localization
- 目标：
  - 在现有中英文基础上增加法语与西班牙语
  - 保持现有 `self.t(zh, en)` 调用模型，避免大范围重构 UI 调用点
- 实现：
  - 新增 `Lang::Fr / Lang::Es`
  - 新增 `language` 设置值解析与保存：`en / zh / fr / es`
  - 根据 `LC_ALL / LANG` 自动识别 `zh / fr / es / en`
  - 用英文文案作为稳定键，向法语 / 西班牙语词典做映射
  - 法语 / 西班牙语必须补齐当前 UI 全量英文键，不接受说明文案回退英文
- 验证：
  - `cargo fmt --all`
  - `cargo check --workspace`
  - `cargo build --workspace`
  - `cargo test --workspace`
  - 额外增加源码级词典覆盖测试，防止未来新增英文文案后漏翻

## 2026-03-20 Product Refocus: 从“分析器”转向“释放空间工具”

### 用户目标重述
- 用户打开 DirOtter 的首要诉求不是“保存扫描历史”或“导出错误 CSV”，而是尽快知道：
  - 现在能释放多少空间
  - 先删什么最安全
  - 点一次后能否立刻见效

### 当前低价值 / 高成本项
- 扫描完成自动保存完整快照到 SQLite
- 扫描完成自动写入扫描历史
- 扫描完成自动导出错误 CSV
- 主导航持续暴露 `History / Errors / Diagnostics`

### 保留项
- 三档扫描模式与盘符快捷入口
- Overview 顶部清理建议与 `一键清理缓存`
- 最大文件夹 / 最大文件证据区
- Inspector 删除执行与风险提示
- `.dirotter-staging -> 后台 purge` 极速缓存清理链路

### 优化方案
1. 扫描完成默认不再自动落 SQLite 快照、不再自动写历史、不再自动导出错误 CSV  
   - 这些功能改成手动或开发者模式入口
   - 默认完成态只保留用户可见结果和清理动作
2. `重算整套清理建议` 重新定义为“生成当前用户真正能操作的候选列表”  
   - 不再为整棵树做重型全量分析
   - 只保留规则命中的候选、每类 Top-N、可执行动作与预计释放空间
3. `History / Errors / Diagnostics` 从主路径降级  
   - 进入二级入口或仅在调试模式显示
4. 新增内存相关能力时，避免误导性“系统一键释放内存”承诺  
   - 优先考虑 `减小 DirOtter 占用` 或 `刷新资源占用`
   - 如需系统级内存功能，应单独标为实验性工具，而不是主卖点

### 实施方案
#### Phase 1: 去掉默认重收尾
- 完成扫描后只保留：
  - summary
  - top files / top dirs
  - 当前会话内可用的清理建议
- SQLite 快照 / 历史 / 错误 CSV 改为：
  - 用户手动点击保存
  - 或仅在开发诊断模式启用

#### Phase 2: 重构清理建议计算
- 把当前 `build_cleanup_analysis(store)` 从“全量扫全树”收口为：
  - 规则命中目录优先
  - 文件只保留超过阈值且可操作的候选
  - 每类保留 Top-N
  - 背景增量更新，不阻塞完成态切换

#### Phase 3: 收口导航
- 主导航优先保留：
  - Overview
  - Live Scan
  - Result View
  - Settings
- `History / Errors / Diagnostics` 收入二级入口或开发者开关

#### Phase 4: 内存能力取舍
- 不建议承诺“系统一键释放内存”
- 如确需提供，优先做：
  - `减小 DirOtter 占用`
  - `清理残留 staging`
  - `刷新资源占用`
- 这类功能必须明确标注为辅助工具，不替代磁盘空间清理主链路

### 实施完成状态（2026-03-20）
1. Phase 1 已完成
   - 扫描完成默认不再自动保存 SQLite 快照
   - 扫描完成默认不再自动写入历史
   - 扫描完成默认不再自动导出错误 CSV
   - 完成态只保留轻量结果整理与清理建议生成
2. Phase 2 已完成
   - 清理建议已改为规则命中候选生成
   - 每类候选带 Top-N 上限
   - 全局候选总量带上限，避免扫描结束后再做整树重分析
3. Phase 3 已完成
   - 主导航已收口为 `Overview / Live Scan / Result View / Settings`
   - `History / Errors / Diagnostics` 已移入 `高级工具 / Advanced Tools`
   - 高级工具开关放入 Settings，并持久化到本地设置
4. Phase 4 已完成
   - Inspector 已新增 `释放 DirOtter 内存`
   - Inspector 已新增 `清理残留 staging`
   - 诊断页已新增手动保存当前快照 / 手动记录扫描摘要 / 手动导出错误 CSV

### 最终验证
- `cargo fmt --all`：通过
- `cargo test --workspace`：通过
- `cargo build -p dirotter-app`：通过

### Phase 1 增量进展（2026-04-03）
1. 已完成
   - 抽离 `cleanup.rs`
   - 抽离 `controller.rs`
   - 抽离 `dashboard.rs + dashboard_impl.rs`
   - 抽离 `result_pages.rs`
   - 抽离 `settings_pages.rs`
   - 抽离 `advanced_pages.rs`
2. 当前收益
   - `dirotter-ui/src/lib.rs` 不再同时承载 cleanup 规则、后台 controller 和主要页面渲染细节
   - 页面层已基本完成首轮拆分，主文件更接近“应用协调器”而不是“UI 大杂烩”
   - 下一步应转向共享 helper 下沉、状态结构收口和扫描链路成本优化
3. 当前验证
   - `cargo fmt --all`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### 下一层增量进展（2026-04-03）
1. 已完成
   - `NodeStore::mark_dirty()` 向祖先传播
   - `NodeStore::rollup()` 改为 dirty-only 重算
   - `top_n_largest_files / largest_dirs` 改为固定容量候选堆
   - `CacheStore::save_snapshot()` 改为事务式替换并取消每次强制 WAL 截断
2. 当前收益
   - snapshot 节奏上的重复全量计算和重复大堆分配已明显收口
   - snapshot 保存路径不再每次执行同步重 checkpoint
3. 当前验证
   - `cargo test -p dirotter-core`：通过
   - `cargo test -p dirotter-scan`：通过
   - `cargo test -p dirotter-cache`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### 更深一层增量进展（2026-04-03）
1. 已完成
   - `NodeStore::add_node()` 改为 entry-time 维护祖先 `size_subtree / file_count / dir_count`
   - `Aggregator::make_snapshot_data()` 不再先做补账式 `rollup`
   - 快照 Top-N 的 delta 直接从命中节点导出，不再走 `path -> NodeId` 回查
2. 当前收益
   - 扫描线程把聚合账本前移到节点写入时维护，快照阶段进一步从“重算”转成“取数”
   - `aggregator` 的快照组装成本继续下降，尤其适合高频 snapshot 节奏
3. 当前验证
   - `cargo fmt --all`：通过
   - `cargo test -p dirotter-core`：通过
   - `cargo test -p dirotter-scan`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### Phase 3 启动进展（2026-04-03）
1. 已完成
   - `EntryEvent.path / parent_path / name` 改为共享 `Arc<str>`
   - `BatchEntry.path` 改为共享 `Arc<str>`
   - `Publisher.frontier` 改为共享路径队列，进度事件只在真正发给 UI 时再物化 `String`
   - `Aggregator.pending_by_parent` 与 `root_path` 改为共享路径键
   - `ScanProgress.current_path` 改为共享 `Arc<str>`
   - `SnapshotView.top_files / top_dirs` 改为共享路径排行
   - `ScanEvent::Finished` 的 Top-N 排行改为共享路径，UI 收到后再统一物化
   - `dirotter-ui` 内部 `scan_current_path / live_top_* / completed_top_*` 改为共享路径持有
   - `ResolvedNode.name / path` 改为共享 `Arc<str>`
   - `SnapshotView.nodes` 不再为每个节点强制复制 name/path `String`
   - 实时/最终 `SnapshotView` 默认不再携带变更节点列表，只保留 `changed_node_count`
   - `ScanEvent::Finished` 不再重复携带可由 `store` 重建的 Top-N 排行
2. 当前收益
   - walker 到 publisher 的热路径已不再为同一条路径层层复制 owned `String`
   - 共享路径现在已推进到 UI 接管后的排行/进度状态，真正的字符串物化进一步收口到渲染 helper
   - 实时 snapshot 的节点 payload 也已开始复用共享字符串，而不是为每个节点重复分配完整路径
   - 实时快照与完成态事件里已经移除了两块明确冗余的展示 payload：无用节点列表、可重建 Top-N
   - snapshot payload 大小和 snapshot 组装耗时现在已经开始有回归阈值保护
3. 当前验证
   - `cargo fmt --all`：通过
   - `cargo test -p dirotter-scan`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo test -p dirotter-core`：通过
   - `cargo clippy -p dirotter-scan --all-targets -- -D warnings`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
   - `cargo clippy -p dirotter-core --all-targets -- -D warnings`：通过

### 性能基线进展（2026-04-04）
1. 已完成
   - 为大树扫描新增 `snapshot payload` 大小阈值测试
   - 为 `Aggregator::make_snapshot_data(false)` 新增组装耗时与 payload 阈值测试
   - 扩展 `crates/dirotter-testkit/perf/baseline.json`
2. 当前收益
   - 后续如果有人重新把 snapshot 改回“大量节点 + 大量字符串”的路径，测试会直接报警
   - 不再只靠人工感知“变慢/变重”，而是开始有可执行的性能红线
3. 当前验证
   - `cargo test -p dirotter-scan incremental_snapshot_generation_stays_under_threshold -- --nocapture`：通过
   - `cargo test -p dirotter-testkit benchmark_snapshot_payload_threshold_massive_tree -- --nocapture`：通过

### 运行时观测进展（2026-04-04）
1. 已完成
   - 为 snapshot 新增低成本 runtime telemetry：
     - `avg/max snapshot changed nodes`
     - `avg/max snapshot materialized nodes`
     - `avg snapshot ranked items`
     - `avg/max snapshot text bytes`
   - diagnostics 导出现在会自动带上这些指标
2. 当前收益
   - 不需要在热路径上为每个 snapshot 额外做一次 JSON 序列化，也能看出 payload 是否重新膨胀
   - 如果后续有人把 live snapshot 又改回“携带大量节点 / 大量文本”，诊断数据会直接暴露回退
3. 当前验证
   - `cargo test -p dirotter-telemetry`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### SnapshotView 分层进展（2026-04-04）
1. 已完成
   - `SnapshotView` 已从单一结构改为显式分层：
     - `LiveSnapshotView`
     - `FullSnapshotView`
     - `SnapshotView::{Live, Full}`
   - live 扫描路径现在只接轻量视图，不再默认暴露 `nodes`
   - full-tree 节点物化改为显式 `Full` 路径
2. 当前收益
   - 轻量实时路径与重型调试/全量路径的类型边界变清楚了
   - 后续继续压 live payload 时，不容易再把 `nodes` 误带回常规路径
3. 当前验证
   - `cargo test -p dirotter-scan`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### UI NodeId 化进展（2026-04-04）
1. 已完成
   - `SelectedTarget` 已新增 `node_id`
   - `TreemapEntry` 已新增 `node_id`
   - 新增 `select_node()`，当前结果树内的选择优先走 `NodeId`
   - treemap 页与 cleanup 候选列表中，凡是明确来自当前 `NodeStore` 的点击都优先按节点选择
2. 当前收益
   - UI 内部对当前结果树的选择不再主要依赖路径字符串回查
   - 当前会话结果树里的选中态与 treemap 交互更接近“ID 驱动 + 路径兜底”
3. 当前验证
   - `cargo test -p dirotter-ui`：通过
   - `cargo test -p dirotter-scan`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### UI View-Model 下沉进展（2026-04-04）
1. 已完成
   - 新增 `crates/dirotter-ui/src/view_models.rs`
   - `summary_cards`
   - `scan_health_summary / scan_health_short`
   - `current_ranked_dirs / current_ranked_files`
   - `contextual_ranked_files_panel`
   - 以及相关排行物化 helper 已从 `lib.rs` 下沉
2. 当前收益
   - `DirOtterNativeApp` 主文件进一步回到状态协调职责
   - 结果页、首页、状态栏依赖的展示物化逻辑开始集中到独立模块，而不是继续散在主状态实现里
3. 当前验证
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### 批量字符串物化收口进展（2026-04-04）
1. 已完成
   - `view_models.rs` 里的实时/完成态排行已改为共享 `RankedPath`
   - `contextual_ranked_files_panel()` 已改为返回共享路径排行
   - `live_files` 已改为共享路径持有
   - `TreemapEntry.name / path` 已改为共享 `Arc<str>`
2. 当前收益
   - 排行面板和结果页不再默认为每次刷新批量构造 `Vec<(String, u64)>`
   - 实时扫描文件列表在 UI 接管阶段也不再立刻把共享路径落成 `String`
   - 字符串物化点继续后移到点击、文本截断和真正渲染边缘
3. 当前验证
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### Inspector 与删除链路共享路径化进展（2026-04-04）
1. 已完成
   - `SelectedTarget.name / path` 已改为共享 `Arc<str>`
   - 当前结果树命中的 Inspector 目标不再默认构造 owned `String`
   - 删除执行计划仍在边界处显式转回 `String`
2. 当前收益
   - Inspector、删除确认窗、cleanup 候选和 treemap 目标现在共享同一批路径/名称分配
   - 字符串物化继续收口到真正需要对外部 API 或执行计划交互的边界
3. 当前验证
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### UI 路径状态共享化进展（2026-04-04）
1. 已完成
   - `CleanupPanelState.selected_paths` 已从 `HashSet<String>` 改为 `HashSet<Arc<str>>`
   - `treemap_focus_path` 已从 `Option<String>` 改为 `Option<Arc<str>>`
   - cleanup 勾选、treemap 聚焦和父级跳转链路已改为直接复用共享路径
2. 当前收益
   - UI 内部高频 `contains/remove/切换聚焦` 不再反复持有独立路径副本
   - Inspector、cleanup、treemap 三条链路现在共享同一套路径状态模型
   - 真正需要 `String` 的地方继续只保留在执行计划和外部动作边界
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### Inspector / Confirm View-Model 下沉进展（2026-04-04）
1. 已完成
   - `view_models.rs` 已新增 Inspector 目标摘要、后台删除任务摘要、永久删除确认和 cleanup 确认的展示模型
   - `ui_inspector()`、`ui_delete_confirm_dialog()`、`ui_cleanup_delete_confirm_dialog()` 已改成消费 view-model，而不是在主状态函数里直接拼展示文本
2. 当前收益
   - `DirOtterNativeApp` 主文件继续从“状态协调 + 展示整形”回到更明确的协调职责
   - Inspector 和确认窗的文本拼装现在集中在 `view_models.rs`，后续继续优化展示边界时更容易统一处理
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### Inspector 动作态与反馈文案下沉进展（2026-04-04）
1. 已完成
   - `view_models.rs` 已新增 Inspector 动作可用性模型
   - 打开位置 / 快速清理 / 回收站 / 永久删除 / 系统内存释放 的启用条件已改为由 view-model 统一计算
   - Explorer 反馈、删除反馈和最近执行摘要也已改由 view-model 统一整形
2. 当前收益
   - `ui_inspector()` 不再同时承担按钮启用判断、提示文案选择和反馈文案拼装
   - Inspector 已进一步从“布局 + 状态判断 + 文案拼接”收口为“布局 + 动作分发”
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo build --workspace`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过

### Inspector Workspace Context 下沉进展（2026-04-04）
1. 已完成
   - `view_models.rs` 已新增 Workspace Context 展示模型
   - 根目录和来源文案已改为由 view-model 统一整形
2. 当前收益
   - `ui_inspector()` 里的 Workspace Context 已不再直接读取和拼装状态字段
   - Inspector 主函数现在更接近纯布局 + 动作分发，展示整形已基本收口完成
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过

### Cleanup Details Window 下沉进展（2026-04-04）
1. 已完成
   - `view_models.rs` 已新增 cleanup 详情窗的 tabs / 统计区 / 按钮态 / item 行展示模型
   - `ui_cleanup_details_window()` 已改为消费 view-model，不再在布局函数里直接拼分类标签、统计值和 item 元数据
2. 当前收益
   - cleanup 详情窗从“重布局函数 + 大量展示整形”收口为“布局 + 交互分发”
   - 后续如果继续调清理建议文案、标签或按钮策略，不需要再穿插修改 UI 布局主体
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过

### Cleanup Details 交互控制流收口进展（2026-04-04）
1. 已完成
   - `ui_cleanup_details_window()` 已从多布尔旗标流改为“动作收集 + 统一处理”
   - 新增 cleanup 详情窗动作枚举与集中处理 helper
   - 打开位置、主操作触发、永久删除触发、勾选写回和选中对象聚焦都已脱离窗口尾部的分散 `if` 链
2. 当前收益
   - cleanup 详情窗主函数不再依赖多组 `trigger_* / request_* / select_*` 状态拼接控制流
   - 交互分发边界更清楚，后续补测试或继续拆函数时更容易定位单个动作路径
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
