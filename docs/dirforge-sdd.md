# DirForge 系统设计说明书（SDD）

## 1. 目标与范围

DirForge 是一款 **Windows 优先** 的生产级 Rust 桌面磁盘分析器，目标是支持百万级节点扫描、扫描过程持续可交互、支持增量刷新、重复文件检测与安全清理。

技术基线：

- Rust
- `egui/eframe`（原生桌面 UI）
- `windows-rs`（Windows API 集成）
- `rayon`（CPU 并行）
- `walkdir`（目录遍历）

## 2. 总体架构

核心原则：解耦“发现文件、元数据采集、聚合、去重、缓存、UI 渲染”。

```text
Desktop UI (egui/eframe)
  -> UI State / ViewModel
  -> Event & Snapshot Bus
  -> { Scan Engine | Aggregate Engine | Dup Engine | Cache Engine }
  -> Storage + OS Integration
```

### 模块职责

- **Scan Engine**：只做遍历与轻元数据采集。
- **Aggregate Engine**：目录回卷、扩展名统计、Top-N。
- **Dup Engine**：独立去重流水线，不能阻塞基础扫描。
- **Cache Engine**：快照持久化、增量重扫、失效策略。
- **UI**：仅消费快照和增量事件，不直接操作扫描内部状态。

## 3. 线程模型

建议分四类执行域：

1. **UI Main Thread**：输入/渲染/命令分发。
2. **IO Scan Pool（有界）**：目录枚举、metadata 读取。
3. **CPU Pool（Rayon）**：聚合、排序、哈希、布局。
4. **Background Service**：缓存落盘、日志批处理、图标预取。

并发原则：

- IO 与 CPU 分池，避免互相污染。
- 线程间通过消息传递，不共享巨型可变状态。
- UI 不直接读取扫描期的大锁结构。

## 4. 扫描流水线

```text
Stage 0 Root Planning
Stage 1 Directory Enumeration
Stage 2 Metadata Acquisition
Stage 3 Local Aggregation
Stage 4 Tree Rollup
Stage 5 Snapshot Publish
Stage 6 Deferred Deep Tasks
```

设计要点：

- 事件批量化发出，避免逐文件刷 UI。
- 元数据阶段只采“轻元数据”，不读文件内容。
- 回卷以目录完成为单位批处理，不做每文件回根锁更新。
- 快照发布做节流（如 50~100ms coalescing）。
- 重哈希/缩略图/深探测放入延迟任务。

## 5. 缓存架构

三层缓存：

- **L1 内存热缓存**：可见树节点、可见 treemap tile。
- **L2 会话缓存**：当前扫描索引、扩展名分片统计。
- **L3 持久化缓存**：卷快照、文件身份、历史清单。

建议持久化 FileIdentity（非仅 path）：

- `volume_id`
- `file_id`
- `path_hash`
- `size`
- `mtime`
- `attrs`

失效策略：子树失效、文件失效、卷级失效、schema 失效分层处理。

## 6. 重复文件检测架构

四阶段去重：

1. `size -> files[]` 初筛
2. partial hash 重分桶
3. strong hash 最终确认
4. 结果整形（keeper 推荐、风险级别、可回收空间）

约束：

- 禁止全盘直接全量哈希。
- 去重队列独立，支持吞吐限流与前台交互降权。
- 输出面向 UX：重复组、风险、推荐保留项、删除模拟收益。

## 7. UI 刷新策略

UI 目标：扫描期间连续可视、连续可操作。

关键策略：

- 快照合并（coalescing）
- 局部失效（tree/table/treemap/details 独立）
- 大列表虚拟化
- treemap 脏子树增量更新
- 交互优先（拖拽/滚动时降低后台合并频率）

建议帧预算：

- idle：目标 60 FPS
- scanning：15~30 FPS 视觉刷新，优先输入延迟
- 重计算分帧切块

## 8. 关键数据结构

推荐扁平 NodeStore：

```text
NodeId -> Node { parent, children-range, kind, size_self, size_subtree, ... }
Vec<Node>
HashMap<PathKey, NodeId>
String Interner
```

优势：

- 更好的 cache locality
- 并行聚合与排序更友好
- 序列化/快照持久化简单

## 9. Windows 专项设计

- reparse point / junction / symlink 策略化处理，避免循环与重复计数。
- 权限失败纳入错误中心，不因单点失败中断全局。
- 使用 `windows-rs` 获取卷属性、稳定文件身份、回收站能力。

## 10. 里程碑建议

1. 高性能扫描基础（IO 池 + NodeStore + 增量快照）
2. 生产级 UI（虚拟化 + treemap 增量）
3. 持久缓存（增量重扫 + 崩溃恢复）
4. 重复文件系统（四阶段 pipeline）
5. Windows 深度集成（权限/卷/Shell）
