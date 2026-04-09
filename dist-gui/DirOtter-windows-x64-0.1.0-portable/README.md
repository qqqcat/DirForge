# DirOtter

DirOtter 是一个基于 Rust 的本地磁盘分析器，当前产品定位已经明确收口为：

- 先快速扫描磁盘
- 再给出可执行的释放空间建议
- 最后直接在应用内完成删除或缓存快清

## 当前状态（2026-04-09）

当前项目处于 **Production Readiness**，已经具备稳定的端到端主链路，但仍未达到完全意义上的生产级发布标准。

本次仓库复验结果：

- `cargo fmt --all -- --check`：通过
- `cargo check --workspace`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo build --workspace`：通过

本地发布链路现状：

- 已存在 Windows 便携包：`dist/DirOtter-windows-x64-0.1.0-portable.zip`
- 已生成校验文件：`dist/DirOtter-windows-x64-0.1.0-portable.zip.sha256.txt`
- `dist/DirOtter-windows-x64-0.1.0-portable/BUILD-INFO.json` 显示本地发布包来自 `97ecd0d532909dc643db7bde324c898fa1d0111d`
- 当前本地产物状态：`NotSigned`

## 当前已落地能力

- 扫描链路：`worker -> aggregator -> publisher` 并发扫描、批次发布、节流、取消与完成态收尾。
- 用户扫描体验：三档扫描模式 `快速扫描（推荐）/ 深度扫描 / 超大硬盘模式`，并提供盘符快捷扫描。
- 首页主路径：Overview 会优先展示 `一键提速（推荐）`、清理建议摘要和关键证据区，而不是暴露底层参数。
- 清理建议：规则驱动分类、风险分级、评分和每类 Top-N 收口，优先把缓存、下载、安装包等可执行候选提到前面。
- 清理执行：支持 `Move to Recycle Bin`、`Delete Permanently` 和低风险缓存项的 `Fast Cleanup`。
- 删除反馈：后台删除线程会逐项回传已处理/成功/失败统计和当前处理项；删除完成后的结果同步也已迁出 UI 主线程。
- 结果视图：只在扫描完成后展示当前目录的直接子项，支持逐层下钻，不再走实时重布局路径。
- 轻量存储：设置使用 `settings.json` 持久化；结果恢复只使用当前会话临时 `zstd+bincode` 快照，不再依赖历史数据库。
- 设置容错：如果持久设置目录不可写，应用会自动回退到临时会话存储，并在设置页明确提示。
- 多语言：支持 19 种语言选择，其中 `中文 / English / Français / Español` 为完整 UI 文案，其余语言当前以英文回退。
- 发布准备：仓库已包含正式 CI、Windows 发布 workflow、打包脚本、安装脚本和可选代码签名入口。

## 当前工程判断

当前仓库已经不是“功能能跑但质量不可控”的状态，主要工程面判断如下：

- `dirotter-ui` 的首轮拆分已经完成，页面层、controller、cleanup 分析和 view-model 已不再全部堆在单文件里。
- 扫描快照链路已经完成一轮真正的增量化和少拷贝收口：
  - dirty 祖先传播
  - entry-time 聚合维护
  - 共享 `Arc<str>` 路径
  - live/full snapshot 类型拆分
  - payload 阈值测试和运行时 telemetry
- 轻量存储模型已经稳定：默认无数据库、事务式写入、会话快照、退出清理与陈旧临时目录回收都已落地。
- 工程门槛已经收口到 `fmt + check + test + clippy -D warnings`，并进入 CI。

## 仍然存在的主要短板

- 缺少自动化视觉回归，页面留白、栅格和多语言布局仍主要依赖人工检查。
- 真实删除链路的跨平台边界覆盖仍不够深，当前最成熟的仍是 Windows 路径。
- 结果默认只保留在当前会话内，这符合当前产品定位，但不适合“跨会话历史分析”场景。
- Windows 代码签名链路虽然已接好，但若未配置 secrets，最终产物仍会保持未签名。

## 工作区结构

```text
crates/
  dirotter-app        # 原生应用入口
  dirotter-ui         # UI、页面、view-model、交互状态
  dirotter-core       # NodeStore、聚合与查询
  dirotter-scan       # 扫描事件流与聚合发布
  dirotter-dup        # 重复文件候选检测
  dirotter-cache      # settings.json + 会话快照
  dirotter-platform   # Explorer、回收站、卷信息、快速清理 staging
  dirotter-actions    # 删除计划与执行
  dirotter-report     # 文本/JSON/CSV 报告导出
  dirotter-telemetry  # diagnostics 与运行时指标
  dirotter-testkit    # 性能阈值与回归基线
```

## 构建与运行

```bash
cargo run -p dirotter-app
```

质量检查：

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 发布与安装

- CI：`.github/workflows/ci.yml`
- Windows 发布：`.github/workflows/release-windows.yml`
- 便携打包：`scripts/package-windows.ps1`
- 可选签名：`scripts/sign-windows.ps1`
- 便携安装：`scripts/install-windows-portable.ps1`
- 便携卸载：`scripts/uninstall-windows-portable.ps1`

最终用户可直接下载 `DirOtter-windows-x64-<version>-portable.zip`，解压后运行 `DirOtter.exe`，或执行安装脚本安装到当前用户目录。

## 文档导航

- 综合评估：`docs/dirotter-comprehensive-assessment.md`
- 系统设计：`docs/dirotter-sdd.md`
- UI 规格：`docs/dirotter-ui-component-spec.md`
- 安装与使用：`docs/dirotter-install-usage.md`
- 快速上手：`docs/quickstart.md`
- 工程改进计划：`docs/engineering-improvement-plan-2026-04.md`
- 发现记录：`findings.md`
- 进展记录：`progress.md`
- 当前任务计划：`task_plan.md`
