# DirOtter 系统设计说明书（SDD，2026-04-09）

## 1. 目标

DirOtter 的当前目标很明确：用尽量低摩擦的桌面流程，帮助用户在本地磁盘上快速找到可释放空间并直接处理。

默认产品主路径：

1. 扫描磁盘或目录
2. 汇总最大占用和规则驱动的清理建议
3. 在 Inspector 或建议详情里直接执行删除/快清
4. 在当前会话内确认结果

## 2. 当前成熟度

- 当前阶段：`Production Readiness`
- 当前默认存储：`settings.json + 当前会话临时快照`
- 当前最成熟平台：Windows

## 3. 架构总览

```text
Desktop UI (eframe/egui)
  -> App coordinator + page modules + view-models
  -> bounded event pipeline
  -> scan / core / actions / platform / cache / telemetry
  -> local filesystem + OS integration
```

workspace 模块职责：

- `dirotter-app`：原生窗口启动入口
- `dirotter-ui`：页面、状态协调、view-model、交互控制
- `dirotter-scan`：扫描 worker、聚合、发布链路
- `dirotter-core`：`NodeStore`、聚合值、Top-N 查询
- `dirotter-actions`：删除计划和执行
- `dirotter-platform`：Explorer、回收站、卷信息、fast cleanup staging
- `dirotter-cache`：设置与会话快照
- `dirotter-telemetry`：运行时观测与 diagnostics
- `dirotter-report`：文本/JSON/CSV 报告导出
- `dirotter-dup`：重复文件候选检测
- `dirotter-testkit`：阈值测试和基线守卫

## 4. UI 结构

当前 shell 固定为：

- 顶部工具栏
- 左侧导航
- 中央主内容区
- 右侧 Inspector
- 底部状态栏

主页面：

- `Overview`
- `Live Scan`
- `Result View`
- `Settings`

二级入口：

- `Errors`
- `Diagnostics`

当前 UI 组织方式已经从“大型单文件”收口为：

- `dashboard.rs` / `dashboard_impl.rs`
- `result_pages.rs`
- `settings_pages.rs`
- `advanced_pages.rs`
- `controller.rs`
- `cleanup.rs`
- `view_models.rs`

## 5. 扫描流水线

```text
root planning
  -> concurrent walker
  -> aggregator
  -> bounded publisher
  -> UI consumption
```

当前扫描设计要点：

- worker 并发枚举目录与元数据
- aggregator 负责乱序父子节点整合
- publisher 负责节流、快照和完成态发布
- UI 不直接承担重型扫描计算
- `Stop Scan` 通过后台轮询取消标记安全退出

扫描模式：

- `快速扫描（推荐）`
- `深度扫描`
- `超大硬盘模式`

三档模式都会完整扫描当前范围，差异只在扫描节奏和界面刷新策略。

## 6. 数据模型与性能策略

核心数据模型是 `NodeStore`。

当前已落地的性能策略：

- dirty 祖先传播
- dirty-only rollup
- entry-time 聚合维护
- 固定容量 top-k
- `Arc<str>` 共享路径
- live/full snapshot 类型拆分
- snapshot payload 与组装耗时阈值测试
- snapshot 运行时 telemetry

这意味着当前 snapshot 路径已经不再依赖“每次全树重算 + 大量重复字符串复制”。

## 7. 结果与恢复模型

当前结果模型有两个明确边界：

1. 默认不做跨会话历史数据库
2. 只在当前会话内保留临时恢复能力

具体做法：

- 设置落到 `settings.json`
- 结果恢复使用 `zstd+bincode` 临时快照
- 删除完成后的结果同步在后台进行
- Result View 默认不恢复完整快照，只用扫描完成时保留的 Top-N 结果画轻量 Treemap
- 如果持久设置目录不可写，自动回退到临时会话存储
- 临时 session 根目录会在退出时清理，并定期回收陈旧目录
- Overview / Live Scan 的 Top-N 证据直接来自扫描快照或完成态 Top-N，不在 UI 帧里再次访问文件系统过滤，避免权限、长路径或已清理路径把结果面板清空
- Result View 使用 Top-N 目录和文件生成固定画布 Treemap；完整树已驻留时才使用当前目录直接子项支持逐层进入

## 8. 清理与删除执行模型

清理分析层基于扫描完成后的 `NodeStore` 生成：

- 分类
- 风险分级
- 评分
- 每类 Top-N 候选

执行路径：

- 普通项：`Move to Recycle Bin`
- 高风险显式确认后：`Delete Permanently`
- 低风险缓存：`Fast Cleanup`

`Fast Cleanup` 当前语义：

- 目标先移入 `.dirotter-staging`
- UI 立即获得“已移出”的反馈
- 后台继续 purge
- 卷根 staging 不可写时回退到源路径父目录
- 删除完成后的默认收尾先走轻量同步：过滤摘要、Top-N、错误列表和清理建议；不为一键缓存清理强制重建完整结果树
- 低风险缓存规则覆盖 `Temp`、`Cache`、`.cache`、`LocalCache`、`INetCache`、`__pycache__` 等常见 Windows / 开发工具缓存根
- 清理分析不在完成收尾里对大量候选逐项调用 `fs::metadata()` 计算访问时间，避免把扫描完成后的整理阶段变成二次磁盘遍历

删除反馈设计：

- 后台线程逐项上报进度
- Inspector 和顶部横幅显示已处理/成功/失败数量
- 删除完成后的结果同步独立于 UI 主线程
- 失败详情收口到专门详情视图，而不是散落在外层摘要里

## 9. Diagnostics 与可观测性

当前 diagnostics 只关注当前会话。

当前会展示或支撑：

- 结构化诊断 JSON
- snapshot changed/materialized nodes
- ranked items
- text-bytes 估算
- 应用内存优化与 staging 清理维护动作

当前不再默认承担：

- 自动扫描历史
- 自动诊断归档
- 自动错误导出

## 10. 发布与质量门槛

当前质量门槛：

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`

当前发布门槛基础设施：

- `ci.yml`
- `release-windows.yml`
- `package-windows.ps1`
- `sign-windows.ps1`
- 安装/卸载脚本

## 11. 当前仍需补强的部分

1. 视觉回归自动化
2. 真实删除跨平台边界测试
3. Windows 正式签名链路落地
4. UI 协调层剩余复杂度继续收口
