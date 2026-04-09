# DirOtter

DirOtter 是一个基于 Rust 的本地磁盘分析器，当前聚焦于“尽快帮用户释放空间”，并收口为以下主能力：

- 目录扫描与进度事件流
- 目录树聚合与 Top-N 查询
- 规则驱动的清理建议与安全缓存清理
- 重复文件候选识别
- Inspector 内删除执行（回收站 / 永久删除）
- 当前会话内的诊断与恢复能力

## 品牌说明

`DirOtter` combines `Dir` from directory and `Otter` for its clever, tidy, exploratory character: an analyzer that helps you dig through storage and make sense of your file tree.

- `Dir` 直接指向目录树、文件系统和磁盘结构
- `Otter` 强调聪明、灵活、善于整理与探索
- 整体品牌语气应更像“冷静的分析工具”，而不是“激进的垃圾清理器”

当前 UI 主题已按这套语义收口：

- 主品牌色：`River Teal` `#2F7F86`
- 深色基调：`Deep Slate` / `App background #11181C`
- 浅色基调：`Mist Gray` / `#E8EEF0`
- 轻暖辅助色：`Sand Accent` `#D8C6A5`

> 当前状态：**工程化验证阶段（Production Readiness）**  
> 目标状态：**生产级（Production）**

## 项目现状（2026-03-19）

基于当前代码、实测问题修复和全量回归测试，DirOtter 已具备从“扫描 -> 分析 -> 展示 -> 直接处理”的端到端主链路。

本轮综合评估结论：

- 扫描链路已并发化（worker + 聚合线程 + 有界发布队列），可稳定处理大目录、取消、错误和完成态。
- 删除动作已进入 Inspector，支持回收站删除、永久删除确认、后台任务提示、Windows 回收站二次校验，以及删除后的局部刷新。
- 桌面 UI 已从“控件级补丁”转向“页面级布局系统”：
  - 统一最大内容宽度
  - 对称 gutter
  - 页面级纵向滚动
  - 去除固定高度主卡和固定高度排行榜
  - 状态胶囊按当前语言实时本地化
- 启动时会优先选择系统盘/首个卷作为默认根路径，并提供盘符快捷按钮，点击即可直接扫描对应卷。
- 扫描入口已从 `SSD / HDD / Network + batch / snapshot` 收口为三档用户模式：
  - `快速扫描（推荐）`
  - `深度扫描`
  - `超大硬盘模式`
- Overview 已新增 `清理建议` 卡片，会在扫描完成后按规则汇总缓存、下载、视频、压缩包、安装包等候选，并区分 `可清理 / 谨慎 / 禁删`。
- 已新增 `一键清理缓存（推荐）` 流程：安全缓存项会先被极速移出当前目录，再由后台继续释放空间，不再复用普通回收站删除链路。
- 结果视图已改为“扫描完成后再看”的轻量目录下钻页，不再做实时 treemap。
- 结果视图已进一步调整为“上方摘要 + 下方填充型结果区”，避免列表只占一小块高度、下方大片留白。
- Overview 与 Settings 的滚动页已补底部安全区，并移除 Settings 内多余的二次宽度裁切，修复末尾卡片看起来被截断的问题。
- 卡片与提示条已统一改为“放宽 `clip rect` 后再绘制”，修复 `egui` 紧凑子布局把右边框和下边框切掉的问题。
- Overview 已继续重做为专用 dashboard 首页：
  - 更窄的独立首页宽度约束（继续从 `1240` 收到 `1160`）
  - 对称 gutter
  - 页面宽度改为真正的居中内容列，不再依赖左右 spacer 拼接
  - KPI 卡片改为强制吃满各自分配列宽，避免四张卡片缩在左侧破坏整体对称
  - 顶部四张卡改为唯一指标：`磁盘已用 / 磁盘可用 / 已扫描体积 / 错误`
  - 卷空间摘要已并回首页指标与扫描卡，不再重复做一张独立大卡
  - Hero 结论区、KPI 指标条、全宽扫描卡、双列证据区
  - 扫描卡已进一步改成“盘符优先、手动目录次要”的 Windows 风格入口，不再把大号路径输入框放在卡片顶部
  - 保持 19 语言选择下的可读性，其中 `中文 / English / Français / Español` 为完整界面文案
