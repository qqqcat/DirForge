# DirForge

DirForge 是一个基于 Rust 的本地磁盘分析器原型，当前聚焦于：

- 目录扫描与进度/快照事件流
- 目录树聚合与 Top-N 查询
- 重复文件候选识别
- 删除计划与模拟执行（回收站/永久删除模拟）
- 报告导出与 SQLite 快照缓存

> 当前状态：**工程化验证阶段（Production Readiness）**。
> 目标状态：**生产级（Production）**，并以稳定性、可观测性、执行安全与跨平台一致性达标作为发布门槛。

## 项目现状（2026-03）

基于当前代码与测试结果，本项目已具备从“扫描 -> 分析 -> 建议 -> 执行模拟 -> 导出”的端到端主链路。核心能力集中在 `dirforge-scan`、`dirforge-core` 与 `dirforge-ui`。

详细评估见：

- `docs/dirforge-comprehensive-assessment.md`

## 主要能力（已落地）

- 扫描引擎：支持进度事件、批事件、快照事件、完成事件、取消扫描。
- 核心模型：`NodeStore` + `rollup()` + Top-N 文件/目录查询。
- 去重能力：按大小与哈希进行候选分组。
- 操作链路：删除计划生成、风险分层、模拟执行与审计输出。
- 报告能力：文本报告、摘要 JSON、重复项/错误 CSV 导出。
- 缓存能力：SQLite 持久化快照、历史记录、设置读写。


## 生产级达成标准（Definition of Production）

为避免“目标是生产级”与“现状描述”混淆，项目采用双轨表述：

- **现状**：已具备端到端可运行能力，但仍在稳定性与平台深度补强阶段。
- **目标**：达到 Production 级发布标准。

建议以以下四项作为生产门槛：

1. **稳定性**：大规模目录扫描可预测，长时间运行无明显资源泄漏。
2. **执行安全**：真实删除链路具备预检查、审计、失败恢复与回滚策略。
3. **可观测性**：关键性能/错误/动作指标可追踪、可导出、可定位。
4. **平台一致性**：Windows/macOS/Linux 行为差异被显式建模并可验证。

## 当前限制

- 扫描模型仍以单线程主循环 + 事件推送为主，极大规模目录下还有优化空间。
- UI 与扫描流的快照合并策略可用但仍偏保守，需继续优化峰值内存和刷新抖动。
- 平台能力虽有封装，但跨平台行为一致性、系统权限边界处理仍需补强。
- 可观测性已起步，但系统级指标、结构化日志与诊断归档仍可深化。

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
