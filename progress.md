# Progress Log

## 2026-04-04（确认窗完整列表与失败详情卡片）
- `一键提速` / cleanup 确认窗已改为可滚动列表，全部待处理路径都会以完整路径展示，不再只预览前几项或使用截断路径。
- Inspector `最近执行` 在存在失败项时已新增可点击详情入口，完整失败路径、失败原因和处理建议统一收口到详情卡片。
- 外层删除失败 banner 已移除对首条原始失败原因的直接拼接，避免在 Inspector 外层继续显示被截断的失败文本。
- 已为上述改动补回归测试：
  - cleanup 确认窗列出全部目标
  - 最近执行失败详情入口
  - 失败详情中的完整路径 / 原因 / 建议
  - 外层失败反馈不再泄露原始失败原因
- 本轮验证：
  - `cargo fmt`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo build`：通过
  - `cargo run -p dirotter-app`：应用已启动，因 GUI 常驻运行在超时后终止

## 2026-04-04（失败详情窗重设计与删除进度回传）
- 参考 `web-design-guidelines` skill 的最新规则，重新收口了失败详情窗：
  - 窗口宽度进一步受控，避免内容卡超出屏幕
  - 关闭动作上移到顶部，避免滚到底部才能关闭
  - 每个失败项改为整行全宽卡片，路径文本强制换行，不再按内容宽度把卡片撑爆
- 失败原因展示已改为“本地化失败标题 + 本地化解释 + 建议 + 可选技术细节”，不再把英文原始错误串当主文案。
- 删除后台线程现已逐项上报进度并主动请求重绘，Inspector 后台任务卡和顶部横幅会持续显示：
  - 已处理数量
  - 成功 / 失败数量
  - 当前处理路径
- 本轮验证：
  - `cargo test -p dirotter-actions`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo test --workspace`：通过

## 2026-04-04（失败详情多语言补齐）
- 已补齐失败详情按钮、标题、说明、失败卡片标题、建议标题，以及 `view_models` 中关联的快速清理/选择操作文案，覆盖全部已支持语言，不再只修补法语/西语。
- `i18n` 现已对拆分生成字典缺失的键增加补丁层与 legacy fallback，避免 `view_models.rs` 中的新键在部分语言下直接回退成英文。
- 已新增并保留 `view_models.rs` 英文键覆盖测试，用来持续卡住后续多语言漏翻。
- 本轮验证：
  - `cargo fmt`：通过
  - `cargo test -p dirotter-ui`：通过

## 2026-04-04（删除后结果同步迁出 UI 主线程）
- 重新定位“一键提速删除时窗口变成 Not Responding”的根因后，已确认问题不在删除执行线程本身，而在删除完成后的结果同步仍压在 `egui::App::update()` 主线程：
  - 旧实现会在主线程同步重建 `NodeStore`
  - 同步重算 cleanup analysis，而这一步会再次大量访问文件系统元数据
  - 同步刷新结果排行与 diagnostics
- 参考 `egui` 官方最新异步建议，现已把删除完成后的结果同步拆成单独后台阶段：删除线程结束后，UI 进入 `结果同步中`，后台线程重建结果视图、清理建议和相关摘要，完成后再一次性回填界面状态。
- Inspector 后台任务卡和顶部横幅新增了独立的 `结果同步中 / Sync Results` 阶段，避免删除完成后主线程长时间卡死并被 Windows 判定为 `Not Responding`。
- 已新增回归测试，覆盖删除完成后进入后台结果同步阶段时的 Inspector 展示模型。
- 本轮验证：
  - `cargo fmt`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo build`：通过
  - `dirotter-app`：已重编译并重新启动验证

## 2026-04-04（结果视图按需载入迁出 UI 主线程）
- 进一步复盘后又发现第二个阻塞点：`结果视图` 页面在进入时仍会同步执行 `ensure_store_loaded_from_cache()`，直接在 UI 线程完成：
  - SQLite 读取快照
  - zstd 解压
  - `NodeStore` 反序列化
  - 排行与 cleanup analysis 重建
- 这条链路在删除/结果同步期间点击 `结果视图` 时会再次触发，并重新把界面拖进 `Not Responding`。
- 现已把结果视图的按需快照载入也改成后台 session：
  - 删除或结果同步进行中且 `store` 不在内存时，结果视图会显示等待同步提示，不再同步读快照
  - 非删除场景下，结果视图需要恢复快照时会先进入后台加载态，准备完成后再一次性落回 UI
- 已新增回归测试：
  - 删除同步期间禁止启动结果快照载入
  - 结果视图恢复快照时必须使用后台 session，而不是同步塞住 UI 线程
  - `result_pages.rs` 新增文案已纳入全语言翻译覆盖测试
- 本轮验证：
  - `cargo fmt`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo build`：通过
  - `dirotter-app`：已重编译并重新启动验证

## 2026-04-03（整体工程评估、计划落地与质量收口）
- 已完成一轮针对 workspace、核心 crate、扫描链路、平台层与 UI 主体的整体质量评估。
- 结论已从“继续加功能”切换为“先做质量收口和架构减债”，并识别出三条主线：
  - 拆解 `dirotter-ui`
  - 让扫描快照链路增量化
  - 收口 Rust 工程质量门槛
- 已新增 `docs/engineering-improvement-plan-2026-04.md`，系统化整理改进计划与实施计划。
- 已修复当前 `clippy -D warnings` 暴露的问题，并顺手修正：
  - 扫描发送阻塞时间采样实现
  - 删除计划中的路径类型判断
- 已继续修复 `dirotter-actions` 中被 `clippy` 继续挖出的类型复杂度、重复分支和 `io::Error::other` 风格问题。
- 已清理 `dirotter-ui` 生成翻译文件中的隐形字符，并修复两处 UI 层 clippy 告警。
- 已同步更新 `task_plan.md`、`findings.md`、`docs/dirotter-comprehensive-assessment.md`。
- 本轮最终工程复验已完成：
  - `cargo fmt --all`：通过
  - `cargo build --workspace`：通过
  - `cargo test --workspace`：通过
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过

## 2026-04-03（Phase 1：cleanup analysis 模块拆分）
- 已开始执行 UI 模块拆分的第一阶段，优先从 `cleanup analysis` 下手。
- 新增 `crates/dirotter-ui/src/cleanup.rs`，将以下纯规则/纯分析逻辑移出 `lib.rs`：
  - 清理分类
  - 风险判断
  - 候选评分
  - Top-N 收口
  - 清理分析生成
  - 缓存快清 eligibility 判断
