# DirOtter Linux 迁移说明（2026-04-05）

## 1. 新增 Linux 独立入口

- 新增目录：`platforms/dirotter-linux/`
- 新增可执行包：`dirotter-linux`

运行方式：

```bash
cargo run -p dirotter-linux
```

## 2. 复用策略

Linux 版本继续复用现有跨平台核心：

- `dirotter-core`
- `dirotter-scan`
- `dirotter-dup`
- `dirotter-cache`
- `dirotter-actions`
- `dirotter-report`
- `dirotter-ui`
- `dirotter-telemetry`

## 3. 平台层行为

Linux 平台能力沿用 `dirotter-platform` 的非 Windows / 非 macOS 分支：

- 文件管理器打开：`xdg-open`
- 选中定位：回退到打开父目录
- 回收站删除：通过 `trash` crate
- 稳定文件身份：走 Unix 元数据（`dev + inode`）

## 4. 编译验证

```bash
cargo check -p dirotter-linux
```
