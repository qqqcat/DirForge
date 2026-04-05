# DirOtter 项目综合评估报告（2026-04-03）

## 1. 评估范围与方法

本轮评估覆盖以下内容：

1. Workspace 架构与 crate 边界（`crates/*`）
2. 核心链路：扫描、聚合、缓存、报告、平台能力、删除执行、UI 集成
3. UI 页面级布局系统
4. 文档与代码一致性
5. 构建与测试验证：
   - `cargo build --workspace`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`

## 2. 总体结论

DirOtter 当前处于 **可持续迭代的工程原型阶段**：主链路稳定，crate 分层方向正确，但尚未达到“高质量 Rust 代码”标准。它不是失控屎山，但局部已经出现明显技术债，尤其集中在 UI 单文件膨胀和扫描快照重复全量计算上。

### 2.1 当前已达到的能力

- **扫描链路稳定**：并发扫描、聚合、取消、错误与完成态都具备可回归验证路径。
- **完成态稳定**：扫描结束阶段不再因完整树复制或重任务同步收尾而假死。
- **删除流程回归主界面**：危险动作已进入 Inspector，支持回收站删除、永久删除确认和后台执行。
- **删除后即时局部刷新**：删除成功后，Inspector、榜单、概览统计与 treemap 会同步变化。
- **目录上下文分析增强**：选中文件夹后，“最大文件”榜单会切换到该目录内部。
- **首次体验改善**：默认根路径优先落到系统盘/首个卷，并提供盘符快捷扫描按钮。
- **扫描入口已用户化**：普通用户不再面对 `SSD / HDD / Network + batch / snapshot`，而是三档扫描模式。
- **清理建议主路径已落地**：Overview 会在完成态优先给出可释放空间、分类建议和安全缓存一键清理入口。
- **扫描收尾已开始异步化**：目录遍历结束后，最终快照保存、历史写入与建议汇总转到后台整理阶段，避免最后一拍把窗口卡成无响应。
- **默认收尾已完成降级**：自动快照落库、历史写入和错误导出已移出默认完成态，改为高级工具中的手动动作。
- **页面级布局策略已成型**：主要内容页具备统一最大内容宽度、对称 gutter、页面级纵向滚动；标题状态胶囊能够随语言实时本地化。
- **多语言能力已扩展**：设置页现已支持 19 种语言，并改为下拉选择；其中 `中文 / English / Français / Español` 为完整 UI 文案，自动检测已覆盖 `zh / en / ar / nl / fr / de / he / hi / id / it / ja / ko / pl / ru / es / th / tr / uk / vi`。
- **结果视图已与实时扫描解耦**：Treemap 方向收口为轻量结果页，只在扫描完成后按目录层级查看直接子项。
- **结果视图版式已修正**：目录结果区改为吃满剩余高度的主浏览区，避免只显示一小段内容而留下大块空白。
- **Overview / Settings 截断问题已修正**：滚动页补上底部安全区，Settings 移除额外固定宽度包裹，末尾卡片不再贴边裁切。
- **卡片描边裁切根因已修正**：统一卡片和提示条在绘制前放宽 `clip rect`，避免 `egui` 在紧凑子布局中裁掉右边框和下边框。
- **Overview 首页结构已再次收口**：首页已改为 Hero 结论区、唯一 KPI 指标条、全宽扫描卡、双列证据区，去掉与 KPI 重复的卷摘要大卡。
- **Overview 首页栅格已独立**：首页不再复用设置页组件，改为更窄页面宽度 + 显式列宽分配，修正卷摘要漂移、卡片重叠和左右留白不对称。
- **停止扫描已修正为安全收尾**：worker 不再因为条件变量等待而挂住，停止扫描后会进入 `Stopping` 态并安全退出。
- **SQLite 快照膨胀已收口**：同一路径只保留一份最新快照，写入改为事务式替换；WAL 维护不再绑死在每次保存热路径上。
- **缓存清理已切入极速链路**：安全缓存项先进入同卷 staging 区，UI 立即返回成功，再由后台继续 purge。
- **永久删除已开始下探低层路径**：Windows 文件永久删除优先走低层 fast path，减少大文件删除时被高层文件系统调用拖住的概率。
- **主导航已完成收口**：默认只保留 `Overview / Live Scan / Result View / Settings`，维护型页面进入 `高级工具`。
- **应用自身占用已有诚实工具**：可手动释放 DirOtter 工作集内存并清理 `.dirotter-staging` 遗留项。
- **页面结构已重排**：Settings 不再依赖说明卡拼贴，改为纵向分组设置流，直接规避紧凑并排卡片的边缘问题。
- **Settings 已改为主流设置页模式**：窄内容列、分组章节、设置行结构，高频控制项前置。
- **基础质量工具链已纳入本轮复核**：当前评估不再只看 `build/test`，也纳入 `clippy -D warnings` 作为质量信号。

### 2.2 当前仍需重点补强的方向

- **UI 核心已明显膨胀**：`dirotter-ui/src/lib.rs` 体量过大，页面渲染、控制器、分析逻辑和状态机仍耦合在同一文件。
- **UI 拆分已开始，但仍处早期**：`cleanup analysis` 和后台 `controller` 已独立成模块，说明拆分路径可行；不过页面层仍在主文件内。
- **扫描快照仍偏重**：当前快照期间仍依赖重复的全量 `rollup + top-k` 计算，后续大目录/长时间扫描会继续放大成本。
- **数据流复制仍偏多**：扫描事件在跨线程链路上仍大量传递拥有型字符串，Rust 的共享数据优势未充分利用。
- **正式栅格系统仍未完全收口**：虽然已经去掉固定高度主卡与榜单，但 `Overview / Live Scan / Treemap` 仍需要统一的正式页面栅格。
- **UI 自动化回归不足**：像留白、对齐、滚动和状态胶囊语言切换这类问题仍主要靠人工截图发现。
- **删除用户反馈偏粗粒度**：当前是阶段性后台任务提示，还没有字节级或阶段级进度。
- **规则仍偏保守**：V1 规则已足够落地，但阈值、分类覆盖和 warning 策略仍需继续打磨。
- **跨平台真实删除测试矩阵仍需扩展**：权限不足、文件被占用、系统目录等场景还需要更多实机覆盖。
- **持久化热路径仍有优化空间**：快照保存仍偏保守，事务边界和 checkpoint 策略后续应继续收口。

## 3. 本轮验证结果

### 构建验证

- `cargo build --workspace`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过

### 测试验证

- `cargo test --workspace`：通过

当前覆盖包括：

- `dirotter-scan`：扫描取消、受限目录、符号链接回路、目录积压、深宽树
- `dirotter-core`：`NodeStore`、`rollup`、Top-N 查询
- `dirotter-platform`：路径评估、卷信息、Explorer 打开、回收站能力
- `dirotter-actions`：计划、真实删除、受保护路径、目录删除、文件占用失败
- `dirotter-report`：文本/CSV/诊断导出
- `dirotter-telemetry`：指标与系统快照
- `dirotter-testkit`：扫描/去重阈值回归
- `dirotter-ui`：格式化、目录上下文榜单、删除后局部树重建、根路径选择

## 4. 分层能力评估

### 4.1 Scan（`dirotter-scan`）

- 已采用 worker 池 + 聚合线程 + 有界发布队列。
- 发布链路具备节流与中间态丢弃，避免 UI 被事件洪泛拖死。
- 当前评价：已从“能跑”进入“可验证、可持续迭代”的工程实现。

### 4.2 Core（`dirotter-core`）

- `NodeStore` + `rollup` + Top-N 查询稳定。
- 同时承担 UI 局部刷新的核心数据来源。
- 当前评价：是系统中最稳定的共享域模型层。

### 4.3 Actions（`dirotter-actions`）

- 支持删除计划、真实删除执行、失败分类与审计记录。
- UI 已直接消费真实删除执行结果。
- Windows 回收站删除增加系统回收站二次校验。
- 当前评价：已达到“可真实执行”的程度，但反馈颗粒度和实机边界覆盖仍需增强。

### 4.4 Cache / Report / Telemetry

- SQLite 承担设置、历史、审计。
- 诊断包、摘要 JSON、错误 CSV 等导出链路可用。
- 当前评价：工程骨架齐全，适合作为后续排错和发布诊断基础。

### 4.5 UI（`dirotter-ui`）

**现状：**

- 页面结构稳定为：Overview / Live Scan / Result View / Settings
- `History / Errors / Diagnostics` 已降级为二级高级工具入口
- `Operations` 页面已移除，危险动作进入 Inspector
- 扫描时工具栏语义已切换为 `Stop Scan`
- 扫描设置已切换为 `快速扫描（推荐）/ 深度扫描 / 超大硬盘模式`
- 结果页已切换为扫描完成后生成的轻量目录下钻视图
- Overview 已切换为“分析 + 决策”首页：顶部先给出清理建议，而不是只展示体积数据
- Overview 已移除与顶部指标重复的卷空间摘要大卡，卷级信息并入 KPI 与扫描卡
- 已支持分类详情窗与 `一键清理缓存（推荐）` 确认流
- 结果页的目录条形图区域已改为内部滚动的填充型主区域
- 主要页面已采用外层纵向滚动，避免内部固定高度区域把内容裁掉
- 榜单不再依赖固定高度 mini-scroll，而是按当前条目展开到页面内容流中
- 删除后已支持局部树重建，概览统计与 treemap 同步变化
- 永久删除已增加确认层，删除结果会提供更明确的撤销/失败提示
- 删除确认后会立即关闭弹窗，并切换到后台任务提示，用户仍可继续浏览结果
- 选中文件夹后，“最大文件”榜单会切换到该目录内部的最大文件
- 标题旁状态胶囊已支持多语言实时切换，不再保留硬编码英文状态
- 设置页语言选项现已扩展到 19 种语言，并改为下拉选择；核心导航、状态和操作文案完整覆盖中文、英文、法语和西班牙语，其余新增语言当前回退英文文案
- 缓存一键清理现在优先追求“立即感觉成功”，不再与普通回收站删除共用同一条慢链路

**评价：**

- 已经不是“开发者面板”，而是具备实际用户价值的桌面磁盘分析器雏形。
- 当前短板已经从“功能可用性”转向“UI 架构拆分、统一栅格系统与视觉成熟度”。
- 用户扫描体验的主要技术门槛已明显下降，后续重点不再是隐藏参数，而是继续压实模式文案与真实场景反馈。
- 产品心智已开始从“看目录数据”转向“直接释放空间”，但 V2 仍需要补重复文件、长期未使用文件等更强建议来源。

## 4.6 Rust 优势发挥情况

### 已发挥的部分

- 使用 `enum / Result / typed id` 管理状态和错误边界，基本避免了弱类型脚本式实现。
- 并发扫描链路采用 worker、聚合和有界通道，线程安全边界清晰。
- `NodeStore` 已做过瘦身，说明项目对 Rust 的内存布局优势已有意识。
- crate 分层总体清楚，平台相关 `unsafe` 基本收束在专门模块。

### 尚未充分发挥的部分

- 增量算法没有跟上：快照期仍存在全量计算。
- 数据流少拷贝化不足：消息链路中仍频繁传递 `String`。
- 工具链门槛尚未完全前置：直到本轮才把 `clippy -D warnings` 作为显式质量标准。
- UI 模块边界没有利用 Rust 模块系统进一步收口，导致大文件持续膨胀。

## 5. 风险矩阵

| 风险项 | 级别 | 现状 | 建议 |
|---|---|---|---|
| 真实删除跨平台边界一致性 | 中 | 有执行链路、审计与回收站校验，但实机覆盖仍有限 | 扩展权限/锁冲突/系统目录/回收站可见性测试 |
| UI 缺少自动化交互/视觉回归 | 中 | 目前主要靠人工回归 | 增加 UI 集成测试或最小截图回归 |
| 删除中的用户感知偏粗 | 中 | 有后台任务提示但无细粒度进度 | 增强阶段性反馈与完成后定位/提示 |
| 页面栅格体系尚未完全统一 | 中 | 主要页面已改用外层滚动和对称 gutter，但仍缺统一 12-column 体系 | 收口 Overview / Live Scan / Treemap 页面栅格 |
| 文档与代码再次漂移 | 低 | 本轮已同步，但后续变更快 | 合并功能时绑定文档检查 |
| UI 单文件继续膨胀 | 高 | `dirotter-ui` 已承担过多职责 | 分阶段拆解 controller / analysis / pages / widgets |
| 扫描快照重复全量计算 | 高 | 当前可以工作，但规模上来后成本会继续放大 | 优先做增量 rollup 与增量 top-k |
| 生成文件污染静态检查 | 中 | 本轮已发现翻译生成文件中的隐形字符 | 将生成脚本或生成后校验纳入质量门槛 |

## 6. 近期优先级建议（未来 2~4 周）

1. **拆分 `dirotter-ui`**
   - 先拆 controller、cleanup analysis、diagnostics、pages
   - 让 `DirOtterNativeApp` 回到装配和协调职责

2. **扫描链路增量化**
   - 让 dirty 标记真正参与 rollup
   - 将 top-k 改为固定容量候选结构

3. **收口质量门槛**
   - 固化 `fmt / build / test / clippy`
   - 为关键页面和关键扫描路径补回归

## 7. 结论

DirOtter 当前已经从“概念原型”进入“可持续迭代的工程化桌面工具原型”阶段，但要进一步迈向高质量 Rust 项目，下一步必须从“继续堆功能”切换为“拆 UI、减快照成本、收口工程质量门槛”。

当前最关键的变化不是再加几个功能，而是主链路和页面布局策略都已开始成体系：

- 扫描不会轻易假死
- 完成态不会轻易卡死
- 删除动作不再跳页
- 删除后的 UI 不再等下次重扫
- 页面内容不再依赖固定高度小框裁切
- 文档与代码现已重新对齐

## 2026-04-03 增量更新

- `dirotter-ui` 的模块拆分已继续推进到页面层，不再只停留在 `cleanup analysis` 和后台 `controller`。
- `dashboard` 页面已独立到 [dashboard.rs](E:/DirForge/crates/dirotter-ui/src/dashboard.rs) 与 [dashboard_impl.rs](E:/DirForge/crates/dirotter-ui/src/dashboard_impl.rs)，这说明 UI 核心单文件继续拆解是可执行而且低风险的。
- `current_scan` 与 `treemap` 页面也已独立到 [result_pages.rs](E:/DirForge/crates/dirotter-ui/src/result_pages.rs)，说明页面拆分不需要先重写状态层。
- `history / errors / diagnostics / settings` 页面也已独立到 [advanced_pages.rs](E:/DirForge/crates/dirotter-ui/src/advanced_pages.rs) 与 [settings_pages.rs](E:/DirForge/crates/dirotter-ui/src/settings_pages.rs)，说明 `dirotter-ui` 的首轮页面拆分已基本落地。
- `NodeStore` 的 dirty 链路和 top-k 路径也已开始收口到更增量、更低分配的实现，位置在 [lib.rs](E:/DirForge/crates/dirotter-core/src/lib.rs)；snapshot 持久化也已改成事务式替换，位置在 [lib.rs](E:/DirForge/crates/dirotter-cache/src/lib.rs)。
- 因此，对“UI 单文件继续膨胀”的风险判断不变，但当前风险状态较上次评估已略有改善：拆分不再只是计划，已经开始稳定落地。
- 扫描核心链路也已继续向更“Rust 式”的 entry-time 增量维护推进：`NodeStore::add_node()` 会即时维护祖先聚合值，而 [aggregator.rs](E:/DirForge/crates/dirotter-scan/src/aggregator.rs) 的 snapshot 组装已不再先依赖补账式 `rollup()`。
- 扫描消息链路的少拷贝化也已开始落地：`walker -> aggregator -> publisher` 内部已经改用共享 `Arc<str>` 传递路径，只有 UI 边界才继续物化 `String`。
- 这一层已经进一步推进到事件边界：`ScanProgress.current_path`、实时 snapshot Top-N 和完成态 Top-N 也已改为共享路径，物化点继续后移到 UI 接管时。
- 共享路径现在还继续推进到了 UI 持有层，实时/完成态排行和当前路径都已不再默认落成 `String`。
- `ResolvedNode` payload 这一层也已开始共享化，实时 snapshot 的节点视图不再为每个节点重复构造 `name/path String`。
- 实时 snapshot 和完成态事件里的冗余展示 payload 也已开始被直接删掉：非 full-tree snapshot 不再携带节点列表，完成态事件也不再重复发送可由 `store` 重建的 Top-N。
- snapshot 关键路径的性能基线也已开始固化：现在不仅测整次扫描耗时，还新增了 snapshot payload 大小和 snapshot 组装耗时阈值。
- snapshot 稀疏化现在还开始进入运行时观测：telemetry 已新增 changed nodes、materialized nodes、ranked items 和 text-bytes 估算指标，diagnostics 能直接暴露 payload 是否重新膨胀。
- `SnapshotView` 也已继续从类型层收口：当前 live/full 视图已经显式分开，不再把 `nodes` 作为常规实时路径默认暴露的字段。
- UI 当前结果树的选择态也已开始往 `NodeId` 优先推进，treemap 和 cleanup 候选这类 store-backed 交互不再主要依赖路径字符串。
- UI 展示层也已开始收口成独立 view-model 模块，说明 `dirotter-ui` 的减债已从“拆页面”推进到“拆状态协调和展示整形的边界”。
- UI 的高频榜单和结果页条目也已开始真正少拷贝：实时/完成态排行、上下文榜单、`live_files` 和 `TreemapEntry` 都已推进到共享路径持有。
- UI 内部剩余的路径状态热点也已继续共享化：cleanup 勾选集合和 treemap 当前焦点不再持有独立 `String` 副本，而是直接复用 `Arc<str>`。
- Inspector 和两个删除确认窗的展示整形也已继续下沉到 `view_models`，说明 UI 主文件已经不只是“文件数变少”，而是在职责上逐步收口。
- Inspector 的动作可用性判断和反馈文案现在也已改由 `view_models` 统一计算，UI 渲染层里的状态分支继续下降。
- Inspector 底部区域现已继续重做为系统内存状态卡；`Workspace Context` 已删除，`ui_inspector()` 改为消费 `view_models` 输出的内存状态摘要与最近一次释放结果。
- cleanup 详情窗的 tabs、统计区、按钮态和 item 行展示整形也已开始下沉到 `view_models`，说明“重 UI 函数减债”已经扩展到 Inspector 之外。
- cleanup 详情窗的交互控制流现在也已从布尔旗标链收口为动作枚举分发，函数内部职责开始真正分层。
- 剩余确认窗现在也已统一为动作分发模式，说明窗口级交互控制流的收口已经覆盖主要弹窗，而不是只停在单个大函数。
- 这说明“Rust 优势发挥不足”的结论仍成立，但已经从“问题判断”进入“正在被逐步修正”的状态；后续真正的短板会继续收敛到页面 helper 的最终物化策略，以及性能基线继续细化。
