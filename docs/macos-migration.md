# DirOtter macOS 迁移说明（2026-04-05）

本文档说明本次将现有 Rust/Windows 项目迁移到 macOS 版本时的拆分策略、可复用模块与平台替换点。

## 1. 新增目录与构建入口

- 新增独立目录：`platforms/dirotter-macos/`
- 新增独立可执行入口：`dirotter-macos`
- 运行命令：

```bash
cargo run -p dirotter-macos
```

该入口复用现有 UI/扫描/核心模型能力，只替换平台层能力实现。

## 2. 可直接复用的模块

以下 crate 基本为跨平台 Rust 逻辑，可直接复用：

- `dirotter-core`：数据结构、聚合与查询
- `dirotter-scan`：扫描 worker/聚合/publisher
- `dirotter-dup`：重复文件识别
- `dirotter-cache`：SQLite 缓存
- `dirotter-actions`：删除计划与执行流程编排
- `dirotter-report`：报告输出
- `dirotter-ui`：Egui 界面与交互状态
- `dirotter-telemetry`：观测初始化

## 3. Windows 专有点与 macOS 对应替换

### 3.1 文件管理器集成（Explorer -> Finder）

- Windows: `explorer /select,...`
- macOS: `open -R <path>`（在 Finder 中定位文件）
- 目录打开能力继续使用 `open <path>`

### 3.2 内存能力

原先以下能力仅在 Windows 实现：

- 进程内存指标（working set/pagefile/private）
- 系统内存指标（内存负载、可用内存）
- 一键释放内存（工作集裁剪 + 系统文件缓存裁剪）

本次为 macOS 增加了可运行实现：

- `process_memory_stats()`：通过 `sysinfo` 获取当前进程 RSS/虚拟内存
- `system_memory_stats()`：通过 `sysinfo` 获取系统总内存与可用内存并计算负载
- `release_system_memory()`：保留统一接口，使用“释放前后采样”生成报告

> 说明：macOS 没有与 Windows `EmptyWorkingSet` 等价且稳定的免特权接口，因此 `trim_process_memory()` 在 macOS 采用 best-effort 成功返回，系统级缓存裁剪字段保持 `false`。

### 3.3 回收站/删除

- 删除主流程仍由 `dirotter-actions` 统一编排。
- 回收站能力继续复用 `trash` crate，在 macOS 上对接系统废纸篓。
- Windows 专有“回收站二次校验”逻辑仍仅在 Windows 下启用。

## 4. 目录与工作区调整

- 已将 `platforms/dirotter-macos` 加入 workspace，参与统一 `cargo check --workspace`。
- 便于后续把 macOS 专有资源（打包脚本、签名、Info.plist 模板）继续收口在该目录内。

## 5. 平台并存与编译验证建议

当前仓库保持 **Windows + macOS 双入口并存**：

- Windows 入口：`cargo run -p dirotter-app`
- macOS 入口：`cargo run -p dirotter-macos`

建议在各自原生系统上执行：

```bash
# 在 macOS 主机
cargo check -p dirotter-macos

# 在 Windows 主机
cargo check -p dirotter-app
```

> 说明：跨平台交叉检查通常还需要目标平台的 C/汇编工具链（例如 Apple SDK 或 MSVC 工具集）。在非对应主机上仅安装 Rust target 往往仍不足以通过所有依赖的本地编译步骤。

## 6. 后续建议（可选）

1. 为 `dirotter-macos` 增加 `.app` 打包脚本（`cargo-bundle` 或自定义 `xcodebuild` 流程）。
2. 增加 Finder 标签页/显示包内容等更细粒度行为映射。
3. 在 CI 增加 `x86_64-apple-darwin` 与 `aarch64-apple-darwin` 交叉检查。