- `清理建议详情` 窗口现已改为居中受控对话框，补上顶部关闭入口，并为右侧大小列保留固定宽度，避免长路径把尺寸信息挤出窗口。
- `清理建议详情` 顶部操作条现已直接提供删除入口：缓存项可快速清理，其他选中项可直接移到回收站或永久删除。
- 红色高风险项的文案已改为“手动处理 / Manual Review”，并引导用户先打开所在位置，再自行确认后续操作。
- 停止扫描已改为真正可退出的流程：后台 walker 会定期轮询取消标记，工具栏按钮会进入 `正在停止 / Stopping` 状态，取消后的部分扫描结果不再误记为完成态历史。
- Windows 永久删除文件已接入更低层的 fast path；缓存一键清理则改为 `staging -> 后台 purge` 两阶段流程，优先保证“点完立刻感觉成功”。
- 扫描完成前的最终整理已改为后台阶段：目录遍历结束后，只保留轻量结果整理和可执行清理建议生成，不再默认把快照落库、历史写入和错误 CSV 导出压在完成态。
- `清理建议详情` 现已补充快捷操作：`全选安全项 / 清空所选 / 打开所选位置 / 快速清理选中缓存`；Inspector 选中缓存项时也会显示 `快速清理缓存`。
- `一键清理确认` 现已改为可滚动完整列表，会按完整路径列出本次要处理的全部项目，避免只看到前几项或被截断的路径。
- Inspector 的 `最近执行` 在存在失败项时，现已改为可点击的失败详情入口；完整路径、失败原因和处理建议统一收口到详情卡片中，外层不再展示截断的失败原因。
- 失败详情卡片现已改为顶部关闭、受控宽度和整行卡片布局；失败主因默认展示为本地化文案，原始技术细节退到次级说明，避免多语言界面被英文错误串主导。
- 失败详情相关的新文案现已补齐到全部已支持语言；`view_models` 新增的快速清理、选择安全项、打开所选等操作文案也已纳入统一翻译覆盖与测试校验。
- 删除执行链路现已逐项回传后台进度，Inspector 与顶部横幅会持续显示已处理/成功/失败统计和当前处理项，避免“一键提速执行中像卡死”的观感。
- 删除完成后的结果同步现已独立迁到后台阶段；`NodeStore` 重建、清理建议重算和结果摘要同步不再压在 UI 主线程里，顶部横幅和 Inspector 会显示 `结果同步中 / Sync Results`，避免窗口在收尾阶段进入 `Not Responding`。
- `结果视图` 自身的恢复也已改成后台载入；删除/结果同步期间点击 `结果视图` 不会再同步读取持久化数据库或解压旧缓存，而是按需从当前会话的临时快照恢复结果树。
- 默认主导航已收口为 `Overview / Live Scan / Result View / Settings`；高级工具仅保留 `Errors / Diagnostics` 二级入口。
- `诊断` 页现在只展示当前会话的结构化诊断信息，并保留内存优化与恢复类维护动作。
- 首页主路径继续服务“扫描磁盘并释放空间”；右侧 `Quick Actions` 现已提供独立 `一键释放系统内存 / Release System Memory`，避免把扫描和内存释放混成同一入口。
- `优化 DirOtter 内存占用` 与“清理异常中断的临时删除区”仍保留在 `高级工具 -> Diagnostics`，作为应用级维护与恢复工具。
- 清理建议计算已从“全树全量候选”收口为“规则命中 + 每类 Top-N + 全局上限”的可执行候选生成，优先保证完成态切换和清理操作速度。
- 已移除 SQLite 作为启动前置依赖；设置改为轻量文件存储，结果恢复改为当前会话临时快照。
- Settings 已进一步收口为窄内容列的分组设置页，优先展示高频设置，再展示视觉与说明信息。
- 设置页语言选择现已扩展到 19 种语言，并从平铺按钮收口为下拉菜单；启动时会优先按 `zh / en / ar / nl / fr / de / he / hi / id / it / ja / ko / pl / ru / es / th / tr / uk / vi` 系统语言环境自动选择。
- 法语与西班牙语现已补齐为完整 UI 版本，不再依赖英文说明文案回退；`dirotter-ui` 测试会自动扫描当前 `self.t(...)` 英文键并校验词典覆盖。
- `dirotter-testkit` 中的大树 snapshot 基线已移除 25ms 超短事件等待，性能阈值仍保留，但门禁不再因为调度抖动而偶发失败。
- 仓库现已补齐正式 GitHub Actions：
  - `.github/workflows/ci.yml` 默认执行 `template validation + fmt + check + clippy + test + release build`
  - `.github/workflows/release-windows.yml` 在 `v*` tag 或手动触发时生成 Windows 发布包
- Windows 发布链路现已提供：
  - `DirOtter-windows-x64-<version>-portable.zip`
  - `.sha256.txt` 校验文件
  - 可选 Authenticode 签名入口（配置 secrets 后自动启用）
  - 便携安装/卸载脚本
- Workspace `cargo check --workspace` 与 `cargo test --workspace` 均通过。
- 当前主要短板已从“主链路能否工作”转向“正式栅格系统、视觉回归保护和删除过程细粒度反馈”。

详细评估见：

- `docs/dirotter-comprehensive-assessment.md`

