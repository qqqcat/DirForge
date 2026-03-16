# DirForge 安装与使用文档

> 本文档适用于当前仓库版本（原生 `egui/eframe` GUI + SQLite 缓存实现）。

## 1. 文档目标

本文档覆盖以下内容：

- 安装前环境准备
- 源码获取与构建
- GUI 应用运行方式
- 首次使用流程（扫描 / 历史 / 错误中心 / 设置）
- 多语言默认行为（系统语言）
- 常见问题排查与清理

## 2. 当前版本能力概览

当前 DirForge 已具备：

- 原生桌面 GUI（`egui/eframe`）
- 文件系统扫描与目录聚合
- Top-N 大文件统计
- 重复文件候选检测（按文件大小分组）
- SQLite 结构化缓存（快照、历史、错误、设置）
- 历史快照页、错误中心页、设置页
- 中英文界面支持，默认跟随系统语言环境

## 3. 环境要求

### 3.1 操作系统

- Linux / macOS / Windows（均可开发构建）
- 产品目标平台为 Windows（Windows 集成能力优先完善）

### 3.2 必需软件

- Rust stable（建议最新稳定版）
- Cargo（随 Rust 安装）
- Git

### 3.3 依赖说明

项目使用 `rusqlite` 并启用 `bundled` 特性，会自动构建/链接 SQLite，无需手动预装 SQLite CLI。

### 3.4 环境自检

```bash
rustc --version
cargo --version
```

## 4. 获取源码

```bash
git clone <your-repo-url> DirForge
cd DirForge
```

## 5. 构建与测试

### 5.1 格式化

```bash
cargo fmt --all
```

### 5.2 编译检查

```bash
cargo check --workspace
```

### 5.3 运行测试

```bash
cargo test --workspace
```

## 6. 运行应用（GUI）

### 6.1 开发模式运行

```bash
cargo run -p dirforge-app
```

运行后会启动原生桌面窗口（不是控制台 TUI）。

### 6.2 构建 release 版本

```bash
cargo build -p dirforge-app --release
```

产物：

- Linux/macOS: `target/release/dirforge-app`
- Windows: `target\release\dirforge-app.exe`

## 7. 首次启动与默认行为

### 7.1 系统语言默认策略

应用启动时读取系统语言环境变量：

- 优先 `LC_ALL`
- 回退 `LANG`

默认规则：

- 以 `zh` 开头 → 中文
- 其他 → 英文

你可以在 Settings 页面手动切换语言，设置会持久化到 SQLite。

### 7.2 主题默认策略

默认深色主题，可在 Settings 页面切换深/浅色并持久化。

### 7.3 默认缓存数据库

应用默认在仓库运行目录创建：

- `dirforge.db`（SQLite 文件）

## 8. 基础使用流程

### 第一步：Dashboard 启动扫描

1. 打开应用后进入 Dashboard
2. 在 “Scan root / 扫描根路径” 输入目标路径
3. 点击 “Start Scan / 开始扫描”

### 第二步：Current Scan 查看结果

扫描过程中可查看：

- 文件数、目录数、扫描字节、错误数
- Top 20 大文件
- 重复候选汇总信息

### 第三步：History 查看历史快照

扫描完成后会自动记录到历史：

- 扫描根路径
- 文件/目录统计
- 字节量
- 错误数
- 时间戳

在 History 页面可选择历史项并联动查看对应错误。

### 第四步：Error Center 查看错误详情

错误中心展示扫描错误明细：

- 失败路径
- 失败原因（如 `metadata` / `read_dir` 错误）

### 第五步：Settings 修改偏好

可配置：

- 语言（中文 / English）
- 主题（Dark / Light）

设置项会持久化到 SQLite `settings` 表。

## 9. 缓存与数据说明（SQLite）

当前缓存结构包含：

- `snapshots`：扫描节点快照（JSON）
- `scan_history`：历史汇总
- `scan_errors`：按历史记录关联的错误条目
- `settings`：应用设置键值对

说明：

- 应用会尝试加载当前 root 的最近快照
- 每次扫描完成后会写入快照 + 历史 + 错误

## 10. 导出与文件产物

### 10.1 报告导出

扫描完成后会导出：

- `dirforge_report.txt`

### 10.2 运行目录常见文件

- `dirforge.db`：SQLite 缓存
- `dirforge_report.txt`：文本报告

## 11. 常见问题（FAQ）

### 11.1 启动慢或首次编译耗时长

首次构建 `eframe` 及图形栈依赖会较慢，属于正常现象。

### 11.2 `cargo check` 或 `cargo test` 失败

建议：

```bash
cargo clean
cargo check --workspace
cargo test --workspace
```

并确认 Rust 版本与网络可用性。

### 11.3 界面语言不符合预期

- 检查系统 `LC_ALL` / `LANG`
- 在 Settings 页面手动切换并保存

### 11.4 看不到历史或错误

仅扫描完成后才会落库历史。若扫描被取消或崩溃，可能没有完整记录。

### 11.5 Windows “在资源管理器中显示”行为

平台封装在非 Windows 环境会返回 `Unsupported`，请在 Windows 实机验证。

## 12. 升级与兼容说明

当前 schema 采用 `CREATE TABLE IF NOT EXISTS` 初始化策略。
后续如字段变更，建议增加显式 schema 版本与迁移脚本。

## 13. 清理与卸载

### 13.1 清理构建产物

```bash
cargo clean
```

### 13.2 清理运行数据

```bash
rm -f dirforge.db dirforge_report.txt
```

### 13.3 完全移除

删除仓库目录即可。

## 14. 相关文档

- 系统设计：`docs/dirforge-sdd.md`
- UI 规格：`docs/dirforge-ui-component-spec.md`
- 快速上手：`docs/quickstart.md`
