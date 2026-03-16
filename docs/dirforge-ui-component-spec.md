# DirForge UI 高保真组件规格（v1）

## 1. 设计基线

## 当前实现映射

- 已提供页面：Dashboard / Current Scan / Treemap / History / Errors / Operations / Diagnostics / Settings。
- 已实现大列表虚拟化（show_rows）与基础帧预算指标展示。
- 已实现多语言默认策略（系统语言）与设置持久化。

产品定位：专业、克制、高信息密度、可长时使用。

默认策略：

- 默认密度：`Compact`
- 默认主题：`Dark`（支持 Light）
- Inspector 固定宽度：`320px`

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
- 左侧 Directory Tree
- 中央工作区（Treemap/Files/Duplicates/Extensions/Timeline）
- 右侧 Inspector
- 底部 Results Panel
- 状态栏

## 10. 设计 Token 建议结构

```text
color/
radius/
space/
font/
size/
```

用于支持浅/深主题切换与组件复用。
