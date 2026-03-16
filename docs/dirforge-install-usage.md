# DirForge 安装与使用文档（含性能阈值与批执行链路）

## 1. 新增能力（本次更新）

- Benchmark 套件与性能阈值固化（扫描与去重）
- 错误分类升级（User / Transient / System）
- 操作中心执行链路增强（回收站/永久删除模拟 + 批执行结果追踪）

## 2. 安装与构建

```bash
git clone <your-repo-url> DirForge
cd DirForge
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

## 3. 运行应用

```bash
cargo run -p dirforge-app
```

## 4. 关键使用流程

1. Dashboard 设置扫描参数（profile / batch / snapshot interval）并启动。
2. Current Scan 查看实时扫描与虚拟化文件列表。
3. Errors 页面查看分类统计（User/Transient/System）。
4. Operations 页面：
   - 生成删除计划
   - 运行“模拟回收站删除”或“模拟永久删除”
   - 查看批执行结果（success/failure + message）
5. Diagnostics 页面导出 `dirforge_diagnostics.json`。

## 5. Benchmark 与阈值验证

本项目已内置阈值测试：

- `benchmark_scan_threshold_small_tree`
- `benchmark_dup_threshold_small_dataset`

运行：

```bash
cargo test -p dirforge-testkit --test benchmark_thresholds
```

默认阈值（可在测试文件中调整）：

- 扫描阈值：`SCAN_THRESHOLD_MS = 4000`
- 去重阈值：`DUP_THRESHOLD_MS = 1200`

## 6. 运行产物

- `dirforge.db`：SQLite 缓存
- `dirforge_report.txt`：扫描报告
- `dirforge_diagnostics.json`：诊断导出

## 7. 常见问题

### 7.1 错误分类说明

- `User`：常见权限/访问问题
- `Transient`：临时性错误（网络/超时等）
- `System`：其余系统或内部错误

### 7.2 永久删除模拟失败

高风险文件在永久删除模拟下会被阻断并标记失败，这是安全策略设计。
