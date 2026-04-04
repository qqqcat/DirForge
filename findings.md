# Findings

## 2026-04-03 Engineering Assessment Follow-up

## Verification
- 已新增专项计划文档：`docs/engineering-improvement-plan-2026-04.md`
- 已修复当前 `clippy -D warnings` 失败项
- 已修复两处低成本实现问题：
  - 扫描发送阻塞时间采样不准确
  - 删除计划中的文件/目录类型判断过于粗糙
- 已修复 `dirotter-actions`、`dirotter-ui` 在 `clippy -D warnings` 下继续暴露出的风格和生成文件问题
- `cargo fmt --all`：通过
- `cargo build --workspace`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过

## Key Findings
- 当前仓库不是典型失控“屎山”，但 `dirotter-ui` 已出现明显 God File / God Object 趋势。
- 扫描快照链路仍有重复全量计算，后续应优先做增量化。
- Rust 的类型安全和并发安全发挥得不错，但少拷贝数据流和增量算法优势尚未充分发挥。
- 工程质量门槛需要收口到 `clippy -D warnings`，否则代码质量会继续缓慢下滑。

## Immediate Actions
- 将专项改进计划与实施计划落地为独立文档。
- 先修当前静态检查失败和低风险实现问题，再进入下一轮架构级重构。
- 清理生成翻译文件中的隐形字符，避免它们持续污染质量门槛。

## Phase 1 Progress
- `cleanup analysis` 已从 [lib.rs](E:/DirForge/crates/dirotter-ui/src/lib.rs) 抽离到独立模块 [cleanup.rs](E:/DirForge/crates/dirotter-ui/src/cleanup.rs)。
- 这次抽离优先迁移了纯分析逻辑：分类、风险判断、候选评分、Top-N 收口和清理计划预处理。
- `DirOtterNativeApp` 目前仍保留薄包装方法，以保证现有 UI 和测试行为不变。
- `delete / memory` 的后台线程和 relay 逻辑已抽离到 [controller.rs](E:/DirForge/crates/dirotter-ui/src/controller.rs)。
- 当前 `lib.rs` 仍保留 UI 状态变更和结果消费逻辑，但线程启动与后台结果收取已不再直接写在主文件里。

## 2026-03-18 Scan Experience Optimization

## Verification
- 代码改造已完成，新增 `ScanMode` 预设与 UI 模式选择。
- 自动化测试已补充模式映射与预设模式扫描覆盖。
- `cargo fmt --all`：通过
- `cargo test --workspace`：通过

## Key Changes
- 扫描入口已从 `SSD / HDD / Network + batch / snapshot` 收口为三档用户模式。
- `ScanMode` 已集中定义在扫描层，避免 UI、测试和文档各自维护一套规则。
- UI 已明确提示“三种模式都会完整扫描当前范围，差异只在扫描节奏与界面刷新方式”。
- 扫描模式会保存到本地设置，避免每次重启重新选择。

## Product Impact
- 普通用户不再被迫理解底层性能参数。
- “快速扫描（推荐）”为默认路径，显著降低首次使用成本。
- “深度扫描”和“超大硬盘模式”把复杂场景选择从技术术语改成任务语义。

## 2026-03-18 Result View Simplification

## Verification
- 结果页已与实时扫描解耦，扫描中不会再尝试实时 treemap。
- 新页面只读取完成后的结果树，并只展示当前目录的直接子项。
- 会在扫描完成后把最终树快照写入 SQLite 快照缓存。

## Key Changes
- 新增 `Result View` 页面，替代文档里残留的实时 treemap 预期。
- 结果页支持下钻、返回上级和“跳到当前选中目录”。
- 删除成功后，结果页会跟随 `NodeStore` 局部重建即时刷新。

## 2026-03-19 Result View Layout Fix

## Verification
- 结果页主列表已从自然高度卡片改为填充型结果区。
- 条形图区会吃满页面剩余高度，长列表改为内部滚动。

## Key Changes
- Result View 页面切到 fill-height 页面布局。
- 目录结果条形图区加入显式剩余高度计算，避免大面积空白。
- 条目渲染改为 `show_rows`，在填充型结果区内承载长列表。

