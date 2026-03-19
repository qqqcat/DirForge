# Task Plan

## Goal
将 DirOtter 的扫描入口从“技术参数调优”改为“用户可理解的扫描模式”，降低普通用户的理解成本，并把实现、测试和文档一次性对齐到 2026-03-18 的代码状态。

## Problem Statement
- 当前扫描入口直接暴露 `profile / batch / snapshot interval`。
- `SSD / HDD / Network` 与 `batch / snapshot` 都偏向实现细节，不适合普通用户。
- 文档也在引导用户手动调参数，进一步放大了理解门槛。

## Optimization Strategy
1. 用三档用户模式替代技术参数：
   - `快速扫描（推荐）`
   - `深度扫描`
   - `超大硬盘模式`
2. 保留内部调优能力，但只在代码里维护映射，不再在 UI 暴露数值旋钮。
3. 明确说明：
   - 三种模式都会完整扫描当前范围
   - 差异只在扫描节奏、事件批次和界面刷新方式
4. 将模式定义集中到扫描层，避免 UI、测试、文档各自维护一套描述。

## Internal Mapping
| 用户模式 | 内部 profile | batch_size | snapshot_ms | metadata_parallelism | deep_tasks_throttle |
|---|---:|---:|---:|---:|---:|
| 快速扫描（推荐） | `Ssd` | `256` | `75` | `4` | `64` |
| 深度扫描 | `Hdd` | `192` | `60` | `6` | `96` |
| 超大硬盘模式 | `Network` | `640` | `150` | `3` | `192` |

说明：
- 用户只看到模式名称和场景说明。
- `profile / batch / snapshot` 继续作为扫描引擎内部实现细节存在。

## Execution Plan
- [x] 梳理代码影响面：扫描 UI、扫描配置、测试、README、使用文档、设计文档、工作记录。
- [x] 在 `dirotter-scan` 中引入统一的 `ScanMode` 预设模型。
- [x] 用 `ScanConfig::for_mode(...)` 统一生成内部扫描配置。
- [x] 将 UI 从 `SSD/HDD/Network + batch/snapshot` 改为三档模式选择。
- [x] 添加模式说明文案，明确“完整扫描不变，只调整节奏与刷新方式”。
- [x] 持久化用户所选扫描模式到本地设置。
- [x] 补充测试，验证模式映射与预设模式可完成扫描。
- [x] 更新 README、使用指南、快速上手、UI 规格、系统设计、综合评估、任务记录。

## Verification Plan
1. 代码层验证：
   - `ScanMode` 设置值可 round-trip
   - `ScanConfig::default()` 回到推荐模式
   - 三档模式在内部节奏上具备明显差异
2. 集成验证：
   - 三档模式都能完成 sample fixture 扫描
3. 工程验证：
   - 运行 `cargo fmt --all`
   - 运行 `cargo test --workspace`

## Verification Status
- `cargo fmt --all`：已通过
- `cargo test --workspace`：已通过

## Result
- 扫描入口已从技术参数面板收口为用户模式选择。
- 默认推荐路径更清晰，普通用户不再被迫理解 `batch / snapshot`。
- 文档、实现和测试已围绕同一套扫描模式定义收口。

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
  - 卡片容器增加统一外边距，避免描边被子布局裁剪矩形截断
