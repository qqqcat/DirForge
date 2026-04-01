# DirOtter 系统设计说明书（SDD，2026-03-20）

## 1. 目标与范围

DirOtter 是一款本地磁盘分析器工程化原型，目标是稳定“扫描 -> 聚合 -> 展示 -> 去重候选 -> 操作计划 -> 报告/诊断”主链路，并持续补齐平台能力、可观测性与执行安全。

## 2. 当前实现状态

- 已实现：扫描事件流（`Progress/Batch/Snapshot/Finished`）、取消扫描、错误分类上报。
- 已实现：`NodeStore` 扁平结构、`rollup` 聚合、Top-N 查询。
- 已实现：`eframe/egui` UI、历史快照缓存、页面级滚动、对称内容 gutter、轻量结果视图与列表展示、19 语言选择（其中中文/英文/法语/西班牙语为完整文案，其余新增语言当前回退英文 UI）。
- 已实现：默认根路径选择、盘符快捷扫描、三档扫描模式预设、Inspector 内真实删除（回收站/永久删除确认）、后台删除任务提示、目录上下文文件榜单、文本/CSV/诊断导出。
- 已实现：系统快照与指标描述、路径可达性评估、诊断归档。

> 当前成熟度：**Production Readiness**

## 3. 总体架构

```text
Desktop UI (egui/eframe)
  -> UI State / ViewModel
  -> Event Bus (bounded)
  -> { Scan Engine | Aggregate Engine | Dup Engine | Cache Engine }
  -> Storage + OS Integration
```

### 模块职责

- **Scan Engine**：目录遍历与轻元数据采集，产出进度/批次/快照/完成事件。
- **Scan Preset Layer**：将 `快速扫描 / 深度扫描 / 超大硬盘模式` 映射到内部 `profile + batch + snapshot + 并发阈值`。
- **Aggregate Engine**：接收 walker 事件并构建目录树，处理乱序父子到达。
- **Cleanup Suggestion Layer**：基于扫描完成后的 `NodeStore` 做规则分类、风险判断、评分与候选收口，优先生成可执行结果而不是完整命中清单。
- **Fast Cleanup Lane**：安全缓存项走 `staging -> background purge`，优先让用户立刻摆脱“卡住等待删除”的感知。
- **Dup Engine**：独立去重流水线，避免阻塞基础扫描。
- **Cache Engine**：设置、审计与按需快照/历史管理。
- **Product Refocus Direction**：默认用户路径优先“扫描 -> 清理建议 -> 执行删除 -> 释放空间确认”；快照/历史/错误导出已转为按需能力，而非默认完成态成本。
- **UI**：消费事件流、支持盘符快捷扫描、执行删除后的局部重建，并进行可视化。
- **Cleanup UX**：在 Overview Hero 中优先展示“可释放空间”、建议详情入口和安全缓存一键清理入口。
- **Result View**：只在扫描完成后消费缓存/结果树，按当前目录直接子项做轻量下钻，不参与实时布局。
- **Result View Layout**：采用固定头部 + 填充型结果区，列表区域吃满剩余高度并在内部滚动。
- **Action UX**：将长耗时删除从 UI 主线程中剥离，并通过“确认窗关闭后转后台任务”的方式，在顶部横幅、Inspector 与状态栏持续提示执行状态。
- **Fast Delete Path**：Windows 文件永久删除优先使用低层句柄删除，失败后回退到现有文件系统删除。
- **Maintenance Utilities**：普通用户的系统内存释放入口固定放在右侧 `Quick Actions` 的 `一键释放系统内存`；技术性维护下沉到 Diagnostics，包括 `优化 DirOtter 内存占用` 与“清理异常中断的临时删除区”。后者仍是应用级维护，会优先把结果树写入快照，再释放应用内存并在需要时按需回载。

### 当前页面布局策略

- Shell 层使用固定壳体：顶部工具栏、左导航、右 Inspector、底部状态栏
- 中央内容区使用最大宽度约束与对称 gutter
- 主要内容页使用外层纵向滚动，不通过固定高度主卡或固定高度榜单裁切内容
- 页面内部采用显式两列或单列组合，而不是依赖自然高度列布局碰运气对齐
- Overview 采用独立首页宽度与显式对称 gutter
- Overview 采用 `Hero 结论区 -> KPI 指标条 -> 全宽扫描卡 -> 双列证据区`
- Overview 不再保留独立 `卷空间摘要` 大卡；卷级信息并入 KPI 与扫描卡，避免重复信息
- Overview 的双列证据区使用显式列宽分配，避免在 Inspector 存在时出现漂移、重叠和左右留白失衡
- Settings 采用窄内容列的分组设置流，不与 Overview 复用同一套版式语义

