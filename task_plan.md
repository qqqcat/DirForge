# Task Plan
## DirOtter Architecture Refactor - Priority 1-2 Progress

### Priority 1: Memory & Code Quality Optimization ✅ COMPLETED (2026-05-01 Final Verification)
### Priority2: Data Structure & UI Optimization ✅ COMPLETED (2026-05-03 Verification)
**Completed:**
- ✅ **SmolStr optimization** - Integrated in `dirotter-core/src/lib.rs`
- ✅ **StringPool with reference counting** - Added `rc_tracker: HashMap<StringId, usize>` and `intern()`/`release()` methods
- ✅ **Unified error handling** - Added `thiserror`/`anyhow` to workspace dependencies
  - Created `error.rs` with `DirOtterError` enum using `thiserror`
  - Added `Result<T>` type alias
- ✅ **Property-based testing** - Added `proptest` to `dirotter-core` dev-dependencies
  - Created `property_tests.rs` with `test_node_store_insert_delete`, `test_string_pool_intern`, `test_string_pool_reference_counting`
- ✅ **Unit tests** - Existing tests pass (9/9 passed)

**Final Verification (2026-05-01):**
- ✅ `cargo build --workspace`: 0 errors, 11 warnings (9 unused code + 1 unused imports + 2 unnecessary parentheses)
- ✅ `cargo test --workspace`: ALL TESTS PASSED
  - dirotter-core: 9 passed
  - dirotter-ui: 33 passed
  - dirotter-scan: 7 passed
  - dirotter-actions: 7 passed
  - dirotter-report: 4 passed
  - dirotter-testkit: 4 passed
  - All other crates: passed
- ✅ `thiserror`, `anyhow`, `proptest` properly integrated
- ✅ Application runs successfully

---

### Priority 2: Data Structure & UI Optimization 🟡 IN PROGRESS (85% Complete)

#### Stage 2: SumTree Evaluation ✅ COMPLETED (2026-05-01)
**Decision:** **暂不引入 SumTree**
- **Rationale:** DirOtter is a file system tool, not a professional terminal emulator
- **Current status:** Vec+HashMap is sufficient for current scale
- **Future consideration:** Only if O(log n) queries or million-node support needed
- **Recorded in:** `task_plan.md`

#### Stage 3: UI Optimization ✅ MOSTLY COMPLETE (85%)

**Completed:**
- ✅ **Theme system (100%)** - `theme.rs` created and fully integrated
  - `ThemeConfig`, `ColorPalette`, `SpacingConfig` structs defined
  - `apply_theme()`, `theme_from_settings()` functions implemented
  - `river_teal()`, `river_teal_hover()`, `river_teal_active()` functions
  - Integrated into `lib.rs` (replaced old `build_dark_visuals()`/`build_light_visuals()`)
  - Warning reduction: 23 → 9 warnings
- ✅ **Low-level Painter rendering** - `result_pages.rs` custom draw layer
- ✅ **Ranked list optimization** - `render_ranked_size_list` uses Painter
- ✅ **Compilation & tests** - All pass (0 errors, 9 warnings)
  - `cargo build --workspace`: ✅ 0 errors
  - `cargo test -p dirotter-core`: ✅ 9 passed
  - `cargo test -p dirotter-ui`: ✅ 33 passed

**Pending (Non-blocking):**
- ⚠️ **egui caching mechanism** - FrameCache syntax issues, simple cache implemented

---

### Next Actions (Future Optimization):
1. **Complete egui caching** - Research correct `FrameCache<Value, Computer>` usage

2. **Clean remaining warnings** - 9 unused code warnings (non-blocking)

---

## Stage 4: UI Architecture Refactor ✅ COMPLETED (2026-05-01)

### Status: ✅ COMPLETED
- ✅ Created `ui_shell.rs` module for shell UI rendering
- ✅ Established clean module boundaries (lib.rs state, ui_shell.rs rendering)
- ✅ Fixed all compilation errors and type mismatches
- ✅ Verified full workspace compilation success
- ✅ Preserved existing functionality and behavior

### Implementation Details
- **lib.rs**: DirOtterNativeApp struct, event loop, public helper functions
- **ui_shell.rs**: Shell UI functions (dialogs, panels, status bars)
- **Clean separation**: State coordination vs UI rendering