## 2026-03-19 Cleanup Suggestion System V1

## Verification
- Overview 已新增清理建议卡片、详情窗和缓存一键清理确认流。
- 规则分类、风险规则与聚合逻辑已补单测。
- `cargo test -p dirotter-ui`：通过

## Key Changes
- 扫描完成后会基于 `NodeStore` 生成规则驱动的清理分析层，而不是只停留在体积数据展示。
- 建议系统会区分 `可清理 / 谨慎 / 禁删`，并把安全缓存项单独提炼为快捷清理入口。
- 批量清理与单项删除统一复用现有回收站删除链路，避免出现两套执行逻辑。

## Product Impact
- 用户进入 Overview 后，首先看到的是“能释放多少空间”，而不是继续自己猜该从哪里下手。
- `一键清理缓存（推荐）` 让产品开始具备“直接帮用户完成任务”的能力。

## 2026-03-19 Overview / Settings Clipping Fix

## Verification
- 首页与设置页滚动布局已补充底部安全区。
- Settings 页已移除页面内部多余的固定宽度包裹。
- `cargo test -p dirotter-ui`：通过

## Key Changes
- 修复了最后一排卡片贴着视口底边时看起来像被裁掉的问题。
- 首页新增清理建议后，卡片间距做了轻量压缩，首屏不再那么拥挤。
- 根因定位为 `egui` 子 `Ui` 的 clip rect 过紧，卡片描边被裁掉；现已统一改为放宽 `clip rect` 后再绘制，而不是继续按页面打补丁。
- 进一步把 Overview / Settings 从并排双列卡片重构为纵向章节流，直接移除最容易重复出问题的布局结构。
- Settings 最终改成主流分组设置页：高频项前置、说明项后置、控制项使用设置行而不是说明卡片堆叠。

## 2026-03-19 French / Spanish Localization

## Verification
- Settings 已可切换 `中文 / English / Français / Español` 四种界面语言。
- 启动时会优先根据 `zh / fr / es / en` 系统语言环境自动选择默认语言。
- 已新增源码级覆盖测试：自动提取当前 `self.t(...)` 英文键，并验证法语 / 西班牙语词典完整命中。
- `cargo fmt --all`：通过
- `cargo check --workspace`：通过
- `cargo build --workspace`：通过
- `cargo test --workspace`：通过

## Key Changes
- 在 `dirotter-ui` 中新增独立 `i18n.rs`，把法语和西班牙语完整接入现有 `中文 + 英文` 本地化调用模型。
- 当前 UI 英文键已补齐到法语与西班牙语完整版本，不再保留“长说明退回英文”的半完成状态。
- 语言设置值已支持 `en / zh / fr / es` round-trip，避免旧逻辑把未知语言回退成英文。

## Product Impact
- DirOtter 现在不再局限于中英文界面，可直接覆盖更多欧洲用户。
- 扩展方式保持了现有 `self.t(zh, en)` 调用结构，后续继续补全文案时改动成本较低。

## 2026-03-17 Project Reassessment

## Verification
- `cargo check --workspace`：通过
- `cargo test --workspace`：通过

## Current Strengths
- 扫描链路已形成 worker + 聚合线程 + 有界发布通道的稳定流水线，取消、错误和完成态可回归验证。
- UI 已从“不断调一个个控件”转向“页面级布局策略”：统一最大内容宽度、对称 gutter、页面级纵向滚动。
- 标题旁状态胶囊不再持有翻译后的字符串，而是由内部状态枚举按当前语言实时渲染。
- 删除动作已进入右侧 Inspector，支持回收站删除、永久删除确认、后台执行与删除后局部刷新。
- 选中文件夹后，“最大文件”榜单可切换到目录上下文，分析不再始终停留在整盘。
- 默认根路径与盘符快捷扫描已降低首次使用门槛。

## Document Drift Fixed
- README 已补齐 2026-03-17 的 UI 布局系统状态。
- 综合评估、系统设计、安装指南和快速上手已从“控件修补思路”更新为“页面级布局思路”。
- UI 规格已移除“固定高度独立滚动排行榜”这类过时表述。