- `DirOtterNativeApp` 保留了薄包装方法，现有调用点和 UI 测试无需大改。
- 已完成回归验证：
  - `cargo test -p dirotter-ui`：通过
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
  - `cargo test --workspace`：通过
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过

## 2026-04-03（Phase 1：controller 模块拆分）
- 继续推进 UI 模块拆分，新增 `crates/dirotter-ui/src/controller.rs`。
- 已将以下后台任务/controller 逻辑从 `lib.rs` 抽离：
  - 删除后台执行 session
  - 删除 relay 状态
  - 内存释放后台 session
  - 后台线程启动与完成结果提取
- `lib.rs` 现在保留的是：
  - UI 状态切换
  - 删除结果落回应用状态后的消费逻辑
  - 页面渲染中对 snapshot 的读取
- 本轮验证结果：
  - `cargo test -p dirotter-ui`：通过
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
  - `cargo test --workspace`：通过
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过

## 2026-02-16
- 已读取并分析参考文档。
- 已建立持久化执行文件：`task_plan.md`、`findings.md`、`progress.md`。
- 下一步：创建增强版 Devin 化目录与核心文件内容。
- 已创建 `.github` 核心文件：`copilot-agent.md`、`devin-loop.md`、`task-queue.md`、`project-context.md`。
- 当前进入下一阶段：创建 memory、instructions、checklists 与 VS Code 配置。
- 已完成 memory 子系统：`goals.md`、`decisions.md`、`progress.md`、`bugs.md`、`lessons.md`。
- 已完成 instructions 子系统：`autonomous.md`、`planning.md`、`execution.md`、`debugging.md`、`testing.md`、`refactor.md`、`git.md`、`quality-gate.md`、`resume.md`。
- 已完成 checklists：`definition-of-done.md`、`recovery.md`。
- 已完成 `.vscode/settings.json` 配置。
- 已完成 `README.md` 与 `docs/autonomous-workflow.md` 使用说明。

## 2026-03-09
- 已完成仓库级审计，识别出路径引用不一致、缺少最小自检入口、首次接入指引不足三类高优先级问题。
- 已新增 `scripts/validate-template.ps1`，可验证模板关键文件、任务队列状态和文档入口是否一致。
- 已新增 `.github/workflows/template-validation.yml`，为模板仓库补上最小 CI 闭环。
- 已新增 `docs/quickstart.md`，明确首次接入和每轮收尾动作。
- 已修正 `README.md`、`docs/autonomous-workflow.md`、`docs/engineering-requirements.md` 中与实际仓库结构相关的说明。

## 2026-03-16
- 完成扫描并发升级、完成态卡死修复、删除流程收口、品牌升级与 DirOtter 主题落地。
- 完成 Inspector 内真实删除、永久删除确认、Windows 回收站二次校验、目录上下文最大文件榜单和删除后局部刷新。
- 完成中文字体回退、人类可读格式化、盘符快捷扫描、默认根路径选择、后台删除任务提示。
- 更新 `README.md`、`docs/dirotter-comprehensive-assessment.md`、`docs/dirotter-ui-component-spec.md`、`docs/dirotter-install-usage.md`、`docs/dirotter-sdd.md`、`docs/quickstart.md`、`task_plan.md`、`findings.md`。

## 2026-03-17（页面级布局系统与文档同步）
- 将标题状态由英文字符串改为内部 `AppStatus` 枚举，并按当前语言实时渲染，修复中文界面状态胶囊未本地化的问题。
- 将 `Overview / Live Scan / History / Errors / Diagnostics / Settings` 切换为页面级纵向滚动，避免内容继续被内部固定高度区域截断。
- 首页与实时扫描页的主区从固定高度主卡与自然高度列，重构为显式两列布局和自然高度流式内容。
- 文件/文件夹 Top-N 榜单移除内层固定高度滚动盒，改由页面整体滚动承载。
- `with_page_width` 改为显式计算对称左右 gutter，并预留滚动条空间，减少中央内容区左右留白不一致的问题。
- 执行最终复验：`cargo check --workspace` 与 `cargo test --workspace` 均通过。
- 再次同步更新 `README.md`、`task_plan.md`、`findings.md`、`progress.md`、`docs/dirotter-comprehensive-assessment.md`、`docs/dirotter-ui-component-spec.md`、`docs/dirotter-install-usage.md`、`docs/dirotter-sdd.md`、`docs/quickstart.md`。

## 2026-03-18（扫描体验优化）
- 在 `dirotter-scan` 中新增 `ScanMode`，正式建立 `快速扫描（推荐）/ 深度扫描 / 超大硬盘模式` 三档用户预设。
- 用 `ScanConfig::for_mode(...)` 统一模式到内部 `profile / batch / snapshot / 并发阈值` 的映射，保留底层调优能力但不再在 UI 暴露。
- 将扫描目标卡从 `SSD / HDD / Network + batch / snapshot` 改为模式化选择，并补充“完整扫描不变，仅调整节奏和刷新方式”的说明。
- 将扫描模式持久化到本地设置，保持下次启动体验一致。
- 补充扫描层单元测试与集成测试，覆盖模式设置 round-trip、默认模式和三档模式可完成扫描。
- 同步更新 `README.md`、`task_plan.md`、`findings.md`、`progress.md`、`docs/dirotter-comprehensive-assessment.md`、`docs/dirotter-ui-component-spec.md`、`docs/dirotter-install-usage.md`、`docs/dirotter-sdd.md`、`docs/quickstart.md`。
- 已完成最终工程复验：`cargo fmt --all` 与 `cargo test --workspace` 均通过。

## 2026-03-18（结果视图轻量化）
- 新增独立 `Result View` 页面，替代对实时 treemap 的继续投入。
- 结果页不再参与实时扫描刷新，只在扫描完成或读取到缓存快照后显示。
- 结果页只展示当前目录的直接子项，并提供逐层下钻、返回上级和跳到当前选中目录。
- 扫描完成后会把最终 `NodeStore` 快照写入 SQLite，保证结果页和后续启动可以复用。
- 补充 UI 单测，验证结果页只返回直接子项且缺失焦点时会回退到根目录。
- 同步更新 README、UI 规格、系统设计、安装指南、快速上手与工作记录。

