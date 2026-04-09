# DirOtter 安装与使用指南（2026-04-09）

## 1. 当前阶段

- 当前：`Production Readiness`
- 目标：`Production`

## 2. 最终用户安装

### 2.1 从发布包安装

1. 下载 `DirOtter-windows-x64-<version>-portable.zip`
2. 校验同目录的 `.sha256.txt`
3. 解压压缩包
4. 二选一：
   - 直接运行 `DirOtter.exe`
   - 执行 `scripts/install-windows-portable.ps1`

### 2.2 卸载

执行：

```powershell
scripts/uninstall-windows-portable.ps1
```

### 2.3 代码签名说明

- 发布 workflow 支持对 `DirOtter.exe` 做可选 Authenticode 签名
- 需要配置 secrets：
  - `WINDOWS_CODESIGN_CERT_BASE64`
  - `WINDOWS_CODESIGN_PASSWORD`
  - 可选 `WINDOWS_CODESIGN_TIMESTAMP_URL`
- 若未配置，发布流程仍可继续，但产物会保持未签名

## 3. 开发者环境

要求：

- Rust stable
- 可运行原生桌面窗口的环境

获取源码：

```bash
git clone <your-repo-url> DirOtter
cd DirOtter
```

## 4. 构建与验证

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

运行桌面应用：

```bash
cargo run -p dirotter-app
```

## 5. 推荐使用路径

1. 启动后优先在 Overview 点击盘符快捷按钮开始扫描。
2. 只有要扫描任意子目录时，再使用手动目录输入框。
3. 优先使用三档扫描模式，而不是纠结底层参数：
   - `快速扫描（推荐）`
   - `深度扫描`
   - `超大硬盘模式`
4. 扫描中需要终止时，使用 `Stop Scan`，按钮会短暂进入 `Stopping` 状态。
5. 在 `Live Scan` 观察实时进度、当前热点目录和最近扫描文件。
6. 扫描完成后，先看 `Overview` 顶部的主动作和清理建议摘要。
7. 再看首页 KPI、最大文件夹和最大文件证据区。
8. 如需目录级下钻，进入 `Result View`，它只展示当前目录的直接子项。
9. 如需清理重复文件，进入 `Duplicate Files`：
   - 页面会先按大小分组选出候选，再在后台补算 hash
   - 默认按组展示，并给出 `推荐保留`
   - `自动选择建议` 会为每组保留 1 个副本，其余标记为删除候选
10. 如需直接处理，使用右侧 Inspector：
   - `Open File Location`
   - `Move to Recycle Bin`
   - `Delete Permanently`
   - 当前选中项若命中低风险缓存规则，还会出现 `Fast Cleanup`
11. 永久删除会先弹确认层；确认后窗口立即关闭，执行转为后台任务。
12. 删除进行中，顶部横幅和 Inspector 会显示已处理/成功/失败统计和当前处理项。
13. 删除完成后，结果同步会在后台进行，UI 不应进入 `Not Responding`。
14. 如需系统级内存辅助动作，使用右侧 `Quick Actions` 的 `Release System Memory`。
15. 如需应用级维护动作，进入 `高级工具 -> Diagnostics`：
   - `优化 DirOtter 内存占用`
   - `清理异常中断的临时删除区`

## 6. 当前存储与恢复说明

- 设置保存在 `settings.json`
- 扫描结果默认只保留在当前会话内
- Result View 恢复使用当前会话临时 `zstd+bincode` 快照
- 应用启动不再依赖 SQLite
- 如果持久设置目录不可写，DirOtter 会自动回退到临时会话存储，并在设置页提示本次偏好不会跨重启保留

## 7. CI 与发布链路

- 持续集成：`.github/workflows/ci.yml`
  - `template validation`
  - `cargo fmt --all -- --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo build --release -p dirotter-app`
- Windows 发布：`.github/workflows/release-windows.yml`
  - `v*` tag 或 `workflow_dispatch`
  - 生成便携 zip
  - 生成 `.sha256.txt`
  - 可选代码签名

## 8. 手动打包

```powershell
scripts/package-windows.ps1 -Configuration release
```

输出目录默认是 `dist/`。

## 9. 关键产物

- `DirOtter-windows-x64-<version>-portable.zip`
- `DirOtter-windows-x64-<version>-portable.zip.sha256.txt`
- `BUILD-INFO.json`
- `settings.json`

以下仍属于可选导出，不是默认 UI 主路径：

- `dirotter_report.txt`
- `dirotter_summary.json`
- `dirotter_duplicates.csv`
- `dirotter_errors.csv`

## 10. 注意事项

- 当前最成熟平台是 Windows。
- 永久删除是高敏感动作，日常场景优先建议走回收站。
- 当前后台反馈是“逐项进度 + 阶段状态”，不是字节级进度条。
- 若要正式对外分发，请先完成 Windows 代码签名配置与验签。
