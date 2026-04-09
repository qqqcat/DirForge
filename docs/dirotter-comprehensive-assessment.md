# DirOtter 项目综合评估报告（2026-04-09）

## 1. 评估范围与方法

本轮复评覆盖：

1. workspace 与 crate 边界
2. 扫描、结果视图、清理建议、删除执行、存储恢复主链路
3. 工程门槛、CI、Windows 发布链路
4. 代码与文档一致性

实际复验结果：

- `cargo fmt --all -- --check`：通过
- `cargo check --workspace`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- `cargo build --workspace`：通过

已核对：

- `.github/workflows/ci.yml`
- `.github/workflows/release-windows.yml`
- `scripts/package-windows.ps1`
- `scripts/install-windows-portable.ps1`
- `scripts/uninstall-windows-portable.ps1`
- `dist/DirOtter-windows-x64-0.1.0-portable/BUILD-INFO.json`

## 2. 总体结论

DirOtter 当前处于 **Production Readiness**。

它已经不是“功能演示型原型”，而是一个主产品路径清晰、工程门槛基本成型、可继续稳定迭代的桌面工具。当前最主要的短板也已经从“主链路能不能工作”转向“发布成熟度、视觉回归和平台边界是否足够稳”。

结论拆开来看：

- 产品主路径已经成立：扫描 -> 清理建议 -> 直接处理 -> 结果确认。
- 工程主路径已经成立：`fmt + check + test + clippy -D warnings + build` 当前全绿。
- 发布基础设施已经成立：CI、Windows 便携打包、SHA-256 校验、安装脚本和可选签名入口都在仓库里。
- 仍未完全到达生产级：当前缺自动化视觉回归、真实删除跨平台覆盖不足、正式签名链路未实配。

## 3. 当前最稳的部分

### 3.1 产品主路径

- Overview 已明确承担首页角色，先给出 `一键提速（推荐）`、清理建议和关键证据区。
- 扫描入口已经用户化，三档模式替代了早期的技术参数暴露。
- Result View 已收口为“扫描完成后再看”的轻量目录下钻页，不再把实时重布局作为主方向。
- Inspector 已是危险动作主入口，支持回收站删除、永久删除确认和低风险缓存 `Fast Cleanup`。

### 3.2 扫描与结果链路

- `worker -> aggregator -> publisher` 的并发结构稳定，取消扫描和完成态收尾都有验证。
- `NodeStore` 已完成一轮真正的增量化收口：
  - dirty 祖先传播
  - dirty-only rollup
  - entry-time 聚合维护
  - 固定容量 top-k
- snapshot 路径已形成“少拷贝 + 稀疏 payload + 阈值守卫”的组合，而不是继续靠全量视图硬顶。

### 3.3 UI 架构

- `dirotter-ui` 仍然是最复杂的 crate，但首轮结构减债已经完成：
  - 页面模块拆出
  - controller 拆出
  - cleanup 规则拆出
  - view-model 拆出
- 当前更接近“协调层 + 页面层 + 展示整形层”的结构，而不再是单文件持续膨胀。

### 3.4 存储与恢复

- 默认产品模型已经完成去数据库化收口。
- 设置使用 `settings.json` 持久化。
- 结果恢复只保留当前会话临时 `zstd+bincode` 快照。
- 持久设置目录不可写时，会自动回退到临时会话存储，并在设置页明确提示。
- 临时 session 目录已具备退出清理和陈旧目录回收。

### 3.5 工程门槛与发布基础设施

- `clippy -D warnings` 已不再是一次性清理，而是当前可持续维持的门槛。
- `ci.yml` 已覆盖：`template validation + fmt + check + clippy + test + release build`
- `release-windows.yml` 已覆盖：测试、release 构建、可选签名、便携打包、checksum 上传和 tag 发布。
- 本地已存在 `0.1.0` 的 Windows 便携包与校验文件。

## 4. 当前主要风险

### 4.1 视觉与交互回归保护不足

- 目前仍缺最小截图回归或视觉 diff。
- 首页栅格、结果页高度、多语言撑开和右侧 Inspector 窄栏布局仍主要靠人工检查。

### 4.2 删除链路的跨平台成熟度不足

- 当前最成熟的是 Windows。
- 权限不足、文件占用、系统目录和回收站可见性等边界已有基础覆盖，但还不够深，尤其是跨平台维度。

### 4.3 正式发布仍缺签名落地

- 仓库支持可选签名，但当前本地产物仍是 `NotSigned`。
- 如果要进入正式对外分发，签名证书配置和验签流程必须成为发布门槛，而不是可选补充。

### 4.4 会话级结果模型是产品取舍，不是缺陷修复

- 当前结果默认只保留在当前会话内，这与“尽快释放空间”的定位一致。
- 但如果未来要支持跨会话历史分析，这将是产品范围变化，不是现有实现的小修小补。

## 5. 详细评估

### 5.1 Scan / Core

- `dirotter-scan` 与 `dirotter-core` 是当前最稳定的技术基础。
- 增量维护、共享路径和 payload 阈值测试已经把优化从“代码技巧”升级成“可守护能力”。

### 5.2 Actions / Platform

- `dirotter-actions` 已能稳定执行回收站删除、永久删除、失败分类和逐项进度回传。
- `dirotter-platform` 已承担 Explorer、卷信息、回收站与 staging 清理平台边界。
- 当前缺的不是基本能力，而是更完整的实机场景覆盖。

### 5.3 Cache / Telemetry / Report

- `dirotter-cache` 当前职责清楚：设置 + 会话快照。
- `dirotter-telemetry` 已能直接辅助判断 snapshot 是否重新膨胀。
- `dirotter-report` 仍保留导出能力，但已不再主导默认产品路径。

### 5.4 UI / Product Fit

- UI 当前已经明显更像“空间释放工具”，而不是“分析工作台”。
- `Errors / Diagnostics` 已经收进二级入口，这是正确方向。
- 当前 UI 最需要的不是继续加页面，而是给现有页面补可回归的稳定度保护。

## 6. 当前建议优先级（未来 2~4 周）

1. 为 Overview、Result View 和 Settings 建立最小视觉回归。
2. 扩展真实删除链路的跨平台与异常场景测试。
3. 配置 Windows 签名 secrets，并把签名验收纳入正式发布流程。
4. 继续收紧 `dirotter-ui` 协调层职责，但不再以“拆更多文件”为目标，而以“稳定边界、降低回归”为目标。

## 7. 最终判断

DirOtter 当前已经具备“可用、可测、可打包、可继续演进”的基础。

如果只问“项目现在是否健康”，答案是：健康，而且比 4 月初更稳。

如果问“是否已经完全生产级”，答案是：还没有。当前距离生产级的差距，主要在发布成熟度和体验回归保护，而不是主链路能力本身。
