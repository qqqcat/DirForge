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
