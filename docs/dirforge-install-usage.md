# DirForge 安装与使用指南（2026-03）

## 0. 阶段说明

- 当前：Production Readiness（生产就绪冲刺）
- 目标：Production（生产级）


## 1. 安装

```bash
git clone <your-repo-url> DirForge
cd DirForge
```

## 2. 构建与测试

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

## 3. 运行

```bash
cargo run -p dirforge-app
```

## 4. 推荐使用流程

1. 在 Dashboard 选择扫描根目录。
2. 调整扫描参数（profile、batch、snapshot interval）。
3. 在 Current Scan 观察实时进度与统计。
4. 扫描完成后查看：
   - Top 文件/目录
   - 重复文件候选
   - 错误分类（User/Transient/System）
5. 在 Operations 中生成删除计划并执行模拟动作。
6. 导出报告和诊断文件。

## 5. 性能阈值测试

项目包含阈值测试：

```bash
cargo test -p dirforge-testkit --test benchmark_thresholds
```

当前默认阈值（小规模基准）：

- 扫描阈值：4000ms
- 去重阈值：1200ms

## 6. 产物说明

- `dirforge.db`：SQLite 缓存（快照/设置/历史）
- `dirforge_report.txt`：文本报告
- `dirforge_summary.json`：摘要 JSON
- `dirforge_duplicates.csv`：重复候选导出
- `dirforge_errors.csv`：错误导出
- `dirforge_diagnostics.json`：诊断导出

## 7. 注意事项

- 删除相关功能当前以“模拟执行”优先，属于安全策略的一部分。
- 大体量目录扫描时，首次运行可能出现较高 CPU/内存占用。
