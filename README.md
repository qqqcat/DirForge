# DirOtter

DirOtter 是一个基于 Rust 的本地磁盘分析器，当前聚焦于：

- 目录扫描与进度/快照事件流
- 目录树聚合与 Top-N 查询
- 规则驱动的清理建议与安全缓存清理
- 重复文件候选识别
- Inspector 内删除执行（回收站 / 永久删除，含审计）
- 报告导出与 SQLite 快照缓存

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

基于当前代码、实测问题修复和全量回归测试，DirOtter 已具备从“扫描 -> 分析 -> 展示 -> 直接处理 -> 导出诊断”的端到端主链路。

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
- 已新增 `一键清理缓存（推荐）` 流程：只对安全缓存项生效，默认走回收站，不混入永久删除。
- 结果视图已改为“扫描完成后再看”的轻量目录下钻页，不再做实时 treemap。
- 结果视图已进一步调整为“上方摘要 + 下方填充型结果区”，避免列表只占一小块高度、下方大片留白。
- Overview 与 Settings 的滚动页已补底部安全区，并移除 Settings 内多余的二次宽度裁切，修复末尾卡片看起来被截断的问题。
- 卡片容器已统一增加外边距，修复 `egui` 子布局裁剪矩形把右边框和下边框切掉的问题。
- 设置页已从中英文扩展到 `中文 / English / Français / Español` 四语言切换；启动时会优先按 `zh / fr / es / en` 系统语言环境自动选择。
- 法语与西班牙语现已补齐为完整 UI 版本，不再依赖英文说明文案回退；`dirotter-ui` 测试会自动扫描当前 `self.t(...)` 英文键并校验词典覆盖。
- Workspace `cargo check --workspace` 与 `cargo test --workspace` 均通过。
- 当前主要短板已从“主链路能否工作”转向“正式栅格系统、视觉回归保护和删除过程细粒度反馈”。

详细评估见：

- `docs/dirotter-comprehensive-assessment.md`

## 主要能力（已落地）

- 扫描引擎：多线程目录扫描、进度/批次/快照/完成事件、取消扫描。
- 扫描体验：面向用户的三档扫描模式，自动隐藏 batch / snapshot 等技术细节。
- 清理建议：基于规则的分类、风险分级、清理评分和分类汇总，优先把“可直接释放空间”的路径提到 Overview。
- 清理执行：支持安全缓存一键清理、分类详情勾选清理，并统一走回收站删除链路。
- 结果视图：基于扫描完成后的缓存结果，只展示当前目录的直接子项，并支持逐层下钻。
- 结果布局：结果页底部主列表会吃满剩余高度，并使用内部滚动承载长列表。
- 核心模型：`NodeStore` + `rollup()` + Top-N 文件/目录查询。
- 去重能力：按大小与哈希进行候选分组。
- 操作链路：Inspector 内真实删除、永久删除确认、后台删除任务提示、Windows 回收站二次校验、风险分层、审计输出与删除后局部刷新。
- 报告能力：文本报告、摘要 JSON、重复项/错误 CSV、诊断包导出与归档。
- 缓存能力：SQLite 负责元数据/历史/设置/审计；快照 payload 使用 `zstd+bincode` 压缩 blob，并保留历史 JSON 兼容读取。
- UI 能力：支持中文/英文/法语/西班牙语切换、系统中文字体回退、Stop Scan、盘符快捷扫描、人类可读格式、页面级滚动、对称内容留白、目录上下文文件榜单、轻量结果视图与删除后即时局部刷新。

## 生产级达成标准

建议以以下四项作为生产门槛：

1. **稳定性**：大规模目录扫描可预测，长时间运行无明显资源泄漏。
2. **执行安全**：真实删除链路具备预检查、审计、失败恢复与回滚策略。
3. **可观测性**：关键性能/错误/动作指标可追踪、可导出、可定位。
4. **平台一致性**：Windows/macOS/Linux 行为差异被显式建模并可验证。

## 工作区结构

```text
crates/
  dirotter-app        # 原生应用入口
  dirotter-ui         # UI 与交互状态管理
  dirotter-core       # 核心域模型与聚合查询
  dirotter-scan       # 目录扫描与事件流
  dirotter-dup        # 重复文件候选分析
  dirotter-cache      # SQLite 缓存层
  dirotter-platform   # 平台能力封装（打开路径/回收站/卷信息等）
  dirotter-actions    # 清理动作计划与执行（含模拟执行）
  dirotter-report     # 报告与导出
  dirotter-telemetry  # 观测初始化与指标骨架
  dirotter-testkit    # 测试夹具、基线与阈值测试
```

## 快速开始

### 环境要求

- Rust stable
- 桌面环境（运行 `eframe` 原生窗口）

### 构建与运行

```bash
cargo run -p dirotter-app
```

### 质量检查

```bash
cargo check --workspace
cargo test --workspace
```

## 文档导航

- 综合评估：`docs/dirotter-comprehensive-assessment.md`
- 系统设计：`docs/dirotter-sdd.md`
- UI 规格：`docs/dirotter-ui-component-spec.md`
- 安装与使用：`docs/dirotter-install-usage.md`
- 快速上手：`docs/quickstart.md`
