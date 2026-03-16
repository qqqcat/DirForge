# DirForge

DirForge 是一个用 Rust 构建的本地磁盘分析器原型，目标是提供 **目录扫描、空间汇总、重复文件候选识别** 和后续清理动作的工程化基础。

> 当前阶段：**pre-alpha（可运行原型）**。

## 当前能力（已在代码中落地）

- 基于 `walkdir` 的目录扫描主链路（支持进度事件、批量事件、周期性快照、取消）。
- 最小可用的数据模型：`NodeStore`、`rollup()`、Top-N 文件/目录查询。
- 原生 GUI 壳（`eframe` + `egui`）可启动并消费扫描状态。
- SQLite 缓存骨架（`rusqlite`）与去重/操作/报告等子 crate 的基础结构。

## 当前限制（请在评估时一并考虑）

- 扫描器仍是单线程遍历 + 事件推送模型，尚未实现更深度的 IO/CPU 分层调度。
- 快照阶段仍依赖 `NodeStore` 克隆，数据量大时会影响吞吐与内存表现。
- 平台能力仍偏薄：`dirforge-platform` 目前以 Explorer 打开封装为主。
- `dirforge-telemetry` 仍是最小占位实现，尚未建立完整可观测链路。
- 删除与报告链路已具备结构雏形，但执行层和审计能力仍在建设中。

## 技术栈（当前 workspace 真实依赖）

- GUI：`eframe` / `egui`
- 数据与序列化：`serde` / `serde_json`
- 扫描：`walkdir`
- 去重哈希：`blake3`
- 缓存：`rusqlite`（bundled SQLite）

## 仓库结构

```text
crates/
  dirforge-app        # 原生应用入口
  dirforge-ui         # UI 与交互状态管理
  dirforge-core       # 核心域模型与聚合查询
  dirforge-scan       # 目录扫描与事件流
  dirforge-dup        # 重复文件候选分析
  dirforge-cache      # SQLite 缓存层
  dirforge-platform   # 平台相关能力封装
  dirforge-actions    # 清理动作计划与执行框架（早期）
  dirforge-report     # 报告与导出（早期）
  dirforge-telemetry  # 观测初始化（占位）
  dirforge-testkit    # 测试夹具与样例数据
```

## 快速开始

### 1) 环境要求

- Rust stable（建议通过 `rustup` 安装）
- 桌面环境（运行 `eframe` 原生窗口）

### 2) 构建与运行

```bash
cargo run -p dirforge-app
```

### 3) 测试

```bash
cargo test
```

## 近期优先级（Roadmap）

1. 扫描数据流优化：从全量快照向 delta/局部快照演进。
2. 平台能力补全：回收站、reparse point、统一错误模型。
3. 可观测性落地：结构化日志、吞吐/错误指标、诊断导出标准化。
4. 动作执行链路深化：预校验、批执行、部分失败处理与审计。
5. 测试与基准强化：边界 fixture、取消/错误场景、性能基线。

## 相关文档

- `docs/dirforge-comprehensive-assessment.md`
- `docs/dirforge-sdd.md`
- `docs/dirforge-install-usage.md`
- `docs/quickstart.md`

---

如果你是第一次查看仓库，建议先从 `crates/dirforge-scan` 与 `crates/dirforge-core` 阅读主链路，再看 `dirforge-ui` 的状态消费方式。