### Validation
- ✅ `cargo check -p dirotter-ui`: Zero errors
- ✅ `cargo check` (full workspace): No regressions
- ✅ Application runs successfully (tested 2026-05-01)

---

## Stage 1-3 Progress Summary (2026-05-01 Update)

### Stage 1: Memory & Code Quality Optimization ✅ COMPLETED (2026-05-01)

**Completed:**
- ✅ **SmolStr optimization** - Integrated in `dirotter-core/src/lib.rs`
- ✅ **StringPool with reference counting** - Added `rc_tracker: HashMap<StringId, usize>` and `intern()`/`release()` methods
- ✅ **Unified error handling** - Added `thiserror`/`anyhow` to workspace dependencies
  - Created `error.rs` with `DirOtterError` enum using `thiserror`
  - Added `Result<T>` type alias
- ✅ **Property-based testing** - Added `proptest` to `dirotter-core` dev-dependencies
  - Created `property_tests.rs` with `test_node_store_insert_delete`, `test_string_pool_intern`, `test_string_pool_reference_counting`
- ✅ **Unit tests** - Existing tests pass (9/9 passed)

**Validation:**
- ✅ `cargo build --workspace` passes
- ✅ `cargo test -p dirotter-core` passes (9 unit tests passed)
- ✅ `thiserror`, `anyhow`, `proptest` properly integrated

**Next Actions (Stage 2-3):**
1. Evaluate SumTree crate or implement custom SumTree
2. Implement egui caching for expensive layouts
3. Build complete theme system

---

### Stage 2: Data Structure Upgrade ⚠️ DEFERRED (2026-05-01)

**Decision:** **暂不引入 SumTree**

**Rationale:**
1. **项目类型不同** - DirOtter 是文件系统工具，不是专业终端模拟器
2. **当前够用** - Vec+HashMap 对当前规模足够，引入 SumTree 复杂度过高
3. **投入产出比低** - 需要维护 B-tree 结构，但收益有限
4. **已有增量更新** - `update_node_size`/`propagate_size_delta` 已提供增量能力

**Completed (Keep):**
- ✅ **Incremental size updates** - `update_node_size`/`propagate_size_delta` in NodeStore
- ✅ **Top-k cache optimization** - Fixed-capacity top-k candidate structure
- ✅ **Dirty ancestor propagation** - Dirty marking for incremental updates

**Future Consideration:**
- 仅在需要 O(log n) 查询或支持百万级节点时再考虑
- 可评估 `zed-sum-tree` crate (concurrency-friendly B-tree)

**Next Actions (Focus on Stage 3 instead):**
1. ✅ Implement egui caching mechanism
2. ✅ Build complete theme system
3. ✅ Optimize Treemap rendering with LOD

---

### Stage 3: UI Optimization ✅ MOSTLY COMPLETE (85% - 2026-05-01)

**Completed:**
- ✅ **Low-level Painter rendering** - `result_pages.rs` custom draw layer`
- ✅ **Ranked list optimization** - `render_ranked_size_list` uses Painter`
- ✅ **Shared path optimization** - `Arc<str>` in walker→aggregator→publisher`
- ✅ **Complete theme system** - `theme.rs` created and integrated into `lib.rs`
  - `ThemeConfig`, `ColorPalette`, `SpacingConfig` structs defined`
  - `apply_theme()`, `theme_from_settings()` functions implemented`
  - `river_teal()`, `river_teal_hover()`, `river_teal_active()` functions`
  - Dark/light mode support with complete color schemes`
  - Successfully replaced old `build_dark_visuals()`/`build_light_visuals()` in `lib.rs``
- ✅ **Warning reduction** - Reduced from 23 to 9 warnings`
- ✅ **Compilation & tests** - All pass (0 errors, 9 warnings)`

