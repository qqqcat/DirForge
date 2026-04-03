# Task Plan

## Goal
基于 2026-04-03 的整体代码评估结果，推动 DirOtter 从“中上质量、局部已有技术债”的状态，进入“高质量 Rust 工程实现”的下一阶段。

## Problem Statement
- `dirotter-ui` 已膨胀为单文件核心，状态、调度、业务规则和页面渲染混杂。
- 扫描快照链路仍包含明显的重复全量计算。
- 扫描事件流中存在不必要的字符串复制，Rust 的少拷贝优势发挥不充分。
- 工程门槛尚未完全收口到 `clippy -D warnings`。
- 现有计划文档停留在旧任务，不足以指导下一轮系统性改进。

## Optimization Strategy
1. 先做低风险高收益收口：
   - 修复当前静态检查失败
   - 修复已确认的低成本实现问题
   - 更新专项计划与评估文档
2. 再拆 UI 架构：
   - 分离 controller、analysis、diagnostics、pages、widgets
   - 降低 `DirOtterNativeApp` 的职责密度
3. 同步推动扫描链路增量化：
   - 降低快照期间的全量 `rollup + top-k` 成本
   - 让 dirty 标记真正参与增量更新
4. 最后收口数据流和持久化：
   - 减少字符串复制
   - 提升快照写入一致性
   - 固化 CI 质量门槛

## Plan Document
- 详细改进计划与实施计划见：
  - `docs/engineering-improvement-plan-2026-04.md`

## Execution Plan
- [x] 重新审视 workspace、核心 crate、扫描链路、平台层和 UI 主体。
- [x] 产出专项改进计划与实施计划文档。
- [x] 修复当前 `clippy -D warnings` 失败项。
- [x] 修复已确认的低成本实现问题：
  - 扫描发送阻塞时间采样
  - 删除计划中的路径类型判断
- [~] 分阶段拆解 `dirotter-ui`。
  - [x] 抽离 `cleanup analysis` 到独立模块
  - [x] 抽离 controller
  - [ ] 抽离页面模块
- [ ] 分阶段实现扫描快照增量化。
- [ ] 分阶段瘦身扫描消息链路与快照持久化。

## Verification Plan
1. 工程验证：
   - `cargo fmt --all`
   - `cargo build --workspace`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
2. 文档验证：
   - 更新专项计划、综合评估、发现记录和进展记录
3. 后续阶段验证：
   - 为 UI 拆分和扫描增量化补对应回归测试

## Verification Status
- `cargo fmt --all`：已通过
- `cargo build --workspace`：已通过
- `cargo test --workspace`：已通过
- `cargo clippy --workspace --all-targets -- -D warnings`：已通过

## Result
- 本轮重点已切换为“质量收口 + 架构减债”。
- 详细计划已独立沉淀到 `docs/engineering-improvement-plan-2026-04.md`。
- 当前阶段的目标不是继续堆功能，而是为下一轮高质量重构建立边界、顺序和验证门槛。
- 当前仓库已经完成一次质量门槛收口，后续 UI 拆分和扫描增量化可以在更干净的基线上推进。
- Phase 1 已正式启动，`cleanup analysis` 已完成第一次模块拆分。

## Follow-up: Result View Simplification
- Treemap 不再按实时扫描刷新。
- 新结果页只在扫描完成后工作，并优先读取扫描完成后的结果树 / 缓存快照。
- 结果页只展示当前目录的直接子项，支持逐层下钻与返回上级。
- 目标是保留“看目录占比”的核心价值，同时避免百万节点和重布局算法拖垮 UI。

## Follow-up: Result View Layout Optimization
- 结果页不再使用自然高度塌缩的内容卡。
- 页面结构调整为“顶部摘要 + 底部填充型结果区”。
- 条形图区必须吃满剩余高度，长列表走结果区内部滚动，而不是留下大面积未利用空白。

## Follow-up: Cleanup Suggestion System (V1)
- 目标：把首页从“只看数据”推进到“给出可执行的释放空间建议”。
- 分析层：
  - 基于扫描完成后的 `NodeStore`
  - 规则分类 `cache / downloads / video / archive / installer / image / system / other`
  - 风险分级 `Low / Medium / High`
  - 评分采用 `size + unused_days + category bias`
