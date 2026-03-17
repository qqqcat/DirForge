# DirForge

DirForge 是一个基于 Rust 的本地磁盘分析器原型，当前聚焦于：

- 目录扫描与进度/快照事件流
- 目录树聚合与 Top-N 查询
- 重复文件候选识别
- Inspector 内删除执行（回收站 / 永久删除，含审计）
- 报告导出与 SQLite 快照缓存

> 当前状态：**工程化验证阶段（Production Readiness）**。
> 目标状态：**生产级（Production）**，并以稳定性、可观测性、执行安全与跨平台一致性达标作为发布门槛。

## 项目现状（2026-03-16）

基于当前代码与全量回归测试，项目已具备从“扫描 → 分析 → 直接处理 → 导出诊断”的端到端主链路。

本轮综合评估结论：

- 扫描链路已完成并发化（worker + 聚合线程 + 有界队列），可在高吞吐场景下维持更稳定资源曲线。
- 平台能力、审计与诊断链路已形成可落地工程骨架。
- 桌面 UI 已完成连续数轮可用性重构：新增更清晰的工具栏/导航/检查器布局、统一数据格式化、中文字体回退、更稳健的 treemap 标注规则，以及可滚动的排行榜。
- 删除动作已移入右侧 Inspector，支持回收站删除与永久删除确认；删除成功后可对列表、概览统计和 treemap 做局部刷新。
- 启动时会优先选择系统盘/首个卷作为默认根路径，并提供盘符快捷按钮，点击即可直接扫描对应卷。
- Workspace `cargo check/test` 均可通过，含扫描、平台、报告、阈值测试回归。
- 仍需继续补强真实删除确认流、跨平台边界与长跑稳定性验证。

详细评估见：

- `docs/dirforge-comprehensive-assessment.md`

## 主要能力（已落地）

- 扫描引擎：支持多线程目录扫描、进度/批次/快照/完成事件与取消扫描。
- 核心模型：`NodeStore` + `rollup()` + Top-N 文件/目录查询。
- 去重能力：按大小与哈希进行候选分组。
- 操作链路：Inspector 内真实删除、永久删除确认、风险分层、审计输出与删除后局部刷新。
- 报告能力：文本报告、摘要 JSON、重复项/错误 CSV、诊断包导出与归档。
- 缓存能力：SQLite 负责元数据/历史/设置/审计；快照 payload 使用 `zstd+bincode` 压缩 blob，并保留历史 JSON 兼容读取。
- UI 能力：支持中英文切换、系统中文字体回退、实时检查器、Stop Scan 动作、盘符快捷扫描、人类可读文件大小格式化、可滚动排行榜与删除后即时局部刷新。

## 生产级达成标准（Definition of Production）

项目采用双轨表述：

- **现状**：已具备端到端可运行能力，但仍在稳定性与平台深度补强阶段。
- **目标**：达到 Production 级发布标准。

建议以以下四项作为生产门槛：

1. **稳定性**：大规模目录扫描可预测，长时间运行无明显资源泄漏。
2. **执行安全**：真实删除链路具备预检查、审计、失败恢复与回滚策略。
3. **可观测性**：关键性能/错误/动作指标可追踪、可导出、可定位。
4. **平台一致性**：Windows/macOS/Linux 行为差异被显式建模并可验证。

## 工作区结构

```text
crates/
  dirforge-app        # 原生应用入口
  dirforge-ui         # UI 与交互状态管理
  dirforge-core       # 核心域模型与聚合查询
  dirforge-scan       # 目录扫描与事件流
  dirforge-dup        # 重复文件候选分析
  dirforge-cache      # SQLite 缓存层
  dirforge-platform   # 平台能力封装（打开路径/回收站/卷信息等）
  dirforge-actions    # 清理动作计划与执行（含模拟执行）
  dirforge-report     # 报告与导出
  dirforge-telemetry  # 观测初始化与指标骨架
  dirforge-testkit    # 测试夹具、基线与阈值测试
```

## 快速开始

### 环境要求

- Rust stable（建议通过 `rustup` 安装）
- 桌面环境（用于运行 `eframe` 原生窗口）

### 构建与运行

```bash
cargo run -p dirforge-app
```

### 质量检查

```bash
cargo check --workspace
cargo test --workspace
```

## 文档导航

- 生产升级计划与落地：`docs/production-upgrade-2026-03.md`
- 综合评估：`docs/dirforge-comprehensive-assessment.md`
- 系统设计：`docs/dirforge-sdd.md`
- 安装与使用：`docs/dirforge-install-usage.md`
- 快速上手：`docs/quickstart.md`

---

建议阅读顺序：`dirforge-scan` → `dirforge-core` → `dirforge-ui`。
