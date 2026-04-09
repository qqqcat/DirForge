# DirOtter Quick Start（2026-04-09）

本指南用于 5 到 10 分钟内确认：项目能构建、能启动、主路径能走通。

## 1. 获取与验证代码

```bash
git clone <your-repo-url> DirOtter
cd DirOtter
cargo check --workspace
cargo test --workspace
```

如需完整门禁，再执行：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

## 2. 启动应用

```bash
cargo run -p dirotter-app
```

## 3. 建议的 10 分钟体验路径

1. 在 Overview 直接点击盘符按钮开始扫描。
2. 保持默认 `快速扫描（推荐）`，除非你明确在测复杂目录或超大磁盘。
3. 切到 `Live Scan`，确认进度、排行榜和最近扫描文件都在更新。
4. 点击一次 `Stop Scan`，确认按钮进入 `Stopping` 并能安全退出。
5. 重新开始一次扫描并等待完成。
6. 回到 Overview，先确认首页主动作和清理建议摘要是否合理。
7. 打开 `查看详情`，检查绿色/黄色/红色风险分级。
8. 尝试 `一键提速（推荐）` 或 `Fast Cleanup`，确认反馈转为后台任务而不是卡住窗口。
9. 打开 `Result View`，确认它只展示当前目录的直接子项，并支持逐层下钻。
10. 在 Inspector 尝试 `Open File Location` 与 `Move to Recycle Bin`。
11. 在 Settings 切换语言或主题，确认界面即时生效。
12. 如需维护动作，再进入 `高级工具 -> Diagnostics`。

## 4. 预期结果

- 扫描、停止扫描和完成态切换不会卡死 UI。
- 删除完成后的结果同步会在后台进行。
- Result View 进入时不会在当前 UI 帧同步解压或重建结果树。
- 设置可持久化；若设置目录不可写，Settings 会明确显示临时会话存储提示。

## 5. 发布产物速查

默认发布产物位于 `dist/`：

- `DirOtter-windows-x64-<version>-portable.zip`
- `DirOtter-windows-x64-<version>-portable.zip.sha256.txt`
- `DirOtter-windows-x64-<version>-portable/BUILD-INFO.json`

## 6. 常见排查

- GUI 无法启动：确认当前环境支持桌面窗口。
- 中文或多语言出现方框：确认系统具备相应字体，DirOtter 会优先加载系统 fallback。
- 设置未保留：检查 Settings 页面是否提示当前正在使用临时会话存储。
- 发布包未签名：这是当前默认行为，需配置 signing secrets 后才会变为签名产物。