## 2026-03-19（结果视图版式修正）
- 将 `Result View` 页面从外层自然高度滚动改为 fill-height 页面布局，避免主结果区只占一小块高度。
- 将“目录结果条形图”卡片改为剩余高度填充型主浏览区，并把长列表滚动收口到卡片内部。
- 条目列表切换为 `show_rows` 渲染，既填满空间，也为大目录保留稳定滚动性能。
- 同步更新 README、任务计划、工作记录、综合评估、系统设计、UI 规格、安装指南和快速上手。

## 2026-03-19（清理建议系统 V1）
- 新增规则驱动的清理分析层，基于扫描完成后的 `NodeStore` 对缓存、下载、视频、压缩包、安装包、图片、系统文件做分类。
- 新增风险分级与评分逻辑，将建议区分为 `可清理 / 谨慎 / 禁删`，并把系统路径明确拦截。
- 在 Overview 顶部新增 `清理建议` 卡，优先展示可释放空间和分类摘要。
- 新增 `查看详情` 详情窗，支持按分类查看条目、默认勾选安全项并联动 Inspector。
- 新增 `一键清理缓存（推荐）` 确认流，默认走回收站，不复用永久删除路径。
- 将批量清理与单项删除统一收口到同一条删除执行链路，并在删除后同步刷新概览统计、建议和结果视图。
- 补充 `dirotter-ui` 单测，覆盖建议聚合与风险规则。

## 2026-04-04（运行验证）
- 执行 `cargo build`：成功，workspace 编译通过。
- 尝试运行 `cargo run -p dirotter-app -- --help`：应用启动成功并初始化 telemetry，进程保持运行（这表明 GUI/主程序已启动，而不是立即退出）。

## 2026-03-19（首页 / 设置页截断修正）
- 为页面级滚动容器补充底部安全区，避免最后一张卡片紧贴底边产生截断观感。
- Settings 页移除额外固定宽度容器，直接使用页面级宽度约束，修复右侧卡片贴边问题。
- 首页清理建议卡与扫描目标卡做轻量压缩，降低首屏拥挤度。
- 继续追根后，确认真正根因是卡片描边被紧凑子布局的 clip rect 裁掉；现已统一改为放宽 `clip rect` 后再绘制。
- 继续向前推进后，直接重构 Overview / Settings 为纵向章节布局，不再依赖并排双列卡片矩阵。
- 参考 Fluent Layout、GitLab Pajamas Settings Management、Carbon Dashboard hierarchy，把 Settings 重做为窄内容列的分组设置页。

## 2026-03-19（Overview 仪表盘重排）
- 继续调整 Overview，但不再把 Settings 的章节样式直接套到首页。
- 首页现改为更接近主流 dashboard 的信息层次：Hero 结论区、KPI 指标条、双列操作区、双列证据区。
- Hero 区顶部直接展示“现在最值得做什么”，并把 `一键清理缓存（推荐）` / `查看建议详情` 放到首屏动作区。
- 新增首页 KPI 指标条，固定展示文件数、目录数、扫描体积和错误数。
- `扫描设置` 与 `卷空间摘要` 恢复为并列主操作区，`最大文件夹` 与 `最大文件` 恢复为并列证据区。
- 清理掉旧的 `render_cleanup_suggestions_card` 路径，避免首页同时保留两套信息架构。
- 已完成工程复验：`cargo fmt --all`、`cargo test -p dirotter-ui`、`cargo build -p dirotter-app` 全部通过。

## 2026-03-19（Overview 首页重做二次修正）
- 继续修正用户指出的三个问题：`卷空间摘要` 漂移重叠、左右留白不对称、首页仍残留设置页组件语义。
- Overview 现已使用独立的首页最大宽度约束，不再沿用其他内容页的通用宽度。
- 首页双列区域从通用 helper 改为显式列宽分配，避免在 Inspector 存在时卡片互相侵入。
- `扫描设置` 与 `卷空间摘要` 已去掉 `settings_row` 结构，改为首页专用卡片布局。
- 首页继续保持四语言兼容；`cargo test -p dirotter-ui` 中的法语 / 西班牙语覆盖测试已通过。
- 已重新执行 `cargo fmt --all`、`cargo test -p dirotter-ui`、`cargo build -p dirotter-app`，并重新启动桌面应用。

## 2026-03-19（首页 gutter 与清理详情窗修正）
- 页面宽度算法从 `available - 24` 调整为显式侧边 gutter token，修正首页左右留白过小且不稳定的问题。
- `清理建议详情` 改为居中受控对话框，补齐标题栏关闭入口与底部关闭按钮。
- 详情列表行新增固定右侧大小列，避免长路径把大小单位挤出窗口。
- 已重新执行 `cargo fmt --all`、`cargo test -p dirotter-ui`、`cargo build -p dirotter-app`，并重新启动桌面应用。
- 后续根据截图继续收窄 Overview 专用宽度到 `1160`，并把页面 gutter 提高到 `64`，优先让首页在 Inspector 存在时保持肉眼可见的左右对称。
- 继续追根后，页面宽度容器从“左右 spacer 拼接”改为真正的居中内容列，避免内容块自身扩张后视觉中心漂移。
- 再次根据截图修正 KPI 指标条：四张卡片现在会强制吃满各自列宽，不再因内容宽度较小而缩在左边。

## 2026-03-19（Overview 去重、停止扫描收尾与缓存瘦身）
- Overview 再次重做信息架构：保留 Hero 和四张唯一指标卡，移除与其重复的 `卷空间摘要` 大卡。
- 首页四张 KPI 改为 `磁盘已用 / 磁盘可用 / 已扫描体积 / 错误`，卷级补充信息并入全宽 `开始扫描` 卡底部的紧凑状态条。
- Overview 中段从“双列操作区”收口为单张全宽扫描卡，底部保留双列 `最大文件夹 / 最大文件` 证据区。
- 停止扫描已修正为真正可退出流程：worker 在条件变量等待时会短超时轮询取消标记，工具栏进入 `Stopping` 禁用态，取消后不再错误落成完成态历史。
- SQLite 快照改为“每个根路径只保留最新一份”，并在快照写入后主动 checkpoint WAL，避免重复扫描同一路径时 `dirotter.db` 相关文件持续线性膨胀。
- 新增 `dirotter-cache` 单测，验证同一路径快照会覆盖旧快照；全量验证 `cargo fmt --all`、`cargo test --workspace`、`cargo build -p dirotter-app` 已通过。

