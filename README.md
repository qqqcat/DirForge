# DirOtter

DirOtter 是一个基于 Rust 的本地磁盘分析器，当前产品定位已经明确收口为：

- 先用推荐策略扫描磁盘
- 再给出可执行的释放空间建议
- 最后直接在应用内完成删除或缓存快清

## 当前状态（2026-05-03）

当前项目已正式达到 **Production Readiness** 标准，具备稳定的端到端主链路，所有质量门禁均已通过。

本次仓库复验结果（2026-05-03）：

- `cargo fmt --all -- --check`：✅ 通过
- `cargo check --workspace`：✅ 通过（0 errors, 0 warnings）
- `cargo test --workspace`：✅ **94 个测试全部通过**
  - dirotter-actions: 6 passed
  - dirotter-cache: 2 passed
  - dirotter-core: 9 passed
  - dirotter-dup: 5 passed
  - dirotter-platform: 10 passed
  - dirotter-report: 4 passed
  - dirotter-scan: 7 passed
  - integration_scan: 7 passed
  - dirotter-telemetry: 2 passed
  - benchmark_thresholds: 4 passed
  - dirotter-ui: 38 passed
- `cargo clippy --workspace --all-targets -- -D warnings`：✅ 通过（已修复 1 个 warning）
- `cargo build --workspace`：✅ 编译成功

本地发布链路现状：

- 已存在 Windows 便携包：`dist/DirOtter-windows-x64-1.0.1-portable.zip`
- 已生成校验文件：`dist/DirOtter-windows-x64-1.0.1-portable.zip.sha256.txt`
- `dist/DirOtter-windows-x64-1.0.1-portable/BUILD-INFO.json` 已记录当前打包时间、commit 和签名状态
- 当前本地产物状态：`NotSigned`

## 当前已落地能力

- 扫描链路：`worker -> aggregator -> publisher` 并发扫描、批次发布、节流、取消与完成态收尾。
- 用户扫描体验：默认只暴露 `推荐策略`；`复杂目录` 与 `外置/超大硬盘` 保留在高级扫描节奏中，三者扫描范围和结果完整性一致，只调整批处理和界面刷新频率。
- 首页主路径：Overview 会优先展示 `一键提速（推荐）`、清理建议摘要、最大文件夹和所选范围最大项目，而不是暴露底层参数。
- 清理建议：规则驱动分类、风险分级、评分和每类 Top-N 收口，优先把缓存、下载、安装包等可执行候选提到前面；低风险缓存覆盖 `Temp`、`Cache`、`.cache`、`LocalCache`、`INetCache`、`__pycache__` 等常见目录。
- 重复文件审阅：`Duplicate Files` 页面会先按大小收敛候选，再在后台补算哈希；快速去重保留低风险位置约束但放宽到 1 MB / 8 MB 门槛，结果按组展示，并给出推荐保留项、默认删除候选和高风险默认不自动选策略。
- 清理执行：支持 `Move to Recycle Bin`、`Delete Permanently` 和低风险缓存项的 `Fast Cleanup`。
- 删除反馈：后台删除线程会逐项回传已处理/成功/失败统计和当前处理项；一键缓存清理完成后优先轻量同步摘要、Top-N 和清理建议，不为刷新 UI 强制重建完整结果树。
- 结果浏览：不再提供独立结果视图；完成态证据收口到 Overview 的最大文件夹/文件区和右侧 Inspector，避免低价值 Treemap 入口分散主路径。
- 轻量存储：设置使用 `settings.json` 持久化；结果恢复只使用当前会话临时 `zstd+bincode` 快照，不再依赖历史数据库。
- 设置容错：如果持久设置目录不可写，应用会自动回退到临时会话存储，并在设置页明确提示。
- 多语言：支持 19 种语言选择，其中 `中文 / English / Français / Español` 为完整 UI 文案，其余语言当前以英文回退。
- 发布准备：仓库已包含正式 CI、Windows 发布 workflow、打包脚本、安装脚本和可选代码签名入口。

## 当前工程判断

当前仓库已经正式达到 **生产就绪（Production Ready）** 状态，主要工程面判断如下：

**✅ 质量门禁全部通过：**
- 代码格式：100% 符合 Rust 规范
- 编译检查：0 errors, 0 warnings
- 测试覆盖：94 个测试，100% 通过率
- 代码质量：Clippy 全部规则通过
- 构建稳定性：Debug/Release 均编译成功

**核心工程成果：**

- `dirotter-ui` 的首轮拆分已经完成，页面层、controller、cleanup 分析和 view-model 已不再全部堆在单文件里。
- 扫描快照链路已经完成一轮真正的增量化和少拷贝收口：
  - dirty 祖先传播
  - entry-time 聚合维护
  - 共享 `Arc<str>` 路径
  - live/full snapshot 类型拆分
  - payload 阈值测试和运行时 telemetry
- 轻量存储模型已经稳定：默认无数据库、事务式写入、会话快照、退出清理与陈旧临时目录回收都已落地。
- 工程门槛已经收口到 `fmt + check + test + clippy -D warnings`，并进入 CI。
- 测试体系完善：包含单元测试、集成测试、性能基准测试、属性测试和多语言回归测试。

## 已知限制与未来改进

### 当前限制
- ⚠️ **视觉回归测试**：缺少自动化视觉回归，页面布局仍依赖人工检查
- ⚠️ **跨平台删除**：真实删除链路在 Windows 最成熟，其他平台覆盖待深化
- ⚠️ **会话结果**：结果默认只保留在当前会话内（符合产品定位），不支持跨会话历史分析
- ⚠️ **代码签名**：Windows 代码签名链路已接好，但需要配置 secrets 才能生成签名产物

### 已解决的问题（2026-05-03）
- ✅ **Clippy 警告**：修复了 `render_ranked_size_list` 函数的 `too_many_arguments` 警告
- ✅ **测试覆盖**：新增 dirotter-ui 测试，总测试数从 87 提升到 94
- ✅ **构建稳定性**：解决了文件占用导致的构建失败问题

### 未来改进方向
1. **高优先级**：配置 Windows 代码签名 secrets
2. **中优先级**：引入自动化视觉回归测试（截图对比）
3. **中优先级**：扩展跨平台测试覆盖（Linux/macOS）
4. **低优先级**：可选的历史分析结果持久化功能

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
# 运行应用
cargo run -p dirotter-app

# 发布构建
cargo build --release -p dirotter-app
```

质量检查（全部必须通过）：

```bash
# 代码格式检查
cargo fmt --all -- --check

# 编译检查
cargo check --workspace

# 测试（当前 94 个测试）
cargo test --workspace

# 代码质量检查（Clippy）
cargo clippy --workspace --all-targets -- -D warnings

# 完整构建验证
cargo build --workspace
```

**质量门禁**：所有检查必须通过才能合并到主分支。

## 发布与安装

### 当前发布状态
- ✅ **Windows 便携包**：`dist/DirOtter-windows-x64-1.0.1-portable.zip`
- ✅ **校验文件**：`dist/DirOtter-windows-x64-1.0.1-portable.zip.sha256.txt`
- ⚠️ **签名状态**：`NotSigned`（需配置 secrets 进行代码签名）

### CI/CD 流程
- **CI 检查**：`.github/workflows/ci.yml`（自动运行 fmt/check/test/clippy）
- **Windows 发布**：`.github/workflows/release-windows.yml`
- **便携打包**：`scripts/package-windows.ps1`
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