## 4. 当前线程模型

1. **UI 主线程**：输入、渲染、命令分发、事件消费合并。
2. **扫描 worker 池**：并发枚举目录与读取 metadata。
3. **聚合线程**：接收 walker 事件并维护树结构聚合状态。
4. **发布链路**：通过有界通道向 UI 发送批次/快照事件，控制背压。
5. **取消收尾**：worker 在等待目录任务时按短超时轮询取消标记，避免 `Stop Scan` 后挂在条件变量上。

## 5. 扫描流水线

```text
Stage 0 Root Planning
Stage 1 Concurrent Directory Enumeration
Stage 2 Metadata Acquisition
Stage 3 Aggregation / Parent-Child Reconciliation
Stage 4 Rollup & Top-N Extraction
Stage 5 Snapshot Delta Publish
Stage 6 Finished Publish
```

### 当前扫描模式映射

- `快速扫描（推荐）`：默认模式，优先更快给出可操作结果。
- `深度扫描`：对复杂目录树采用更稳的节奏，强调首次排查体验。
- `超大硬盘模式`：降低界面刷新压力，优先保证超大目录和超大容量磁盘的稳定扫描。

说明：

- 三档模式都会完整扫描当前范围。
- `profile / batch / snapshot` 仍然存在，但已退回为扫描层内部实现细节。

## 6. 缓存架构

- **L1 内存热缓存**：当前可见列表/treemap 数据。
- **L2 会话缓存**：当前扫描状态与待处理事件。
- **L3 持久化缓存**：SQLite 设置、审计与按需历史快照。
- **Rust 结构优化**：常驻 `NodeStore` 中的 `Node` 只保留 `name_id / path_id`，完整字符串统一驻留在字符串池；扫描中的增量快照使用 resolved 视图，避免把字符串重复常驻在每个节点上。
- 快照策略：同一路径只保留最新一份 `NodeStore` 快照，避免重复扫描同一路径时 SQLite 体积线性增长；默认不在每次扫描结束后自动写入。
- WAL 管理：快照写入后主动执行 checkpoint，及时收缩 `dirotter.db-wal`。
- Staging 清理：应用启动时会扫描并继续清理遗留的 `.dirotter-staging` 项，避免上次异常退出后残留缓存占用。

## 7. 重复文件检测架构

四阶段去重：

1. `size -> files[]` 初筛
2. partial hash 重分桶
3. strong hash 最终确认
4. 结果整形

## 8. UI 刷新策略

- 快照合并（coalescing）
- 有界队列（`VecDeque`）+ 超限受控丢弃
- 大列表虚拟化
- 页面级滚动 + 对称 gutter
- 清理建议只在扫描完成态或缓存快照可用时分析，不与实时扫描刷新绑定
- 清理建议候选必须受控：
  - 每类候选 Top-N
  - 全局总候选上限
  - 优先缓存、下载、大文件等可执行项
- 结果视图与实时扫描解耦，只读取扫描完成后的树结果
- 结果视图主列表改为“剩余高度填充 + 内部滚动”，避免内容区塌缩
- 删除成功后的局部 `NodeStore` 重建与重新 `rollup`
- 选中文件夹后的上下文文件榜单切换

## 9. 清理建议分析层（V1）

- 规则分类：
  - `cache`
  - `downloads`
  - `video`
  - `archive`
  - `installer`
  - `image`
  - `system`
  - `other`
- 风险规则：
  - 系统路径 => `High`
  - 一般 `AppData` => `Medium`
  - 明确命中缓存路径 => `Low`
- 评分规则：
  - `score = size_gb * 0.7 + unused_days * 0.3`
  - `cache` 额外加分
  - `installer` 轻微加分
  - `system` 直接负分并阻断执行
- 候选限制：
  - `Low / Medium / High` 不同风险使用不同候选上限
  - 全局结果集再次截断，避免详情窗和完成态承载过多无效候选
- V1 快捷动作：
  - Overview 卡片显示可释放空间
  - 详情窗允许勾选安全/谨慎项
  - `一键清理缓存（推荐）` 仅执行安全缓存项，并走 `staging -> 后台 purge` 两阶段极速链路
## 10. 平台能力现状

- Explorer 打开/选择
- 回收站删除入口
- Windows 回收站可见性二次校验
- 永久删除执行入口
- 卷信息查询
- 路径前置校验与统一平台错误模型

## 11. 后续里程碑建议

1. 执行安全：真实删除路径在跨平台边界场景的覆盖加深。
2. 稳定性：长跑压测 + 资源曲线观测。
3. 可观测性：关键链路指标与诊断导出进一步标准化。
4. UI 体验：统一正式栅格、视觉回归和错误恢复流程。
