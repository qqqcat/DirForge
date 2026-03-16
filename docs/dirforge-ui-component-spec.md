# DirForge UI 高保真组件规格（v2）

## 1. 设计基线

## 当前实现映射

- 已提供页面：Dashboard / Current Scan / Treemap / History / Errors / Operations / Diagnostics / Settings。
- 已实现大列表虚拟化（show_rows）与基础帧预算指标展示。
- 已实现多语言默认策略（系统语言）与设置持久化。
- 已落地顶部工具栏、左导航、中央工作区、右侧 Inspector、底部状态栏。
- 已落地人类可读格式化：`format_bytes` / `format_count`。
- 已落地系统中文字体回退与 treemap 标签阈值控制。
- 已区分“扫描到的文件体积（Scanned Size）”与“卷已用空间（Volume Used）”，避免将扫描结果误读为磁盘总容量。
- 已将内部性能指标从主视图下沉到 Diagnostics，主视图以“卷空间、最大文件夹、最大文件、treemap”为中心。

产品定位：专业、克制、高信息密度、可长时使用。

默认策略：

- 默认密度：`Compact`
- 默认主题：`Dark`（支持 Light）
- Inspector 固定宽度：`320px`
- 标签文本策略：超过可读阈值才显示，长文本中间截断。

## 2. 布局与尺寸

- 顶部工具栏：44px
- 二级工具栏：36px
- 左导航：220px
- 右 Inspector：320px
- 底部结果面板：240px
- 常规行高：32px（紧凑 28px）

字体层级：

- 页面标题：20 / Semibold
- 区块标题：16 / Semibold
- 正文：13 / Regular
- 表格：12~13 / Regular
- 注释：11 / Regular

## 3. 组件清单

### 3.1 按钮

- Primary：Start / Confirm / Execute
- Secondary：Refresh / Export
- Ghost：Reveal / Copy Path
- Destructive：Permanent Delete / Clear Queue
- SplitButton：Delete▼ / Export▼
- IconButton：轻量工具动作（必须有 tooltip）

### 3.2 输入与选择

- SearchInput（带清空）
- TextInput（支持错误态）
- NumberInput（带最小/最大值）
- Select（超过 10 项建议可搜索）

### 3.3 信息组件

- Tabs（主 Tab + 次级 Tab）
- Badge（风险/状态）
- Breadcrumb（路径导航）
- Dialog Scaffold（统一骨架）
- Inline Banner / Toast

## 4. 表格规范

通用行为：列排序、列宽调整、列显隐、虚拟化滚动、多选、右键菜单。

### 4.1 文件表默认列

`Name | Path | Size | Subtree Size | Type | Extension | Modified | Risk | Status`

### 4.2 重复组表默认列

`Group ID | Count | Total Size | Reclaimable | Type | Risk | Keeper | Action Status`

### 4.3 错误表默认列

`Type | Path | Stage | Count | Retryable | Last Seen`

### 4.4 扩展名表默认列

`Extension | File Count | Total Size | Avg Size | Largest File | Largest Location`

## 5. 状态标签与语义

### 5.1 风险标签

- `Low`
- `Medium`
- `High`
- `Protected`

### 5.2 扫描状态

- `Idle`
- `Preparing`
- `Scanning`
- `Paused`
- `Completed`
- `Partial`
- `Cancelled`
- `Error`

### 5.4 本地化与字体

- 默认依据系统语言环境推断语言。
- 设置页手动选择优先级高于自动检测。
- Windows 优先加载 `Microsoft YaHei / DengXian / SimHei / SimSun` 作为 CJK fallback。
- 中文 UI 禁止出现 `□` 占位符或 ASCII 拼接式半成品文案。

### 5.3 操作状态

- `Pending`
- `Running`
- `Completed`
- `Partial Success`
- `Failed`

## 6. 颜色语义

建议以 Token 层表达，不在组件里写死色值：

- 中性层：`bg/panel`, `border/default`, `text/primary` 等
- 功能层：`accent`, `success`, `warning`, `danger`, `info`

Treemap 颜色模式：

1. 按 Category（默认）
2. 按 Extension
3. 按 Risk

Treemap 标签规则：

1. 当 tile 宽高低于最小阈值时不渲染标签。
2. 中等 tile 仅显示截断名称。
3. 较大 tile 显示名称 + 人类可读大小。
4. 完整路径统一通过 hover 提示展示，不强行塞入 tile 本体。

## 7. 图标语义

分类：

- 对象图标（Drive/Folder/File/Archive/Video 等）
- 动作图标（Scan/Pause/Delete/Move/Export）
- 状态图标（Success/Warning/Error/Protected）

规则：危险动作图标不可替代文本，需 `icon + label` 或明确 tooltip。

## 8. 关键交互规则

- 全局单一选中源：Tree / Treemap / Table 三视图联动。
- Inspector 只显示当前全局选中对象。
- 危险动作统一入操作中心（队列化执行）。
- 每个面板都必须定义 Empty / Loading / Error 三态。

## 9. 页面装配（核心）

Current Scan 页面必须包含：

- 顶部 Toolbar
- 左侧 Navigation
- 中央工作区（卷空间摘要 + 最大文件夹 + 最大文件 + 最近发现）
- 右侧 Inspector
- 底部 Status Bar
- 状态栏

## 新增交互要求（本轮）

- Errors 页面需展示错误分类统计（User/Transient/System）。
- Operations 页面需展示批执行结果列表（success/failure/message）。
- Diagnostics 页面提供诊断包导出入口。
- 所有大小与计数信息必须使用人类可读格式展示。
- 概览页必须显式区分 `Scanned Size`、`Volume Used`、`Total Capacity` 三类不同语义。
- 默认主视图不展示 `frame / queue depth / batch size / snapshot commit` 这类内部性能数字。
- Settings 页面语言选择改为明确单选，不使用“中文复选框”这种含糊交互。
- Dashboard / Current Scan / History 页面需避免把 4 个以上指标挤在单行纯文本中。
- 扫描过程中的提示 Banner 必须放在页面内容区，不能塞进固定高度工具栏，否则会制造大面积无意义占位。
- 桌面 UI 在存在后台扫描句柄时必须持续请求重绘，确保 `Progress / Snapshot / Finished` 事件会被及时消费，而不是停留在假死式 `Scanning` 状态。

## 10. 设计 Token 建议结构

```text
color/
radius/
space/
font/
size/
```

用于支持浅/深主题切换与组件复用。