## 2026-03-19（极速缓存清理与低层永久删除）
- `一键清理缓存` 改为专用执行模式：先把安全缓存项移动到同卷 `.dirotter-staging`，再由后台线程继续永久清除。
- 清理确认窗、反馈文案、Inspector 后台任务说明已同步改成“快速移出 + 后台释放空间”，不再误导为“先进入回收站”。
- Windows 文件永久删除优先尝试低层句柄删除 fast path，失败后回退到现有文件系统删除。
- 应用启动时会异步继续清理各卷 `.dirotter-staging` 遗留项；扫描器也会跳过该内部目录，避免把应用自身暂存区再扫进结果里。
- 新增 `dirotter-platform` 与 `dirotter-actions` 测试，覆盖 staging round-trip 和 fast purge 快速返回。
- 继续根据交互反馈补齐入口：`清理建议详情` 新增 `全选安全项 / 清空所选 / 打开所选位置 / 快速清理选中缓存`，Inspector 在选中安全缓存项时也会直接显示 `快速清理缓存`。

## 2026-03-20（首页扫描卡入口降噪）
- 删除首页 `开始扫描` 卡标题下方的解释性说明文案，避免继续把实现背景当成首页主信息。
- 将 `快速盘符` 提前为扫描卡第一操作区，保持 Windows 用户以点击盘符为主的扫描路径。
- 手动目录输入框改为盘符区后的次级控件，并收窄为较小宽度，只有在扫描子目录时才需要显式使用。

## 2026-03-20（清理详情顶部删除入口）
- `清理建议详情` 顶部操作条新增直接删除入口，不再只保留“全选 / 清空 / 打开位置”。
- 缓存分类在顶部直接显示 `快速清理选中缓存`，其他分类显示 `移到回收站`。
- 顶部同时补上 `永久删除`，避免用户必须滚到长列表底部才看到主删除动作。

## 2026-03-20（红色高风险项提示改写）
- 将红色高风险标签从 `禁删 / Blocked` 改为 `手动处理 / Manual Review`，避免只给出无操作意义的状态词。
- 系统文件原因说明与分类横幅都改为明确引导：点击条目后使用 `打开所选位置`，再自行确认是否手动处理。
- 法语与西语词典已同步更新，保证 `中文 / English / Français / Español` 四语言下提示一致。

## 2026-03-20（扫描完成收尾改为后台整理）
- 将扫描完成后的重收尾从 UI 线程拆出，新增显式 `整理结果中 / Finalizing` 阶段。
- 目录遍历结束后，最终快照压缩落库、历史写入、错误导出和清理建议汇总改由后台线程完成。
- UI 先保持可响应，再在后台整理结束后切到真正的 `完成 / Completed` 状态，避免最后一拍点击即无响应。

## 2026-03-20（产品聚焦复盘）
- 重新按用户目标审视当前主链路，结论是：默认主路径应聚焦“释放空间”，而不是“积累扫描历史和诊断资产”。
- 已把下一阶段方案收口为四件事：
  - 默认取消自动快照/历史/错误 CSV
  - 重构清理建议为更轻的可执行候选生成
  - 收口主导航，只保留与清理空间强相关的页面
  - 谨慎处理“释放内存”诉求，避免做成误导性卖点

## 2026-03-20（产品聚焦四阶段全部实施）
- 完成扫描默认重收尾已移除：不再在扫描结束时自动保存快照、写扫描历史、导出错误 CSV。
- 扫描完成态现在只保留轻量 `整理结果中 / Finalizing` 收尾，然后直接进入可操作结果页。
- `History / Errors / Diagnostics` 已从主导航降级，只有在 Settings 中开启 `高级工具 / Advanced Tools` 后才显示。
- 启动时如果未开启高级工具，不再默认加载历史或快照，进一步减少无关启动成本。
- 诊断页新增按需维护动作：`手动保存当前快照 / 手动记录扫描摘要 / 手动导出错误 CSV`。
- 清理建议生成已从“全树全量候选”收口为“规则命中 + 每类 Top-N + 全局上限”，优先返回真正可执行的候选。
- 首页主路径已收口为 `一键提速（推荐）`，会在“开始提速扫描 / 一键提速（推荐） / 查看提速建议”之间自动选择最合适的下一步。
- Inspector 已移除技术性维护动作，只保留普通用户更容易理解的文件操作。
- `优化 DirOtter 内存占用` 会先保存当前结果快照，再主动释放当前会话内重结果树与相关状态，并在 Windows 上请求收缩工作集。
- `dirotter-core::NodeStore` 已进一步做 Rust 级瘦身：常驻节点不再重复持有 `name/path String`，而是只保留 intern 后的字符串 ID；结构体体积对比测试显示 `Node` 从旧布局等效的 `128 bytes` 降到 `80 bytes`。
- 新增 Windows 进程/系统内存采样：系统可用内存、内存负载与 DirOtter 工作集现在集中显示在右侧 Inspector 的内存状态卡中。
- 新增低内存压力自动维护：当应用空闲且系统内存紧张时，会优先把结果树转成磁盘快照并释放内存，结果视图在需要时再自动回载。
- Diagnostics 新增恢复型维护动作 `清理异常中断的临时删除区`，用于处理上次缓存快清异常中断后遗留的内部待删内容。
- 法语与西班牙语词典已同步补齐新增的高级工具、维护动作和完成态文案，四语言覆盖测试继续通过。
- 已完成最终工程复验：`cargo fmt --all`、`cargo test --workspace`、`cargo build -p dirotter-app` 全部通过。

## 2026-03-31（首页主动作显性化与结果视图卡顿修复）
- Overview 顶部 Hero 已重排为真正的 `一键提速（推荐）` 首卡：主动作标题、说明和主按钮现在会直接出现在首屏，而不是埋在次级说明区里。
- 空白初始态下，首页主卡本身也已经改成 `一键提速（推荐）` 入口，普通用户首屏不再先看到旧的“开始扫描”语义。
- 扫描期的实时快照不再在 UI 线程里逐条并入 `NodeStore`；实时页只保留排行榜和摘要，完整结果树继续等扫描完成后一次性交付。
- Result View 在扫描进行中继续保持“完成后再看”的轻量提示，不再因为实时快照合并把窗口拖入 `Not Responding`。
- Result View 不再自动载入历史旧缓存；只有当前会话已经有结果摘要时，才允许按需回载对应快照，避免空白态点击时把旧大结果重新拖进内存。
- 新增 UI 回归测试 `live_snapshot_updates_rankings_without_building_store_on_ui_thread`，锁住“更新实时榜单但不物化整树”的行为。
- 新增 UI 回归测试 `result_view_only_reloads_cache_for_current_session_results`，锁住“结果页不自动载入旧缓存”的边界。
- 本轮修复后已重新执行 `cargo fmt --all`、`cargo test --workspace`、`cargo build -p dirotter-app`，并重新启动最新桌面应用。