## Current Risks
- `Overview / Live Scan / Treemap` 之间仍缺一个完全统一的正式栅格系统，视觉成熟度仍依赖人工逐页校正。
- 布局类问题当前主要靠截图人工发现，缺少自动化视觉回归保护。
- 删除过程目前只有阶段性状态提示，缺少更细粒度的进度表达。
- 跨平台真实删除边界覆盖仍有限，尤其是权限不足、占用锁和回收站可见性。

## 2026-03-19 Overview Layout Follow-up

### What Changed
- Overview 已从“设置章节式首页”改回更符合主流 dashboard 的结构，并继续收掉重复信息：
  - Hero 结论区
  - KPI 指标条
  - 全宽扫描卡
  - 双列证据区（最大文件夹 / 最大文件）
- 首页顶部现在优先回答“现在最值得做什么”，而不是堆说明文案。
- 顶部四张卡改为唯一指标：`磁盘已用 / 磁盘可用 / 已扫描体积 / 错误`，不再和独立卷摘要卡重复。
- 首页栅格已单独收窄，并改成显式列宽分配，避免卷空间摘要漂移到相邻区域以及左右 gutter 不对称。
- 首页宽度继续从 `1240` 收窄到 `1160`，并把外层 gutter 提升到 `64`，优先修正右侧视觉上贴近 Inspector 的问题。
- 扫描卡交互已继续向 Windows 常见习惯收口：先点盘符，再在需要时手动输子目录；根目录输入框不再占据卡片顶部主视觉。
- `清理建议详情` 现已补齐窗口关闭入口，并把条目行改为固定大小列，避免右侧大小被长路径挤掉。

### Stability Follow-up
- 停止扫描的真正根因是 worker 可能睡在条件变量上，外部只改取消标记却没有让等待线程及时醒来；现已改为短超时轮询取消标记。
- 取消后的扫描不会再误写入完成态快照与历史，避免把部分扫描结果当成完整结果保存。
- SQLite 快照改为同一路径只保留最新一份，并在写入后执行 WAL checkpoint，控制 `dirotter.db` 相关文件继续膨胀。
- 缓存一键清理不再复用普通回收站删除链路，而是改为 `staging -> 后台 purge` 两阶段方案，优先保证点击后的即时反馈。
- 扫描最后阶段的真正卡顿点并不是遍历本身，而是 UI 线程同步执行最终快照压缩/落库、历史写入、错误导出和清理建议重算；现已拆到后台整理阶段。
- Windows 文件永久删除已开始接入低层 fast path，降低大文件永久删除时长期卡在高层删除调用上的概率。

### Residual Risks
- 目前仍缺自动化视觉回归，首页结构调整后主要依赖人工检查间距、节奏和两列收缩行为。

## Product Direction Follow-up
- 对普通用户而言，自动扫描历史、自动错误 CSV、自动快照落库的价值明显低于“现在能清多少、怎么一键处理”。
- 当前最值得保留的主路径是：快速扫描 -> 清理建议 -> 缓存快清 / 删除执行 -> 结果确认。
- `History / Errors / Diagnostics` 更像维护者和诊断工具，后续应降级为二级入口，而不是继续和主清理路径并列。
- “一键释放内存”不宜做成含糊承诺；如果要做，应限定为应用自身占用优化或实验性辅助工具。

## 2026-03-20 Product Refocus Implemented

## Verification
- `cargo fmt --all`：通过
- `cargo test --workspace`：通过
- `cargo build -p dirotter-app`：通过

## Key Changes
- 扫描完成默认不再自动保存快照、历史和错误 CSV，完成态成本明显下降。
- 清理建议已改为规则命中候选生成，并通过每类 Top-N 与全局上限控制分析规模。
- 主导航已收口为 `Overview / Live Scan / Result View / Settings`，维护型页面通过 `高级工具` 开关进入。
- 诊断页现在承担按需维护动作，避免把普通用户拖入工程化收尾流程。
- Inspector 已补充 `释放 DirOtter 内存` 与 `清理残留 staging`，让“减少应用自身额外占用”有了诚实且可操作的入口。

