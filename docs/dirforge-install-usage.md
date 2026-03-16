# DirForge 安装与使用文档（生产化版本）

## 1. 当前版本说明

本版本已实现以下生产化特性：

- M1：扫描事件批量化、Snapshot coalescing、扫描 profile（SSD/HDD/Network）
- M2：去重四阶段（size/partial/full/结果整形）基础实现
- M3：Treemap 主视图、大列表虚拟化、帧预算与队列深度监控
- M4：SQLite schema migration、操作中心、诊断页与诊断包导出

## 2. 环境要求

- Rust stable + Cargo
- Git
- 支持桌面图形环境（运行 eframe 原生窗口）

## 3. 安装与构建

```bash
git clone <your-repo-url> DirForge
cd DirForge
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

## 4. 运行方式

```bash
cargo run -p dirforge-app
```

Release 构建：

```bash
cargo build -p dirforge-app --release
```

## 5. 首次使用流程

1. Dashboard 选择扫描路径、扫描 profile、batch size、snapshot 间隔。
2. 启动扫描，进入 Current Scan 查看实时指标与虚拟化文件列表。
3. 切到 Treemap 查看目录空间分布与交互。
4. 扫描完成后在 History 查看历史记录，在 Errors 查看错误详情。
5. 在 Operations 查看删除计划（含高风险统计）并记录审计事件。
6. 在 Diagnostics 导出 `dirforge_diagnostics.json` 诊断包。
7. 在 Settings 调整语言（默认跟随系统 `LC_ALL`/`LANG`）和主题。

## 6. 运行产物

- `dirforge.db`：SQLite（snapshots / scan_history / scan_errors / settings / operation_audit）
- `dirforge_report.txt`：扫描报告
- `dirforge_diagnostics.json`：诊断导出文件

## 7. 常见问题

### 7.1 首次构建慢

`eframe` 图形依赖首次编译耗时较长，属于正常现象。

### 7.2 语言默认不符合预期

应用优先读取 `LC_ALL`，回退 `LANG`。可在 Settings 中覆盖并持久化。

### 7.3 历史为空

仅扫描完成后会写入 `scan_history`，取消扫描可能无完整记录。

## 8. 清理

```bash
cargo clean
rm -f dirforge.db dirforge_report.txt dirforge_diagnostics.json
```

## 9. 相关文档

- `docs/dirforge-comprehensive-assessment.md`
- `docs/dirforge-sdd.md`
- `docs/dirforge-ui-component-spec.md`