## 2026-04-01（内存入口回归右侧与删除卡顿修复）
- 右侧 Inspector 的 `Quick Actions` 已新增独立 `一键释放内存 / Release Memory`，不再把“提速”语义和扫描入口混在一起。
- 扫描卡已恢复为纯扫描语义：首页扫描入口再次只负责磁盘扫描，说明文案明确把内存释放指向右侧独立入口。
- 删除完成后的结果同步已从“每个成功目标都 clone 一整份 `NodeStore` 再重建”改成单次批量更新，避免多目标清理时 UI 锁死和内存瞬时翻倍。
- `NodeStore` 的完整路径已改成共享分配，不再同时在节点和索引里各自保留独立副本；`compact_node_bytes` 目前为 `88`，旧等效重字符串布局为 `120`。
- 法语 / 西班牙语词典已补齐新增的内存释放与扫描解耦文案。
- 已重新执行 `cargo fmt --all`、`cargo test --workspace`、`cargo build -p dirotter-app`，通过后重新启动最新桌面应用。

## 2026-04-01（系统内存释放改造）
- 右侧 Quick Actions 的内存入口已升级为 `一键释放系统内存 / Release System Memory`，语义不再停留在“只释放 DirOtter 自己”。
- `dirotter-platform` 新增 `release_system_memory()`：会先读取系统内存状态，再尝试收缩当前进程、当前交互会话中的高占用进程，并在权限允许时裁剪系统文件缓存，最后回传 before/after 报告。
- 内存释放已改成后台线程执行；UI 只负责发起和接收结果，避免点击后把主线程拖进 `Not Responding`。
- Diagnostics JSON 现已包含最近一次系统内存释放报告，方便对照系统可用内存变化。
- 法语 / 西班牙语词典已同步补齐 `Release System Memory`、系统缓存裁剪和 before/after 反馈等新增文案。
- 已再次执行 `cargo fmt --all`、`cargo test --workspace`，全部通过。

## 2026-03-19（法语 / 西班牙语支持）
- 在 `dirotter-ui` 中新增 `Fr / Es` 语言枚举、设置持久化解析与系统语言自动检测，覆盖 `zh / fr / es / en`。
- Settings 页语言切换扩展为 `中文 / English / Français / Español` 四选项。
- 采用“英文键 -> 法语 / 西班牙语词典”的方式扩展现有本地化层，避免重写全量 `self.t(zh, en)` 调用点。
- 已将法语 / 西班牙语从“核心词汇覆盖”补齐为完整 UI 版本，不再依赖英文说明文案回退。
- 新增源码级完整性测试：自动提取当前 `self.t(...)` 英文键，并校验法语 / 西班牙语词典覆盖。
- 补充 UI 单测，覆盖语言设置 round-trip、区域设置检测和核心操作文案翻译。
- 已完成工程复验：`cargo fmt --all`、`cargo check --workspace`、`cargo build --workspace`、`cargo test --workspace` 全部通过。
- 已同步更新 `README.md`、`docs/dirotter-install-usage.md`、`docs/quickstart.md`、`docs/dirotter-ui-component-spec.md`、`docs/dirotter-comprehensive-assessment.md`、`docs/dirotter-sdd.md`、`task_plan.md`、`findings.md`、`progress.md`。

## 2026-04-03（Phase 1：dashboard 页面模块拆分）
- 继续推进 `dirotter-ui` 模块化，新增 `crates/dirotter-ui/src/dashboard.rs` 与 `crates/dirotter-ui/src/dashboard_impl.rs`。
- 已将 `ui_dashboard`、`render_overview_hero`、`render_live_overview_hero`、`render_overview_metrics_strip`、`render_scan_target_card` 从 `lib.rs` 抽离到独立页面模块。
- `lib.rs` 现在只保留 `ui_dashboard` 入口转发，页面渲染细节不再继续堆在主文件里。
- 本轮拆分过程中发现新模块文案复制出现编码污染，已改为基于原始 `lib.rs` 块生成实现并收回到 `src/` 下，最终代码状态已恢复干净可维护。
- 已完成验证：`cargo fmt --all`、`cargo test -p dirotter-ui`、`cargo clippy -p dirotter-ui --all-targets -- -D warnings`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过。

## 2026-04-03（Phase 1：current_scan / treemap 页面模块拆分）
- 继续推进页面层拆分，新增 `crates/dirotter-ui/src/result_pages.rs`。
- 已将 `ui_current_scan` 与 `ui_treemap` 从 `lib.rs` 抽离到独立页面模块，`lib.rs` 仅保留薄转发入口。
- 本轮拆分覆盖的页面职责包括：实时扫描概览、实时 Top-N 列表、最近扫描文件表，以及结果视图的轻量层级浏览与目录条形图。
- 该拆分验证了 `dirotter-ui` 页面层可以直接复用现有私有 helper 和状态方法，不需要先补一层额外抽象。
- 已完成验证：`cargo fmt --all`、`cargo test -p dirotter-ui`、`cargo clippy -p dirotter-ui --all-targets -- -D warnings`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过。

## 2026-04-03（Phase 1：advanced / settings 页面模块拆分）
- 继续推进页面层拆分，新增 `crates/dirotter-ui/src/settings_pages.rs` 与 `crates/dirotter-ui/src/advanced_pages.rs`。
- 已将 `ui_diagnostics`、`ui_settings`、`ui_history`、`ui_errors` 从 `lib.rs` 抽离到独立页面模块，`lib.rs` 继续收口为状态协调和入口转发。
- 至此，`dirotter-ui` 的主要页面层已基本全部脱离主文件：`dashboard`、`current_scan`、`treemap`、`history`、`errors`、`diagnostics`、`settings` 均已模块化。
- 这轮拆分再次证明现有页面 helper 的可见性边界足够支撑继续模块化，暂时不需要为了拆分而先补一层额外抽象。
- 已完成验证：`cargo fmt --all`、`cargo test -p dirotter-ui`、`cargo clippy -p dirotter-ui --all-targets -- -D warnings`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过。