## Product Impact
- 扫描完成后，用户更快进入“能删什么、怎么删”的主路径，而不是继续等待落库和导出。
- 界面主导航变得更像清理工具，而不是分析资产管理台。
- 维护与诊断能力仍保留，但已从默认主流程中降级，减少普通用户困惑。

## Recommended Next Steps
1. 为主页面建立统一的 12-column 栅格和固定 gutter token。
2. 引入最小视觉回归或截图对比，覆盖留白、对齐、标题状态和列表高度。
3. 继续压实删除链路中的阶段反馈与回收站可见性体验。

## Phase 1 Update
- `dashboard` 页面层已从 [lib.rs](E:/DirForge/crates/dirotter-ui/src/lib.rs) 拆出，当前实现位于 [dashboard.rs](E:/DirForge/crates/dirotter-ui/src/dashboard.rs) 与 [dashboard_impl.rs](E:/DirForge/crates/dirotter-ui/src/dashboard_impl.rs)。
- `dirotter-ui` 主文件的职责继续下降：`cleanup analysis`、后台 `controller`、以及首页 `dashboard` 页面都已脱离单文件堆叠。
- 本轮唯一新增问题是页面模块初版出现编码污染；该问题已在同轮修复，没有遗留到主分支状态。
- 下一步最适合继续拆的是 `current_scan / treemap / diagnostics` 页面层，而不是再把更多业务塞回 `lib.rs`。

## Phase 1 Update 2
- `current_scan / treemap` 页面层已继续从 [lib.rs](E:/DirForge/crates/dirotter-ui/src/lib.rs) 拆出，当前实现位于 [result_pages.rs](E:/DirForge/crates/dirotter-ui/src/result_pages.rs)。
- 现在 `dirotter-ui` 主文件已经不再直接承载三块页面细节：`dashboard`、`current_scan`、`treemap`。
- 下一步最合适的是继续拆 `diagnostics / settings` 页面层，然后再评估是否需要把页面共享 helper 再下沉一层。

## Phase 1 Update 3
- `history / errors / diagnostics / settings` 页面层也已继续从 [lib.rs](E:/DirForge/crates/dirotter-ui/src/lib.rs) 拆出，当前实现位于 [advanced_pages.rs](E:/DirForge/crates/dirotter-ui/src/advanced_pages.rs) 与 [settings_pages.rs](E:/DirForge/crates/dirotter-ui/src/settings_pages.rs)。
- 现在 `dirotter-ui` 主文件已不再直接承载主要页面渲染细节，Phase 1 的“先拆页面层”目标基本完成。
- 下一步更合适的是回到共享 helper、状态分组和扫描快照成本，而不是继续往 `lib.rs` 里搬页面代码。

## Core Optimization Update
- `NodeStore` 的 dirty 链路已经从“有字段但基本没用”变成“真正参与 rollup”的实现，位置在 [lib.rs](E:/DirForge/crates/dirotter-core/src/lib.rs)。
- `top_n_largest_files / largest_dirs` 也已不再走“全量建堆再 pop”的高分配路径，而是改成固定容量候选堆，直接降低 snapshot 节奏上的重复开销。
- `save_snapshot()` 已收口为事务式替换，位置在 [lib.rs](E:/DirForge/crates/dirotter-cache/src/lib.rs)，并去掉了每次保存后强制 `wal_checkpoint(TRUNCATE)` 的同步重操作。
- 下一步最合适的是继续下沉共享 helper / 状态分组，或者进一步压缩 `aggregator.make_snapshot_data()` 里的路径解析和 view 组装成本。

