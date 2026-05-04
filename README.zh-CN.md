# DirOtter

<p align="center">
  <img src="docs/assets/dirotter-icon.png" alt="DirOtter 应用图标" width="160">
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.de.md">Deutsch</a>
</p>

**DirOtter** 是一个使用 Rust 构建的开源、本地优先磁盘分析与清理助手。

它帮助用户理解磁盘空间被哪些目录和文件占用，识别大文件夹、大文件和重复文件候选项，并安全清理低风险缓存或临时文件，不会把文件系统数据上传到任何云服务。

DirOtter 的定位是透明、保护隐私、面向日常使用，为用户提供一种比黑箱式磁盘清理工具更安全的替代方案。

## 项目状态

DirOtter 目前处于早期但已经生产就绪的阶段。

核心 Windows 应用已经可用、已测试，并已打包为便携构建。当前项目已经通过格式检查、编译检查、测试、lint 和 workspace 构建验证。

当前验证状态：

- `cargo fmt --all -- --check` 通过
- `cargo check --workspace` 通过，0 errors，0 warnings
- `cargo test --workspace` 通过，94 个测试
- `cargo clippy --workspace --all-targets -- -D warnings` 通过
- `cargo build --workspace` 成功

仓库已经包含 CI workflow、Windows 发布打包、便携安装脚本和可选代码签名入口。

## 为什么需要 DirOtter

现代操作系统和应用会产生大量缓存、临时文件、下载安装包、重复资源和隐藏的空间占用。现有清理工具常见的问题是过于黑箱、过于激进，或者过度依赖特定平台假设。

DirOtter 希望提供一种更安全、更透明的方式：

1. 使用可预测的策略扫描本地磁盘。
2. 解释空间被什么占用。
3. 按风险级别推荐清理候选项。
4. 让用户在删除前先审阅。
5. 优先使用移入回收站这类可逆操作。
6. 默认把文件系统数据保留在本地。

长期目标是成为一个可靠的开源磁盘分析与清理工具，覆盖 Windows、macOS 和 Linux。

## 核心功能

### 磁盘扫描

DirOtter 会扫描用户选择的目录，并构建结构化的磁盘占用视图。

扫描链路支持：

- 并发扫描
- 批量发布
- 节流 UI 更新
- 取消
- 完成态处理
- 轻量级会话快照

默认面向用户的扫描模式聚焦推荐策略；对于复杂目录或大型外置硬盘，也可以调整高级扫描节奏。

### 清理建议

DirOtter 使用规则分析来识别潜在清理候选项。

推荐类别包括：

- 临时文件
- 缓存目录
- 浏览器或应用缓存路径
- 下载的安装包
- 常见低风险生成文件
- 值得用户审阅的大文件和大文件夹

推荐项会按风险级别评分和分组，优先展示更安全的候选项。

### 重复文件审阅

DirOtter 可以使用先按大小分组、再后台哈希校验的策略识别重复文件候选项。

重复文件审阅流程刻意避免激进的自动删除。它会展示候选分组，推荐保留文件，并避免自动选择高风险位置。

### 清理执行

支持的清理动作包括：

- 移入回收站
- 永久删除
- 对低风险缓存候选项执行快速清理

清理执行会在后台处理文件时报告进度和结果统计。

### 本地优先存储

DirOtter 的正常使用不需要数据库。

设置保存在轻量的 `settings.json` 文件中。会话结果只保存为临时压缩快照，并会在不再需要时移除。

如果设置目录不可写，DirOtter 会回退到临时会话存储，并在设置 UI 中明确提示。

### 国际化

DirOtter 支持选择 19 种语言：

- 阿拉伯语
- 中文
- 荷兰语
- 英语
- 法语
- 德语
- 希伯来语
- 印地语
- 印尼语
- 意大利语
- 日语
- 韩语
- 波兰语
- 俄语
- 西班牙语
- 泰语
- 土耳其语
- 乌克兰语
- 越南语

当前 UI 翻译门禁已经覆盖所有支持语言的已发布 UI 文案。新增用户可见 UI 字符串在合并前应为每一种可选语言补齐翻译。

## 安全模型

DirOtter 对删除操作保持保守。

清理属于安全敏感操作，因为错误删除可能造成数据损失。因此 DirOtter 围绕以下安全原则设计：

- 执行前展示清理候选项
- 按风险级别分类推荐项
- 优先使用回收站进行可逆删除
- 避免自动选择高风险重复文件候选项
- 明确区分永久删除
- 将快速清理限制在低风险缓存或临时路径
- 清楚展示操作结果和失败原因

未来工作包括更深入审计不同平台的回收站行为、高风险路径、符号链接、权限失败和不可逆删除边界。

## 工作区结构