## 主要能力（已落地）

- 扫描引擎：多线程目录扫描、进度/批次/快照/完成事件、取消扫描。
- 扫描体验：面向用户的三档扫描模式，自动隐藏 batch / snapshot 等技术细节。
- 提速入口：磁盘侧仍由 Overview 给出单一推荐动作；系统内存释放则固定放在右侧 `Quick Actions`，不再混入扫描入口。
- 清理建议：基于规则的分类、风险分级、清理评分和分类汇总，优先把“可直接释放空间”的路径提到 Overview。
- 清理执行：支持安全缓存一键清理、分类详情勾选清理；缓存清理走极速 staging 链路，普通删除仍保留回收站/永久删除分流。
- 结果视图：基于扫描完成后的缓存结果，只展示当前目录的直接子项，并支持逐层下钻。
- 结果布局：结果页底部主列表会吃满剩余高度，并使用内部滚动承载长列表。
- 核心模型：`NodeStore` + `rollup()` + Top-N 文件/目录查询。
- Rust 内存优化：常驻 `NodeStore` 已改为“节点只保存 intern 后的字符串 ID，完整路径/名称留在字符串池和临时快照里”，避免每个节点重复持有两份 `String`。
- 扫描消息链路优化：`walker -> aggregator -> publisher` 内部热路径已改用共享路径字符串，减少批量事件中的重复 owned `String` 复制。
- 事件边界优化：扫描进度路径、实时 Top-N 和完成态 Top-N 也已改为共享路径，只有 UI 接手展示时才再物化字符串。
- UI 状态优化：实时/完成态排行与当前扫描路径在 UI 内部也已默认共享持有，进一步减少扫描进行中的重复字符串分配。
- Snapshot 节点优化：实时 snapshot 中的节点视图也已改为共享字符串字段，避免为每个节点重复构造完整 `name/path` 文本。
- Payload 收口：实时 snapshot 默认不再携带变更节点列表，完成态事件也不再重复发送可从 `store` 重建的 Top-N 排行。
- 类型收口：`SnapshotView` 已显式拆成 `Live` 与 `Full` 两类视图，轻量实时路径不再默认暴露节点列表。
- UI 选择收口：当前结果树相关交互已开始优先走 `NodeId`，路径字符串更多退回到 fallback 与外部路径场景。
- UI helper 收口：摘要卡片、扫描健康和排行/上下文榜单物化已下沉到独立 `view_models` 模块。
- UI 字符串热点收口：实时/完成态排行、上下文榜单、`live_files` 和 `TreemapEntry` 已改为共享路径优先。
- UI 路径状态收口：cleanup 勾选集合和 treemap 当前焦点也已改为共享路径持有，进一步减少 UI 内部高频 `String` 状态。
- Inspector / Confirm 收口：Inspector 摘要、后台删除任务摘要和两个确认窗的展示整形也已改由 `view_models` 统一生成。
- Inspector 动作态收口：按钮可用性、提示文案和反馈 banner/最近执行摘要也已改由 `view_models` 统一计算。
- Inspector 内存状态收口：右下区域现已改为由 `view_models` 统一生成的系统内存状态卡，集中展示系统可用内存、内存负载、DirOtter 占用与最近一次释放结果。
- Inspector 失败反馈收口：失败详情弹窗和 cleanup 确认中的完整删除列表也已改由 `view_models` 统一提供展示数据。
- Cleanup Details 收口：cleanup 详情窗的 tabs、统计区、按钮态和 item 行展示也已开始改由 `view_models` 统一整形。
- Cleanup Details 动作收口：cleanup 详情窗控制流已改为动作枚举分发，不再依赖多组布尔旗标串联。
- Confirm Dialog 动作收口：永久删除确认窗和 cleanup 确认窗也已改为动作分发模式。
- FastPurge 回退：快清 staging 会先试卷根 `.dirotter-staging`，权限受限时自动回退到源路径父目录，并在必要时做立即删除兜底。
- 性能基线：除扫描总耗时外，仓库现在还对 snapshot payload 大小和 snapshot 组装耗时做阈值回归。
- 运行时观测：diagnostics 现在会显示 live/final snapshot 的 changed nodes、materialized nodes、ranked items 和 text-bytes 估算，用于识别 payload 回退。
- 去重能力：按大小与哈希进行候选分组。
- 操作链路：Inspector 内真实删除、永久删除确认、后台删除任务提示、Windows 回收站二次校验、风险分层与删除后局部刷新。
- 报告能力：文本报告、摘要 JSON 与重复项/错误 CSV 仍可由独立报告模块生成，但不再属于默认 UI 主路径。
- 轻量存储能力：设置使用 `settings.json` 持久化；结果恢复使用当前会话的临时 `zstd+bincode` 快照文件，不再维护扫描历史数据库。
- Explorer 打开语义已收口：选中目录时 `Open File Location` 会直接打开该目录；选中文件时会在父目录中定位该文件，避免目录目标误落到默认位置。
- Inspector 中的 `快速清理缓存 / Fast Cleanup` 已改为上下文动作，只在当前选中项属于低风险缓存时显示，不再长期以灰按钮占位。
- 系统内存释放：右侧 `Quick Actions` 提供 `一键释放系统内存`；Inspector 下半区现已改为可滚动的内存状态卡，不再显示冗长说明，而是直接展示系统可用内存、内存负载、DirOtter 占用和最近一次释放结果。
- 维护能力：高级工具中继续保留应用级 `优化 DirOtter 内存占用`、低内存压力下自动把结果树落盘后释放为轻量状态、按需从快照回载结果，以及清理异常中断后遗留的内部临时删除区。
- UI 能力：支持 19 种界面语言选择（其中中文/英文/法语/西班牙语为完整 UI 文案，其余新增语言当前回退英文文案）、多脚本系统字体回退、Stop Scan、盘符快捷扫描、人类可读格式、页面级滚动、对称内容留白、目录上下文文件榜单、轻量结果视图与删除后即时局部刷新。