- UI：
  - Overview 顶部新增 `清理建议` 卡
  - 支持 `查看详情`
  - 支持 `一键清理缓存（推荐）`
- 执行：
  - 快捷清理默认走回收站
  - 详情窗里绿色默认勾选、黄色默认不勾选、红色锁定不可删
- 验证：
  - `cargo fmt --all`
  - `cargo test --workspace`
  - `cargo build -p dirotter-app`

## Follow-up: Overview / Settings Clipping Fix
- 问题：
  - 首页在新增清理建议后，首屏更紧，底部卡片容易出现“像被截断”的观感
  - Settings 页在页面级宽度约束内部又套了固定宽度容器，右侧卡片更容易贴边
- 修正：
  - 为滚动页统一补充底部安全区
  - Settings 页移除多余的二次固定宽度裁切
  - 首页卡片纵向间距做轻量压缩
  - 卡片和提示条统一走放宽 `clip rect` 的渲染路径，避免描边被子布局裁剪矩形截断
  - 首页和设置页改成纵向章节布局，不再继续依赖双列卡片矩阵
  - Settings 再进一步改为“窄内容列 + 分组设置行”，参考主流设置页模式重做
  - Overview 不再套用设置页的章节样式，改为主流 dashboard 结构：Hero 结论区、KPI 指标条、双列操作区、双列证据区
  - Overview 进一步改为独立首页宽度 + 显式双列宽度分配，修正卷空间摘要漂移、卡片重叠和左右留白不对称
  - 首页继续把专用宽度从 `1240` 收到 `1160`，外层 gutter 提高到 `64`，优先修正视觉上右侧贴 Inspector 的问题
  - KPI 指标条内的四张卡片改为强制填满分配列宽，修正卡片本身缩在左侧造成的假性不对称
  - 清理建议详情窗改为居中受控对话框，补齐关闭入口，并为右侧大小列预留固定宽度，修正长路径导致的右侧截断
  - Overview 再次收口信息架构：移除与 KPI 重复的 `卷空间摘要` 大卡，卷级信息并入首页四张指标卡与全宽扫描卡
  - 顶部四张卡调整为唯一指标：`磁盘已用 / 磁盘可用 / 已扫描体积 / 错误`
  - 首页扫描卡继续收口为“盘符优先、手动目录次要”，移除顶部解释文案，并把缩小后的路径输入框移到盘符区之后
  - 停止扫描改为真正的可退出流程，避免 worker 在条件变量等待时挂死 UI
  - SQLite 快照改为“每个根路径只保留最新一份”，并在写入后主动 checkpoint WAL，避免数据库文件随重复扫描持续膨胀
  - `一键清理缓存` 改为 `staging -> 后台 purge`，先追求秒级体感反馈
  - Windows 文件永久删除接入更低层 fast path，失败时保留现有删除回退
  - 启动时自动清理 `.dirotter-staging` 遗留项，并在扫描阶段排除该内部目录

## Follow-up: French / Spanish Localization
- 目标：
  - 在现有中英文基础上增加法语与西班牙语
  - 保持现有 `self.t(zh, en)` 调用模型，避免大范围重构 UI 调用点
- 实现：
  - 新增 `Lang::Fr / Lang::Es`
  - 新增 `language` 设置值解析与保存：`en / zh / fr / es`
  - 根据 `LC_ALL / LANG` 自动识别 `zh / fr / es / en`
  - 用英文文案作为稳定键，向法语 / 西班牙语词典做映射
  - 法语 / 西班牙语必须补齐当前 UI 全量英文键，不接受说明文案回退英文
- 验证：
  - `cargo fmt --all`
  - `cargo check --workspace`
  - `cargo build --workspace`
  - `cargo test --workspace`
  - 额外增加源码级词典覆盖测试，防止未来新增英文文案后漏翻

## 2026-03-20 Product Refocus: 从“分析器”转向“释放空间工具”

### 用户目标重述
- 用户打开 DirOtter 的首要诉求不是“保存扫描历史”或“导出错误 CSV”，而是尽快知道：
  - 现在能释放多少空间
  - 先删什么最安全
  - 点一次后能否立刻见效

### 当前低价值 / 高成本项
- 扫描完成自动保存完整快照到 SQLite
- 扫描完成自动写入扫描历史
- 扫描完成自动导出错误 CSV
- 主导航持续暴露 `History / Errors / Diagnostics`