## 2026-04-03（下一层：扫描快照链路优化）
- 已开始从 UI 模块拆分转入核心扫描成本优化。
- `crates/dirotter-core/src/lib.rs` 中的 `dirty` 标记已真正参与计算：
  - `mark_dirty()` 现在会向祖先传播，而不是只标脏当前节点。
  - `rollup()` 改为只重算 dirty 节点，避免每次快照都全量遍历整棵树。
  - `top_n_largest_files()` / `largest_dirs()` 改为固定容量候选堆，避免每次快照都为全量节点建大堆。
- `crates/dirotter-cache/src/lib.rs` 中的 `save_snapshot()` 已改为原子事务替换，并去掉每次保存后的强制 `wal_checkpoint(TRUNCATE)`。
- 已为 `dirotter-core` 补充 dirty 传播和增量 rollup 的回归测试。
- 已完成验证：`cargo fmt --all`、`cargo test -p dirotter-core`、`cargo test -p dirotter-scan`、`cargo test -p dirotter-cache`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 全部通过。

## 2026-04-03（下一层：entry-time 聚合维护）
- 继续深入扫描快照链路，不再满足于“snapshot 时少算一点”，而是把聚合维护前移到 `add_node()`。
- `crates/dirotter-core/src/lib.rs` 中，`NodeStore::add_node()` 现在会在节点插入时即时更新祖先的：
  - `size_subtree`
  - `file_count`
  - `dir_count`
- `crates/dirotter-scan/src/aggregator.rs` 中，`make_snapshot_data()` 已移除先 `rollup()` 再取数的路径，改为直接清脏并读取当前聚合结果。
- `aggregator` 的 `top_files_delta / top_dirs_delta` 也改为直接从命中节点导出，不再先把路径字符串组出来再反查 `NodeId`。
- 已补充回归测试，覆盖：
  - `add_node` 的祖先聚合值即时更新
  - `aggregator` 不依赖额外 rollup 也能生成正确 snapshot 排行
- 本轮验证结果：
  - `cargo fmt --all`：通过
  - `cargo test -p dirotter-core`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo clippy -p dirotter-core --all-targets -- -D warnings`：通过
  - `cargo clippy -p dirotter-scan --all-targets -- -D warnings`：通过

## 2026-04-03（Phase 3：扫描消息链路共享路径化）
- 已开始推进“少拷贝数据流”这一层，不再只优化 snapshot 算法。
- `crates/dirotter-scan/src/walker.rs` 中，`EntryEvent` 的 `path / parent_path / name` 已改为共享 `Arc<str>`。
- `crates/dirotter-scan/src/lib.rs` 与 [publisher.rs](E:/DirForge/crates/dirotter-scan/src/publisher.rs) 中，`BatchEntry.path` 和 `Publisher.frontier` 也已改为共享路径。
- `crates/dirotter-scan/src/aggregator.rs` 中，`pending_by_parent` 和 `root_path` 已同步切到共享路径键，避免等待父目录时继续复制同一串路径。
- 这意味着 `walker -> aggregator -> publisher` 的热路径已从“层层 owned String”改成“内部共享、边界物化”：
  - 扫描线程内部共享路径
  - 只有进度事件发给 UI 时才物化当前路径字符串
  - UI 批量文件榜单也只在真正落入 `live_files` 时才转 `String`
- 已完成验证：
  - `cargo fmt --all`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo test -p dirotter-ui`：通过

## 2026-04-03（Phase 3：共享路径推进到事件边界）
- 继续推进“内部共享、边界物化”的方向，不再只停在 `walker -> publisher` 内部。
- `crates/dirotter-scan/src/lib.rs` 中：
  - `ScanProgress.current_path` 已改为 `Option<Arc<str>>`
  - `SnapshotView.top_files / top_dirs` 已改为共享路径排行
  - `ScanEvent::Finished` 的 `top_files / top_dirs` 也已改为共享路径排行
- `crates/dirotter-scan/src/publisher.rs` 不再在发送进度事件时把 `frontier` 路径提前 `to_string()`。
- `crates/dirotter-scan/src/aggregator.rs` 的实时 Top-N 现在直接复用节点共享路径，不再为 snapshot 组装额外字符串。
- `crates/dirotter-ui/src/lib.rs` 中新增统一物化入口：只有在 UI 真实接管 `current_path` 和排行列表时，才把共享路径转成 `String`。
- 本轮验证结果：
  - `cargo fmt --all`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo clippy -p dirotter-scan --all-targets -- -D warnings`：通过
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过

## 2026-04-03（Phase 3：UI 排行与进度状态共享化）
- 共享路径不再只停在扫描事件边界，已经继续进入 `dirotter-ui` 的实时状态层。
- `crates/dirotter-ui/src/lib.rs` 中：
  - `scan_current_path` 已改为 `Option<Arc<str>>`
  - `live_top_files / live_top_dirs / completed_top_files / completed_top_dirs` 已改为共享路径排行
  - 排行与路径只在真正返回给渲染 helper 时再 `to_string()`
- 这意味着实时扫描期间：
  - publisher 不再提早生成进度字符串
  - UI 接到排行后也不再立刻把整组 Top-N 复制成 `String`
  - 只有实际渲染或生成文本榜单时才做物化
- 本轮验证结果：
  - `cargo fmt --all`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过

## 2026-04-03（Phase 3：SnapshotView 节点共享化）
- 继续推进最后一块实时 snapshot 大 payload：`ResolvedNode` 已从字符串拥有型结构改为共享字符串结构。
- `crates/dirotter-core/src/lib.rs` 中：
  - `ResolvedNode.name / path` 已改为 `Arc<str>`
  - `resolved_node()` 现在会直接复用字符串池和节点路径的共享分配
  - `upsert_resolved_node()` 也已按共享字符串输入重建 `NodeStore`
- 这意味着 `SnapshotView.nodes` 不再为每个变更节点重新分配完整 `name/path String`，而是沿用已有共享数据。
- 本轮验证结果：
  - `cargo fmt --all`：通过
  - `cargo test -p dirotter-core`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo clippy -p dirotter-core --all-targets -- -D warnings`：通过

## 2026-04-04（Phase 3：去掉无用 snapshot 节点列表与重复 Top-N）
- 继续压缩实时快照和完成态 payload，不再只做“共享字符串但仍发送整块结构”。
- `crates/dirotter-scan/src/aggregator.rs` 中：
  - 非 full-tree 的 `SnapshotView` 已不再物化 `nodes`
  - 新增 `changed_node_count`，用于保留轻量统计而不是传完整节点列表