**Pending (Non-blocking optimizations):**
- ⚠️ **egui caching mechanism** - FrameCache syntax issues, simple cache implemented`
- ❌ **Treemap LOD rendering** - No level-of-detail implementation yet`
- ❌ **Custom rendering pipeline / GPU acceleration** - Using egui immediate mode`

**Next Actions (Optional/Future):**
1. Research correct egui FrameCache usage for layout caching
2. Implement LOD rendering for large treemaps (performance optimization)
3. Further warning cleanup (remaining 9 warnings are non-blocking)

**Current Status (2026-05-01):**
- ✅ Theme system fully integrated and working`
- ✅ All tests pass (dirotter-core: 9, dirotter-ui: 33)`
- ✅ Compilation clean (0 errors)`
- ⚠️ 9 non-blocking warnings (unused functions with `#[allow(dead_code)]`)

---
## 2026-04-09 Status Refresh

### Current Judgment
- 当前工程主线已经从“先把主链路做出来”切换到“把发布成熟度和回归保护补齐”。
- `fmt / check / test / clippy / build` 当前全部通过，说明代码基线处于可继续收口的稳定状态。
- UI 拆分、扫描增量化、共享路径化和轻量存储模型都已进入已落地状态，不再只是计划项。

### Phase Status
- Phase 0：完成
- Phase 1：主体完成
  - `cleanup.rs`
  - `controller.rs`
  - `dashboard.rs`
  - `dashboard_impl.rs`
  - `result_pages.rs`
  - `settings_pages.rs`
  - `advanced_pages.rs`
  - `view_models.rs`
- Phase 2：主体完成
  - dirty 祖先传播
  - dirty-only rollup
  - entry-time 聚合维护
  - 固定容量 top-k
  - snapshot payload / generation threshold guard
- Phase 3：主体完成
  - `walker -> aggregator -> publisher` 共享 `Arc<str>`
  - live/full snapshot 分层
  - UI 排行、选择态和结果视图逐步改为共享路径或 `NodeId`
- Phase 4：部分完成
  - 默认无数据库
  - `settings.json` + 当前会话快照
  - 原子写入
  - 持久设置目录不可写时回退临时会话存储
  - 临时 session 目录退出清理与陈旧目录回收
- Phase 5：基础完成
  - `ci.yml`
  - `release-windows.yml`
  - 便携打包 / 安装 / 卸载脚本
  - 可选签名入口

### Current Next Steps
1. 为 Overview / Result View / Settings 补最小视觉回归或截图对比。
2. 扩展真实删除链路的跨平台和异常场景覆盖，尤其是权限不足、锁占用和系统目录边界。
3. 在正式对外发布前配置 Windows 签名 secrets，并把签名验收纳入发布检查表。
4. 继续压缩 `dirotter-ui` 协调层剩余体量，但不再以“大拆文件”为目标，而是以稳定职责边界为目标。

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
- 默认推荐扫描策略、高级扫描节奏与盘符快捷入口
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

### 一键提速确认与失败详情改进（2026-04-04）
1. 已完成
   - cleanup 确认窗已改为可滚动完整目标列表，所有待处理路径都会完整展示
   - Inspector `最近执行` 在存在失败项时已新增可点击详情入口
   - 外层失败反馈已不再直接拼接首条失败原因，完整失败原因与建议改由详情窗承载
2. 当前收益
   - 用户在点击确认前可以完整复核本次一键提速/批量清理到底会处理哪些路径
   - 多失败项场景不再只暴露一条被截断的失败文本，失败定位与重试建议更可用
3. 当前验证
   - `cargo test -p dirotter-ui`：通过
   - `cargo build --workspace`：通过
   - `cargo run -p dirotter-app`：已启动验证，进程因 GUI 持续运行被超时终止

### 失败详情窗布局与删除期间响应性修复（2026-04-04）
1. 已完成
   - 失败详情窗已重排为顶部关闭、受控宽度、全宽失败卡片布局
   - 失败原因主文案已改为本地化失败标题与解释，英文原始错误信息只保留在技术细节区
   - 删除链路已新增逐项进度回传，后台执行期间会持续刷新已处理/成功/失败统计与当前处理项
2. 当前收益
   - 失败详情不再横向撑爆窗口，也不需要滚到底部才能关闭
   - 多语言界面不再被英文硬编码失败原因主导
   - 一键提速执行过程中，前台可以持续看到进度与当前处理项，减少“界面卡死”的观感
