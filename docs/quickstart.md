# DirForge Quick Start

本指南用于 5~10 分钟内完成本地运行、基础验证和结果产物检查。

## 1. 环境准备

- 安装 Rust stable（`rustup default stable`）
- 在 Linux/macOS/Windows 任一桌面环境下运行

## 2. 获取代码

```bash
git clone <your-repo-url> DirForge
cd DirForge
```

## 3. 一次性健康检查

```bash
cargo check --workspace
cargo test --workspace
```

如果以上命令均通过，说明当前机器可完整构建并通过测试集。

## 4. 启动桌面应用

```bash
cargo run -p dirforge-app
```

应用启动后建议按以下路径体验：

1. 输入一个本地目录并开始扫描。
2. 在扫描页观察进度、快照和统计。
3. 扫描结束后查看重复候选与删除计划。
4. 进入操作页执行模拟动作并查看结果。
5. 导出报告与诊断文件。

## 5. 关键输出文件

运行/导出后通常可见：

- `dirforge.db`
- `dirforge_report.txt`
- `dirforge_summary.json`
- `dirforge_duplicates.csv`
- `dirforge_errors.csv`
- `dirforge_diagnostics.json`

## 6. 常见排查

- 构建慢：首次依赖编译时间较长，属正常现象。
- 无法启动 GUI：确认当前环境支持桌面窗口。
- 扫描结果偏少：检查目录权限与符号链接策略。

## 7. 下一步阅读

- 架构与设计：`docs/dirforge-sdd.md`
- 综合评估：`docs/dirforge-comprehensive-assessment.md`
- 安装与使用细节：`docs/dirforge-install-usage.md`