- `crates/dirotter-scan/src/lib.rs` 与 [publisher.rs](E:/DirForge/crates/dirotter-scan/src/publisher.rs) 中：
  - `ScanEvent::Finished` 已移除重复的 `top_files / top_dirs`
  - `record_scan_finished()` 现在直接用最终 `NodeStore` 的节点数，而不是依赖 snapshot 节点列表长度
- `crates/dirotter-ui/src/lib.rs` 中：
  - 最终完成态改为在拿到 `store` 后本地重建排名
  - 不再依赖完成事件里重复携带的 Top-N
- 本轮验证结果：
  - `cargo fmt --all`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo test -p dirotter-ui`：通过
  - `cargo clippy -p dirotter-scan --all-targets -- -D warnings`：通过
  - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过

## 2026-04-04（性能基线：snapshot payload 与组装耗时）
- 开始把“压 payload”从实现改动升级为回归门槛。
- `crates/dirotter-testkit/tests/benchmark_thresholds.rs` 新增了：
  - `benchmark_snapshot_payload_threshold_massive_tree`
- `crates/dirotter-testkit/perf/baseline.json` 新增了：
  - `snapshot_massive_tree_payload_bytes`
- `crates/dirotter-scan/src/aggregator.rs` 新增了：
  - `incremental_snapshot_generation_stays_under_threshold`
- 当前覆盖的是两类回归：
  - 公共扫描路径上的 snapshot 展示 payload 大小
  - `make_snapshot_data(false)` 本地组装耗时与序列化体积
- 本轮验证结果：
  - `cargo test -p dirotter-scan incremental_snapshot_generation_stays_under_threshold -- --nocapture`：通过
  - `cargo test -p dirotter-testkit benchmark_snapshot_payload_threshold_massive_tree -- --nocapture`：通过

## 2026-04-04（运行时观测：snapshot 稀疏 payload telemetry）
- 在不引入每次 snapshot 额外 JSON 序列化的前提下，继续补齐 live snapshot 的 runtime 可观测性。
- `crates/dirotter-telemetry/src/lib.rs` 中新增了：
  - `avg/max snapshot changed nodes`
  - `avg/max snapshot materialized nodes`
  - `avg snapshot ranked items`
  - `avg/max snapshot text bytes`
- `crates/dirotter-scan/src/lib.rs` 中新增轻量文本载荷估算，并在 periodic/final snapshot 两条路径上报 telemetry。
- 当前意义是：
  - diagnostics 里不再只能看到“snapshot 提交耗时”，还可以看到 snapshot 是否重新变胖
  - 后续继续压缩 payload 时，已经有真实运行态的观测锚点，而不只靠阈值测试
- 本轮验证结果：
  - `cargo test -p dirotter-telemetry`：通过
  - `cargo test -p dirotter-scan`：通过
  - `cargo build --workspace`：通过
  - `cargo test --workspace`：通过
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过

## 2026-04-04（SnapshotView 分层：live/full 显式化）
- `crates/dirotter-scan/src/lib.rs` 中的 `SnapshotView` 已从单一 struct 收口成显式分层：
  - `LiveSnapshotView`
  - `FullSnapshotView`
  - `SnapshotView::{Live, Full}`
- `crates/dirotter-scan/src/aggregator.rs` 现在会根据 `include_full_tree` 明确生成 `Live` 或 `Full`，而不是让 live 路径天然携带一个“可选 nodes 列表”。
- `crates/dirotter-ui/src/lib.rs` 的实时事件消费也已改成只提取排行，不再结构上默认依赖 `nodes`。
- 这一步的意义是把“轻量实时视图”和“重型全量视图”真正拆成类型边界，而不只是靠约定说 live 路径不要塞节点列表。

## 2026-04-04（UI 状态收口：当前结果树选择改为 NodeId 优先）
- `crates/dirotter-ui/src/lib.rs` 中：
  - `SelectedTarget` 已新增 `node_id`
  - `TreemapEntry` 已新增 `node_id`
  - 新增 `select_node()`，当前结果树内的交互会优先直接落到 `NodeId`
- `crates/dirotter-ui/src/cleanup.rs` 生成的 cleanup 候选也已携带节点 ID。
- `crates/dirotter-ui/src/result_pages.rs` 中，treemap 项点击和进入下一层都改成优先用 `NodeId`，不再只靠路径字符串反查。
- 当前意义是：
  - UI 仍保留路径 fallback，兼容错误页和外部路径
  - 但当前结果树这一层已经开始真正转向“ID 驱动、路径兜底”

## 2026-04-04（UI helper 下沉：view-model 物化脱离主状态文件）
- `crates/dirotter-ui/src/view_models.rs` 已新增并接管一组纯展示 helper：
  - `summary_cards`
  - `scan_health_summary / scan_health_short`
  - `current_ranked_dirs / current_ranked_files`
  - `contextual_ranked_files_panel`
- 当前变化不是“再拆一个文件”这么简单，而是把首页、结果页、状态栏使用的 view-model 物化逻辑从 `lib.rs` 的状态协调实现里拿了出去。
- 这样做的意义是：
  - `DirOtterNativeApp` 继续向“应用协调器”收口
  - 页面层读取的展示数据来源更集中
  - 后续如果要继续优化字符串物化点，会更容易在一个模块里集中处理

## 2026-04-04（字符串热点收口：排行与结果页共享路径化）
- `crates/dirotter-ui/src/view_models.rs` 中：
  - `current_ranked_dirs / current_ranked_files`
  - `ranked_files_in_scope`
  - `contextual_ranked_files_panel`
  已从 `Vec<(String, u64)>` 改为共享 `RankedPath`
- `crates/dirotter-ui/src/lib.rs` 中：
  - `live_files` 已改为共享路径列表
- `crates/dirotter-ui/src/lib.rs` 与 `result_pages.rs` 中：
  - `TreemapEntry.name / path` 已改为共享 `Arc<str>`
- 当前意义是：
  - 结果页和首页的大部分高频榜单不再先批量落成 `String`
  - 共享路径可以直接穿到渲染 helper，只有点击选中或局部文本格式化时才物化

## 2026-04-04（Inspector 与删除链路：SelectedTarget 共享化）
- `crates/dirotter-ui/src/lib.rs` 中的 `SelectedTarget.name / path` 已改为共享 `Arc<str>`。
- `crates/dirotter-ui/src/cleanup.rs` 生成的 cleanup 候选现在会直接复用 `NodeStore` 里的共享名称和路径。
- 删除执行计划仍在 [lib.rs](E:/DirForge/crates/dirotter-ui/src/lib.rs) 的执行边界显式转回 `String`，没有把 `Arc<str>` 强行推入 actions crate。
- 当前意义是：
  - Inspector、删除确认窗、cleanup 候选、treemap 目标不再各自重复复制路径和名称
  - 共享数据停留在 UI 内部，执行链路边界仍保持清楚

## 2026-04-04（UI 路径状态：cleanup 选择与 treemap 聚焦共享化）
- `crates/dirotter-ui/src/lib.rs` 中：
  - `CleanupPanelState.selected_paths` 已改为 `HashSet<Arc<str>>`
  - `treemap_focus_path` 已改为 `Option<Arc<str>>`
- cleanup 详情窗中的勾选状态、批量全选/清空和 treemap 的进入/返回上级逻辑，当前都直接复用共享路径，而不是在 UI 内部继续保留独立 `String` 状态。
- 当前意义是：
  - UI 里最后两块明显的“路径状态型 String”已经继续收口
  - Inspector、cleanup、treemap 三条交互链路现在更接近统一的共享路径模型

## 2026-04-04（Inspector / Confirm 展示整形继续下沉）
- `crates/dirotter-ui/src/view_models.rs` 现在已继续接管：
  - Inspector 目标摘要
  - 后台删除任务摘要
  - 永久删除确认窗摘要
  - cleanup 确认窗摘要
- `crates/dirotter-ui/src/lib.rs` 中对应的 `ui_inspector()`、`ui_delete_confirm_dialog()` 和 `ui_cleanup_delete_confirm_dialog()` 已改成主要负责布局和交互，展示字符串整形改由 view-model helper 提前完成。
- 当前意义是：
  - `DirOtterNativeApp` 继续摆脱“边渲染边拼文本”的模式
  - Inspector/确认窗这一层的展示边界也开始和首页/结果页一样进入集中式 view-model 管理

## 2026-04-04（Inspector 动作态与反馈文案也已下沉）
- `crates/dirotter-ui/src/view_models.rs` 中新增了：
  - Inspector 动作可用性模型
  - Explorer 反馈 banner 模型
  - 删除反馈 banner 模型
  - 最近执行摘要模型
- `crates/dirotter-ui/src/lib.rs` 中的 `ui_inspector()` 现在主要负责按钮点击后的动作分发和布局，不再自己计算：
  - 按钮是否可用
  - 当前应显示哪条提示文案
  - Explorer / 删除 / 最近执行的展示文本
- 当前意义是：
  - Inspector 已继续从“状态分支密集区”退成“消费 view-model 的渲染层”
  - 后续如果要继续改 Inspector 的提示策略或交互状态，不需要再在 UI 布局代码里穿插大量条件判断

## 2026-04-04（Inspector Memory Status 卡重做）
- Inspector 底部原先的 `Workspace Context` 已删除，改成系统内存状态卡。
- 新卡片只保留真正有用的信息：系统可用内存、内存负载、DirOtter 占用，以及最近一次系统内存释放带来的变化。
- Inspector 现已补上独立纵向滚动；释放后新增的反馈不会再把卡片底部信息顶出可视区。
- 原先占空间但无助于决策的长说明文案已移除，300px 窄栏内也不再使用容易横向溢出的 chip 布局。

## 2026-04-04（Cleanup Details Window 继续下沉）
- `crates/dirotter-ui/src/view_models.rs` 现在已新增 cleanup 详情窗对应的展示模型，覆盖：
  - 分类 tabs
  - 统计区与按钮标签/启用态
  - item 行路径、大小、风险/分类标签、unused days 与评分文案
- `crates/dirotter-ui/src/lib.rs` 中的 `ui_cleanup_details_window()` 已改成主要负责：
  - tab 切换
  - 勾选状态写回
  - 选择/打开/触发清理动作
  而不是自己拼大段展示文本。
- 当前意义是：
  - cleanup 详情窗也开始走和 Inspector 一样的“view-model 先整形，UI 再消费”的路线
  - `dirotter-ui` 里另一个较重的 UI 函数已经明显变薄

## 2026-04-04（Cleanup Details 控制流继续收口）
- `crates/dirotter-ui/src/lib.rs` 中的 `ui_cleanup_details_window()` 已从多布尔旗标控制流改成“收集 `CleanupDetailsAction` -> 统一分发”。
- 当前已被统一收口的动作包括：
  - 切换分类
  - 勾选/取消勾选目标
  - 聚焦目标
  - 全选安全项 / 清空所选 / 打开所选位置
  - 主操作触发 / 永久删除触发
- 当前意义是：
  - cleanup 详情窗主函数继续从“渲染 + 一堆尾部状态判断”退回“渲染 + 动作收集”
  - 后续如果要继续补测试或调整动作策略，可以直接围绕 action handler 做局部演进

## 2026-04-04（剩余确认窗控制流也已统一）
- `crates/dirotter-ui/src/lib.rs` 中：
  - `ui_delete_confirm_dialog()`
  - `ui_cleanup_delete_confirm_dialog()`
  现在都已改成“收集动作 -> handler 处理”的窗口模式。
- 当前这些确认窗不再直接在局部布局函数里混合维护确认态和执行态，而是把确认动作统一下沉到 handler。
- 当前意义是：
  - 窗口级控制流风格已经从单点优化扩展到剩余主要弹窗
  - `dirotter-ui` 里这批确认窗现在更适合继续补行为回归测试

## 2026-04-04（FastPurge 平台回退修复）
- 在回归验证中发现，`FastPurge` 之前默认把 staging 根放在卷根目录；当前环境下这会因为无权写入 `C:\\.dirotter-staging` 而失败。
- `crates/dirotter-platform/src/delete.rs` 现在已改成：
  - 先尝试卷根 staging
  - 权限/IO 失败时回退到源路径父目录下的 `.dirotter-staging`
  - 如果 staging rename 仍失败，再对源路径做立即删除兜底，保证快清语义仍然是“源路径立刻消失”
- 当前结果是：
  - `dirotter-platform` 的 `stage_and_purge_file_roundtrip` 已恢复通过
  - `dirotter-actions` 的 `fast_purge_stages_path_and_returns_quickly` 也已恢复通过
