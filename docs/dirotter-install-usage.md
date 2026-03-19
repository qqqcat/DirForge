# DirOtter 安装与使用指南（2026-03-19）

## 0. 阶段说明

- 当前：Production Readiness
- 目标：Production

## 1. 安装

```bash
git clone <your-repo-url> DirOtter
cd DirOtter
```

## 2. 构建与测试

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

## 3. 运行

```bash
cargo run -p dirotter-app
```

## 4. 推荐使用流程

1. 启动后优先使用 Overview 里的盘符快捷按钮，点击即可直接扫描对应卷。
2. 如果要扫描任意子目录，再手动修改根目录输入框。
3. 选择扫描模式，而不是手动调技术参数：
   - `快速扫描（推荐）`：适合日常整理和大多数本地磁盘
   - `深度扫描`：适合首次全面排查或目录层级复杂的场景
   - `超大硬盘模式`：适合超大容量磁盘、外置盘和文件数极多的目录
4. 扫描进行中，在工具栏使用 `Stop Scan` 停止扫描，而不是重复点击 `Start Scan`。
5. 如果某个页面内容超过当前窗口高度，请直接在主内容区内向下滚动；主要页面现在由页面整体滚动承载。
6. 在 Live Scan 页面观察实时进度、热点文件/目录与最近扫描列表。
7. 扫描完成后查看：
   - Overview 顶部的 `清理建议`
   - Overview（卷空间摘要、最大文件夹、最大文件）
   - Result View（只看当前目录直接子项的轻量结果视图）
   - 历史快照
   - 错误分类
8. 在 `清理建议` 卡中优先看“你可以释放多少空间”，再决定是否进入详情。
9. `一键清理缓存（推荐）` 只会处理安全缓存项，并默认使用回收站。
10. 点击 `查看详情` 后，可在分类详情窗中：
   - 查看风险标签
   - 勾选绿色项
   - 复核黄色项
   - 让红色项保持锁定
11. 在右侧 Inspector 查看当前选中文件/目录的上下文信息，并直接执行：
   - `Open File Location`
   - `Move to Recycle Bin`
   - `Delete Permanently`
12. 永久删除会先弹出确认窗口；点击确认后窗口会立即关闭，并转为顶部横幅、状态栏和 Inspector 中的后台任务提示。
13. 回收站删除成功后会提示可从系统回收站恢复；Windows 下还会做系统回收站二次校验。
14. 删除成功后，排行榜、概览统计、清理建议和结果视图会立即局部刷新。
15. 选中文件夹后，“最大文件”榜单会切换为该目录内部的大文件。
16. Result View 只在扫描完成后可用；它不会参与实时扫描刷新。
17. Result View 的目录结果区会吃满页面剩余高度；如果条目很多，请直接在该区域内部滚动。
18. 在 Settings 中切换 `中文 / English / Français / Español` 或深浅主题；标题旁状态胶囊也会跟随语言切换。
19. 导出报告和诊断文件。

说明：

- `Scanned Size` 表示本次扫描实际遍历到的文件总大小。
- `Volume Used` / `Total` 表示卷级别空间信息，不等同于扫描结果。
- 三种扫描模式都会完整扫描当前范围，差异只在扫描节奏和界面刷新方式。
- 首次启动会优先根据系统语言环境在 `zh / fr / es / en` 之间自动选择；Settings 中的手动选择优先级更高。
- `清理建议` 是规则驱动的分析层，不等于“自动删除”；真正执行前仍会经过确认。
- 当前后台删除任务表达的是阶段性状态，不是字节级进度条。

## 5. 性能阈值测试

```bash
cargo test -p dirotter-testkit --test benchmark_thresholds
```

## 6. 产物说明

- `dirotter.db`
- `dirotter_report.txt`
- `dirotter_summary.json`
- `dirotter_duplicates.csv`
- `dirotter_errors.csv`
- `dirotter_diagnostics.json`

## 7. 注意事项

- 永久删除是高敏感动作，当前建议优先使用“移到回收站”。
- 大体量目录扫描时，首次运行可能出现较高 CPU/内存占用。
- 若 GUI 无法启动，请确认运行环境具备桌面窗口支持。
- 若中文显示为方框或乱码，请确认系统存在可用 CJK 字体。