## Core Optimization Update 2
- `NodeStore::add_node()` 已进一步改成 entry-time 维护祖先聚合值，位置在 [lib.rs](E:/DirForge/crates/dirotter-core/src/lib.rs)。
- 这意味着扫描主路径不再完全依赖“等 snapshot 来补账”，而是在节点进入 store 时就把 `size_subtree / file_count / dir_count` 向上累计。
- `Aggregator::make_snapshot_data()` 已移除先 `rollup()` 再生成 snapshot 的路径，位置在 [aggregator.rs](E:/DirForge/crates/dirotter-scan/src/aggregator.rs)。
- `top_files_delta / top_dirs_delta` 现在直接从命中节点导出 `NodeId`，不再先转字符串路径再回查索引。
- 当前收益是：快照线程继续从“计算者”退回“读取者”，下一步可以更聚焦于消息体瘦身，而不是继续反复补聚合账。

## Scan Message Update
- `walker -> aggregator -> publisher` 这一段热路径已开始摆脱层层 `String` 复制。
- [walker.rs](E:/DirForge/crates/dirotter-scan/src/walker.rs) 中的 `EntryEvent` 已切到共享 `Arc<str>`。
- [lib.rs](E:/DirForge/crates/dirotter-scan/src/lib.rs) 与 [publisher.rs](E:/DirForge/crates/dirotter-scan/src/publisher.rs) 中的 `BatchEntry.path` / `frontier` 也已切到共享路径。
- [aggregator.rs](E:/DirForge/crates/dirotter-scan/src/aggregator.rs) 中的 `pending_by_parent` 已同步改为共享路径键，减少等待父目录场景下的重复分配。
- 现在共享路径已经继续推进到事件边界：[lib.rs](E:/DirForge/crates/dirotter-scan/src/lib.rs) 中的 `ScanProgress.current_path`、`SnapshotView.top_files / top_dirs` 和完成态 Top-N 排行都已改为共享路径。
- 共享路径现在已经继续推进到 [lib.rs](E:/DirForge/crates/dirotter-ui/src/lib.rs) 的实时状态层：`scan_current_path / live_top_* / completed_top_*` 也已改为共享路径持有。
- `ResolvedNode` 这一层现在也已继续共享化，位置在 [lib.rs](E:/DirForge/crates/dirotter-core/src/lib.rs)。`SnapshotView.nodes` 不再为每个节点重新分配 `name/path String`。
- `SnapshotView` 的实时路径也已继续收口：[aggregator.rs](E:/DirForge/crates/dirotter-scan/src/aggregator.rs) 中，非 full-tree snapshot 已不再携带变更节点列表，只保留 `changed_node_count`。
- 完成态事件也已移除重复的 Top-N 排行，UI 会在拿到最终 `store` 后本地重建，避免跨线程再携带一份可导出数据。
- 当前还没完全发挥 Rust 的优势，因为部分页面 helper 仍会在渲染前物化文本；但字符串物化点和重复 payload 都已经收口到“真正需要渲染或文本输出时才发生”。

## Perf Guard Update
- 性能回归保护现在不再只覆盖“整次扫描多久结束”，而是开始覆盖 snapshot 这条真正被优化过的链路。
- [benchmark_thresholds.rs](E:/DirForge/crates/dirotter-testkit/tests/benchmark_thresholds.rs) 已新增大树 snapshot payload 阈值测试。
- [aggregator.rs](E:/DirForge/crates/dirotter-scan/src/aggregator.rs) 已新增本地 snapshot 组装耗时与 payload 阈值测试。
- 这意味着后续如果有人重新把 snapshot 改回“带大量节点/大字符串 payload”，或者让组装逻辑重新退化，测试会先响。

## Runtime Telemetry Update
- 当前 snapshot 链路的“变胖风险”已经不再只能靠离线测试识别。
- [lib.rs](E:/DirForge/crates/dirotter-telemetry/src/lib.rs) 现在会继续累计 live/final snapshot 的：
  - `changed_node_count`
  - `materialized_nodes`
  - `ranked_items`
  - `text_bytes` 估算值
- [lib.rs](E:/DirForge/crates/dirotter-scan/src/lib.rs) 不会为了做 telemetry 再额外 JSON 序列化 snapshot，而是只统计共享路径和节点文本长度。
- 这让 diagnostics 能直接回答一个更实际的问题：最近的优化是否真的把实时 snapshot 稳定压成了“稀疏 payload”。
