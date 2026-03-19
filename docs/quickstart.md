# DirOtter Quick Start（2026-03-19）

本指南用于 5~10 分钟内完成本地运行、基础验证和结果产物检查。

## 1. 环境准备

- 安装 Rust stable
- 在 Linux/macOS/Windows 任一桌面环境下运行

## 2. 获取代码

```bash
git clone <your-repo-url> DirOtter
cd DirOtter
```

## 3. 一次性健康检查

```bash
cargo check --workspace
cargo test --workspace
```

## 4. 启动桌面应用

```bash
cargo run -p dirotter-app
```

建议按以下路径体验：

1. 更推荐直接点击盘符快捷按钮开始扫描对应卷。
2. 在扫描页先选择扫描模式：
   - `快速扫描（推荐）`
   - `深度扫描`
   - `超大硬盘模式`
3. 在扫描页观察进度、快照、排行榜与最近发现列表；扫描中如需终止，请使用 `Stop Scan`。
4. 如果页面内容超出当前窗口高度，请直接在主内容区滚动。
5. 扫描结束后在 Overview 和 Live Scan 中查看最大文件夹与最大文件。
6. 优先看 Overview 顶部 `清理建议` 卡，先确认可释放空间总量和分类来源。
7. 直接尝试 `一键清理缓存（推荐）`，它只会处理安全缓存项并默认进入回收站。
8. 如需人工复核，打开 `查看详情`，确认绿色/黄色/红色三类标签。
9. 打开 Result View，只查看当前目录的直接子项，并按需逐层进入下一层。
10. Result View 底部的目录结果区会吃满剩余高度；条目较多时直接在该区域内部滚动。
11. 选中文件夹后，右侧“最大文件”榜单会切换为该目录内部的大文件。
12. 选中一个文件或文件夹，在右侧 Inspector 中直接执行“移到回收站”或“永久删除”。
13. 永久删除会先出现确认层；点击确认后窗口会立即关闭，并转为顶部横幅、状态栏和 Inspector 的后台任务提示。
14. 确认删除后榜单、概览统计、清理建议与结果视图已即时刷新。
15. 在 Settings 中切换 `中文 / English / Français / Español`，确认导航、状态胶囊和主要操作文本会即时更新。
16. 导出报告与诊断文件。

## 5. 关键输出文件

- `dirotter.db`
- `dirotter_report.txt`
- `dirotter_summary.json`
- `dirotter_duplicates.csv`
- `dirotter_errors.csv`
- `dirotter_diagnostics.json`

## 6. 常见排查

- 构建慢：首次依赖编译时间较长，属正常现象。
- 无法启动 GUI：确认当前环境支持桌面窗口。
- 扫描结果偏少：检查目录权限与符号链接策略。
- 模式选择困难：多数情况下直接用 `快速扫描（推荐）` 即可；其他两档主要用于复杂目录或超大容量磁盘。
