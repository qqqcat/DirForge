# DirForge 安装与使用指南（2026-03-16）

## 0. 阶段说明

- 当前：Production Readiness（生产就绪冲刺后期）
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

1. 在 Overview 页面选择扫描根目录。
2. 调整扫描参数（profile、batch、snapshot interval）。
3. 在 Live Scan 页面观察实时进度、热点文件/目录与实时发现列表。
4. 扫描完成后查看：
   - Treemap（只对可读区域显示标签，悬浮看完整路径）
   - 历史快照
   - 错误分类（User/Transient/System）
   - 操作中心中的删除计划与模拟执行结果
5. 在右侧 Inspector 查看当前选中文件/目录的上下文信息。
6. 在 Settings 中切换中英文或深浅主题；Windows 默认会优先加载 Microsoft YaHei / DengXian 等中文字体回退。
7. 导出报告和诊断文件（含诊断归档目录）。

说明：

- `Scanned Size` 表示本次扫描实际遍历到的文件总大小。
- `Volume Used` / `Total` 表示卷级别空间信息，来源于系统磁盘信息，不等同于扫描结果。

## 5. 性能阈值测试

项目包含阈值测试：

```bash
cargo test -p dirforge-testkit --test benchmark_thresholds
```

当前默认阈值（`crates/dirforge-testkit/perf/baseline.json`）：

- 小规模扫描阈值：500ms
- 大规模扫描阈值：3500ms
- 小规模去重阈值：300ms

## 6. 产物说明

- `dirforge.db`：SQLite 缓存（快照/设置/历史/审计）
- `dirforge_report.txt`：文本报告
- `dirforge_summary.json`：摘要 JSON
- `dirforge_duplicates.csv`：重复候选导出
- `dirforge_errors.csv`：错误导出
- `dirforge_diagnostics.json`：诊断导出
- `diagnostics-<timestamp>/`：诊断归档目录（含 manifest）

## 7. 注意事项

- 删除相关功能仍建议以“模拟执行优先”作为默认安全策略。
- 大体量目录扫描时，首次运行可能出现较高 CPU/内存占用。
- 若 GUI 无法启动，请确认运行环境具备桌面窗口支持。
- 若中文显示为方框或乱码，请确认系统存在可用 CJK 字体；应用会优先加载系统字体回退。
