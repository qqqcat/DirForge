# DirForge

DirForge 是一个基于 Rust 的本地磁盘分析器原型，当前聚焦于：

- 目录扫描与进度/快照事件流
- 目录树聚合与 Top-N 查询
- 重复文件候选识别
- 删除计划与模拟执行（回收站/永久删除模拟）
- 报告导出与 SQLite 快照缓存

> 项目阶段：**Pre-Alpha（功能链路可运行，尚未达到生产级）**。

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

- 综合评估：`docs/dirforge-comprehensive-assessment.md`
- 系统设计：`docs/dirforge-sdd.md`
- 安装与使用：`docs/dirforge-install-usage.md`
- 快速上手：`docs/quickstart.md`

---

建议阅读顺序：`dirforge-scan` → `dirforge-core` → `dirforge-ui`。