## 生产级达成标准

建议以以下四项作为生产门槛：

1. **稳定性**：大规模目录扫描可预测，长时间运行无明显资源泄漏。
2. **执行安全**：真实删除链路具备预检查、审计、失败恢复与回滚策略。
3. **可观测性**：关键性能/错误/动作指标可追踪、可导出、可定位。
4. **平台一致性**：Windows/macOS/Linux 行为差异被显式建模并可验证。

## 当前产品取舍建议

下一阶段建议继续把 DirOtter 明确收口为“释放空间工具”，而不是重新回到“扫描历史 / 诊断归档”这类对普通用户价值较低的工作台产品。

- 默认保留：扫描、清理建议、缓存快清、最大文件/目录证据、真实删除执行。
- 默认降级：跨会话结果持久化、扫描历史、诊断导出。
- 导航已收口为：`Overview / Live Scan / Result View / Settings` 为主路径，`Errors / Diagnostics` 通过 `高级工具 / Advanced Tools` 开关进入。
- 内存类功能已拆成两层：右侧 `一键释放系统内存` 负责系统级 working-set / file-cache 裁剪，`高级工具 -> Diagnostics -> 优化 DirOtter 内存占用` 只负责应用自身的结果树和运行时缓存。Rust 侧继续通过更紧凑的 `NodeStore` 和共享路径存储，保证工具本身不会反向放大系统压力。

## 工作区结构

```text
crates/
  dirotter-app        # 原生应用入口
  dirotter-ui         # UI 与交互状态管理
  dirotter-core       # 核心域模型与聚合查询
  dirotter-scan       # 目录扫描与事件流
  dirotter-dup        # 重复文件候选分析
  dirotter-cache      # 轻量设置存储 + 会话临时快照
  dirotter-platform   # 平台能力封装（打开路径/回收站/卷信息等）
  dirotter-actions    # 清理动作计划与执行（含模拟执行）
  dirotter-report     # 报告与导出
  dirotter-telemetry  # 观测初始化、运行时指标与 diagnostics 遥测快照
  dirotter-testkit    # 测试夹具、基线与阈值测试
```

## 快速开始

### 环境要求

- Rust stable
- 桌面环境（运行 `eframe` 原生窗口）

### 最终用户安装

从 GitHub Releases 下载 `DirOtter-windows-x64-<version>-portable.zip`，解压后可直接运行 `DirOtter.exe`，或执行 `scripts/install-windows-portable.ps1` 安装到当前用户目录。

### 构建与运行

```bash
cargo run -p dirotter-app
```

### 质量检查

```bash
cargo check --workspace
cargo test --workspace
```

### CI 与发布

- 持续集成：`.github/workflows/ci.yml`
- Windows 发布：`.github/workflows/release-windows.yml`
- 打包脚本：`scripts/package-windows.ps1`
- 签名脚本：`scripts/sign-windows.ps1`
- 便携安装脚本：`scripts/install-windows-portable.ps1`

## 文档导航

- 综合评估：`docs/dirotter-comprehensive-assessment.md`
- 系统设计：`docs/dirotter-sdd.md`
- UI 规格：`docs/dirotter-ui-component-spec.md`
- 安装与使用：`docs/dirotter-install-usage.md`
- 快速上手：`docs/quickstart.md`
- 当前任务队列：`.github/task-queue.md`
- 工程进展记录：`.github/memory/progress.md`
- 质量门禁说明：`.github/instructions/quality-gate.md`
- 模板校验脚本：`scripts/validate-template.ps1`