### 保留项
- 三档扫描模式与盘符快捷入口
- Overview 顶部清理建议与 `一键清理缓存`
- 最大文件夹 / 最大文件证据区
- Inspector 删除执行与风险提示
- `.dirotter-staging -> 后台 purge` 极速缓存清理链路

### 优化方案
1. 扫描完成默认不再自动落 SQLite 快照、不再自动写历史、不再自动导出错误 CSV  
   - 这些功能改成手动或开发者模式入口
   - 默认完成态只保留用户可见结果和清理动作
2. `重算整套清理建议` 重新定义为“生成当前用户真正能操作的候选列表”  
   - 不再为整棵树做重型全量分析
   - 只保留规则命中的候选、每类 Top-N、可执行动作与预计释放空间
3. `History / Errors / Diagnostics` 从主路径降级  
   - 进入二级入口或仅在调试模式显示
4. 新增内存相关能力时，避免误导性“系统一键释放内存”承诺  
   - 优先考虑 `减小 DirOtter 占用` 或 `刷新资源占用`
   - 如需系统级内存功能，应单独标为实验性工具，而不是主卖点

### 实施方案
#### Phase 1: 去掉默认重收尾
- 完成扫描后只保留：
  - summary
  - top files / top dirs
  - 当前会话内可用的清理建议
- SQLite 快照 / 历史 / 错误 CSV 改为：
  - 用户手动点击保存
  - 或仅在开发诊断模式启用

#### Phase 2: 重构清理建议计算
- 把当前 `build_cleanup_analysis(store)` 从“全量扫全树”收口为：
  - 规则命中目录优先
  - 文件只保留超过阈值且可操作的候选
  - 每类保留 Top-N
  - 背景增量更新，不阻塞完成态切换

#### Phase 3: 收口导航
- 主导航优先保留：
  - Overview
  - Live Scan
  - Result View
  - Settings
- `History / Errors / Diagnostics` 收入二级入口或开发者开关

#### Phase 4: 内存能力取舍
- 不建议承诺“系统一键释放内存”
- 如确需提供，优先做：
  - `减小 DirOtter 占用`
  - `清理残留 staging`
  - `刷新资源占用`
- 这类功能必须明确标注为辅助工具，不替代磁盘空间清理主链路

### 实施完成状态（2026-03-20）
1. Phase 1 已完成
   - 扫描完成默认不再自动保存 SQLite 快照
   - 扫描完成默认不再自动写入历史
   - 扫描完成默认不再自动导出错误 CSV
   - 完成态只保留轻量结果整理与清理建议生成
2. Phase 2 已完成
   - 清理建议已改为规则命中候选生成
   - 每类候选带 Top-N 上限
   - 全局候选总量带上限，避免扫描结束后再做整树重分析
3. Phase 3 已完成
   - 主导航已收口为 `Overview / Live Scan / Result View / Settings`
   - `History / Errors / Diagnostics` 已移入 `高级工具 / Advanced Tools`
   - 高级工具开关放入 Settings，并持久化到本地设置
4. Phase 4 已完成
   - Inspector 已新增 `释放 DirOtter 内存`
   - Inspector 已新增 `清理残留 staging`
   - 诊断页已新增手动保存当前快照 / 手动记录扫描摘要 / 手动导出错误 CSV

### 最终验证
- `cargo fmt --all`：通过
- `cargo test --workspace`：通过
- `cargo build -p dirotter-app`：通过

### Phase 1 增量进展（2026-04-03）
1. 已完成
   - 抽离 `cleanup.rs`
   - 抽离 `controller.rs`
   - 抽离 `dashboard.rs + dashboard_impl.rs`
2. 当前收益
   - `dirotter-ui/src/lib.rs` 不再同时承载 cleanup 规则、后台 controller、以及 dashboard 页面细节
   - 后续页面层拆分路径已明确为 `current_scan -> treemap -> diagnostics`
3. 当前验证
   - `cargo fmt --all`：通过
   - `cargo test -p dirotter-ui`：通过
   - `cargo clippy -p dirotter-ui --all-targets -- -D warnings`：通过
   - `cargo test --workspace`：通过
   - `cargo clippy --workspace --all-targets -- -D warnings`：通过
