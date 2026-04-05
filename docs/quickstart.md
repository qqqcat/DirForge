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

如果在 macOS 上运行独立入口，也可以使用：

```bash
cargo run -p dirotter-macos
```

如果在 Linux 上运行独立入口，也可以使用：

```bash
cargo run -p dirotter-linux
```

建议按以下路径体验：

1. 更推荐直接点击盘符快捷按钮开始扫描对应卷。
2. 只有要改成任意子目录时，再使用盘符区后面的可选手动目录输入框。
3. 在扫描页先选择扫描模式：
   - `快速扫描（推荐）`
   - `深度扫描`
   - `超大硬盘模式`
4. 在扫描页观察进度、排行榜与最近发现列表；扫描中如需终止，请使用 `Stop Scan`。
5. 点击 `Stop Scan` 后应看到 `Stopping` 态，并在短时间内安全返回，不应出现界面卡死或崩溃。
6. 如果页面内容超出当前窗口高度，请直接在主内容区滚动。
7. 扫描结束后先看 Overview 顶部 Hero 区，先确认“现在最值得做什么”。
8. 再看 Overview 中部 KPI 指标条和全宽 `扫描设置` 卡，最后再下钻到最大文件夹和最大文件。
9. 如果界面语言切换为 `Français / Español`，首页结构和按钮顺序应保持一致，不应把卡片撑乱或挤重叠。
10. 直接尝试 `一键清理缓存（推荐）`，它只会处理安全缓存项，并应立即表现为“已移出、后台继续释放空间”。
11. 如需人工复核，打开 `查看详情`，确认绿色/黄色/红色三类标签。
12. 打开 Result View，只查看当前目录的直接子项，并按需逐层进入下一层。
13. Result View 底部的目录结果区会吃满剩余高度；条目较多时直接在该区域内部滚动。
13. 选中文件夹后，右侧“最大文件”榜单会切换为该目录内部的大文件。
14. 选中一个文件或文件夹，在右侧 Inspector 中直接执行“移到回收站”或“永久删除”。
15. 永久删除会先出现确认层；点击确认后窗口会立即关闭，并转为顶部横幅、状态栏和 Inspector 的后台任务提示。
16. 确认删除后榜单、概览统计、清理建议与结果视图已即时刷新。
17. 在 Settings 中切换 `中文 / English / Français / Español`，确认导航、状态胶囊和主要操作文本会即时更新。
18. 反复对同一路径扫描后，检查 `dirotter.db` / `dirotter.db-wal` 不应继续按扫描次数线性变大。
19. 若上次快速缓存清理在后台删除完成前异常退出，本次启动后应自动继续清理内部临时删除区遗留项。
20. 如果需要保留扫描资产，再进入 `高级工具 -> Diagnostics` 手动保存快照、摘要或错误 CSV。
21. 首页应优先出现单一推荐动作 `一键提速（推荐）`，并根据当前状态自动变成“开始提速扫描 / 一键提速（推荐） / 查看提速建议”。
22. 如需临时释放系统内存，请直接使用右侧 `Quick Actions` 的 `一键释放系统内存`；该动作会在后台执行，不应让界面锁死，释放结果会显示在右侧 Inspector 的可滚动内存状态卡中。
23. 如需减少应用自身占用或恢复异常中断的清理现场，请进入 `高级工具 -> Diagnostics` 使用 `优化 DirOtter 内存占用` 或 `清理异常中断的临时删除区`。

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

## 7. 打包

- Linux: `./scripts/package-linux.sh`
- macOS: `./scripts/package-macos.sh`
- 详细步骤见：`docs/release-packaging.md`