```text
crates/
  dirotter-app        # 原生应用入口
  dirotter-ui         # UI、页面、view model、交互状态
  dirotter-core       # Node store、聚合与查询
  dirotter-scan       # 扫描事件流与聚合发布
  dirotter-dup        # 重复文件候选检测
  dirotter-cache      # settings.json 与会话快照存储
  dirotter-platform   # Explorer 集成、回收站、卷信息、清理 staging
  dirotter-actions    # 删除计划与清理执行
  dirotter-report     # 文本、JSON、CSV 报告导出
  dirotter-telemetry  # 诊断与运行时指标
  dirotter-testkit    # 回归与性能测试工具
```

## 构建与运行

### 前置要求

- Rust stable toolchain
- Cargo
- 受支持的桌面平台

Windows 是当前最成熟的目标平台。macOS 和 Linux 支持属于跨平台路线图的一部分。

### 运行应用

```bash
cargo run -p dirotter-app
```

### 发布构建

```bash
cargo build --release -p dirotter-app
```

### 质量门禁

合并变更前应通过以下检查：

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

## 发布与打包

仓库包含 Windows 发布 workflow 和打包脚本。

当前发布相关组件包括：

- 用于格式、检查、测试和 lint 的 CI workflow
- Windows 发布 workflow
- Windows 便携打包脚本
- 可选 Windows 代码签名脚本
- 便携安装脚本
- 便携卸载脚本

当前 Windows 产物包括便携 ZIP 构建和 SHA-256 校验文件。

发布流水线支持代码签名，但在生成签名构建前需要先配置 secrets。

## 路线图

DirOtter 当前重点是提升可靠性、安全性和跨平台支持。

高优先级和中优先级路线图包括：

1. 配置 Windows 代码签名 secrets，生成签名发布产物。
2. 增加 UI 自动化视觉回归测试。
3. 扩展 Linux 文件系统与 trash/delete 行为覆盖。
4. 扩展 macOS 文件系统与 trash/delete 行为覆盖。
5. 审计清理与删除安全边界。
6. 改进发布自动化与 changelog 生成。
7. 改进贡献者文档。
8. 为大型目录、符号链接、权限错误和外置硬盘增加更多集成测试。
9. 在新增用户可见字符串时持续保持 19 种 UI 语言覆盖。
10. 在保持默认体验轻量、本地优先的前提下，评估可选历史持久化。

## Codex 可以如何帮助这个项目

DirOtter 很适合 AI 辅助开源维护，因为它具备真实的多 crate Rust 代码库、安全敏感的文件系统行为、跨平台目标和持续维护负担。

适合 Codex 辅助的开源维护任务包括：

- 审阅 workspace 内的 Rust 变更
- 分诊 issue 并复现 bug
- 改进扫描、清理、重复检测和报告逻辑的测试覆盖
- 审计清理安全规则
- 检查平台特定边界情况
- 改进 CI 和发布 workflow
- 生成并审阅文档更新
- 协助维护翻译一致性
- 起草 pull request 摘要和发布说明

Codex 支持可以帮助项目保持完全开源，同时降低维护成本，让 DirOtter 更安全、更可靠，并在更多平台上更有用。

## 贡献

欢迎贡献。

有价值的贡献方向包括：

- 文件系统扫描性能
- 清理安全规则
- 重复文件审阅体验
- Windows 回收站行为
- Linux 和 macOS 平台支持
- UI 测试
- 视觉回归测试
- 可访问性改进
- 文档
- 翻译
- 打包与发布自动化

提交 pull request 前，请运行完整质量门禁：

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

更详细的贡献者文档应补充到 `CONTRIBUTING.md`。

## 安全

DirOtter 会处理本地文件系统数据和清理操作，因此安全和防止数据损失是项目的重要关注点。

如发现潜在安全或数据损失问题，请尽可能私下报告。后续应通过专门的 `SECURITY.md` 定义推荐报告渠道、支持版本和披露流程。

需要特别关注的领域包括：

- 不安全的删除行为
- 错误的高风险路径分类
- 符号链接或 junction 遍历问题
- 权限边界问题
- 平台特定 trash/recycle-bin 失败
- 不可逆删除 bug
- 错误清理推荐

## 隐私

DirOtter 是本地优先应用。

应用默认在本地分析文件系统元数据，不会把扫描结果、文件路径或清理建议上传到云服务。

未来任何 telemetry 或 crash reporting 都应 opt-in、清晰记录，并保护隐私。

## 许可证

当前 workspace 在 `Cargo.toml` 中声明项目许可证为 MIT。正式扩大分发前，应在仓库根目录补充 `LICENSE` 文件。

## 项目目标

DirOtter 的目标是成为一个用户可以信任的透明、本地优先、开源磁盘分析与清理工具。

项目优先级包括：

- 安全优先于激进清理
- 可解释性优先于黑箱自动化
- 本地处理优先于云依赖
- 可维护性优先于短期功能膨胀
- 跨平台可靠性优先于平台特定捷径
