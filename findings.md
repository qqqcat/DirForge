# Findings

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