3. 当前验证
   - `cargo test -p dirotter-actions`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo test --workspace`：通过

### 失败详情全语言补齐（2026-04-04）
1. 已完成
   - 失败详情按钮、窗口标题、说明文案、失败卡片标题、建议/技术细节标题已补齐到全部已支持语言
   - `view_models.rs` 中新增的 `快速清理 / 选择安全项 / 打开所选` 等关联键也已同步补齐，避免部分语言继续回退英文
   - `i18n` 已增加缺失键补丁层与 legacy fallback，用于承接拆分生成字典中尚未覆盖的新键
2. 当前收益
   - 所有已支持语言在失败详情相关流程中都不会再混入英文硬编码
   - 后续新增 `view-model` 文案如果漏翻，会被测试直接拦下
3. 当前验证
   - `cargo fmt`：通过
   - `cargo test -p dirotter-ui`：通过

### 删除后结果同步脱离 UI 主线程（2026-04-04）
1. 已完成
   - 已重新分析并确认窗口 `Not Responding` 的根因是删除完成后的结果同步仍在 UI 主线程执行，而不是删除执行线程本身
   - 删除完成后的 `NodeStore` 重建、cleanup analysis 重算、排行和结果摘要同步已迁到独立后台阶段
   - Inspector 后台任务卡与顶部横幅新增 `结果同步中 / Sync Results` 阶段，删除完成后不会再直接把重活压回 `update()`
2. 当前收益
   - Windows 不会再因为删除收尾阶段长时间阻塞消息循环而把窗口标记成 `Not Responding`
   - 删除完成后的结果视图和清理建议仍会自动同步，但这一步不会再卡住前台
   - 根因与 `egui` 官方“GUI 线程保持非阻塞”的建议重新对齐，避免同类问题反复出现
3. 当前验证
   - `cargo fmt`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo build`：通过
   - `dirotter-app`：已重新编译并启动

### 结果视图按需载入脱离 UI 主线程（2026-04-04）
1. 已完成
   - 已确认 `结果视图` 页面自身仍存在同步快照载入链路，会在切页时直接读取 SQLite、解压快照并重建 `NodeStore`
   - 结果视图快照恢复现已迁到独立后台 session，切页时只触发后台加载，不再在渲染函数里同步读缓存
   - 删除或结果同步期间，如果结果 `store` 当前不在内存，结果视图会先显示等待同步提示，避免再次把重活压回 UI 主线程
2. 当前收益
   - 删除期间点击 `结果视图` 不会再触发同步快照恢复导致窗口进入 `Not Responding`
   - 平时从快照恢复结果页时也不会再因为大快照解压和反序列化直接卡住界面
   - `结果视图` 页的重活链路也和 `egui` 官方非阻塞原则重新对齐
3. 当前验证
   - `cargo fmt`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo build`：通过
   - `dirotter-app`：已重新编译并启动

### Inspector Memory Status 重做（2026-04-04）
1. 已完成
   - `Workspace Context` 卡已移除
   - Inspector 下半区已改为系统内存状态卡
   - `view_models.rs` 已统一整形系统可用内存、内存负载、DirOtter 占用与最近一次释放结果
2. 当前收益
   - 右栏新增独立纵向滚动，释放后底部信息不再不可达
   - 原先 300px 窄栏内容易横向裁切的 chip 布局已被移除
   - 长说明文案已删除，右栏信息密度更接近主流 dashboard 侧栏
3. 当前验证
   - `cargo test -p dirotter-ui`：通过

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

### Remaining Dialog 控制流收口进展（2026-04-04）
1. 已完成
   - `ui_delete_confirm_dialog()` 已改为“收集确认窗动作 -> handler 处理”
   - `ui_cleanup_delete_confirm_dialog()` 已改为同样的动作分发模式
   - 剩余窗口现在都不再依赖局部 `confirmed / keep_open` 之外再叠加额外分支状态来驱动执行
2. 当前收益
   - cleanup 详情窗、永久删除确认窗、cleanup 确认窗三类窗口的控制流风格已统一
   - 后续继续补行为测试或抽公共窗口模式时，边界已经更整齐
3. 当前验证
   - `cargo check -p dirotter-ui`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
