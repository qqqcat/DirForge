# DirForge UI 组件规格（2026-03-16）

## 1. 当前实现映射

- 已提供页面：Overview / Live Scan / Treemap / History / Errors / Diagnostics / Settings
- 已移除页面：Operations
- 已落地布局：顶部工具栏、左导航、中央工作区、右侧 Inspector、底部状态栏
- 已落地能力：中文字体回退、人类可读大小/计数格式、treemap 标签阈值、独立滚动排行榜
- 已落地动作：Inspector 内 `Move to Recycle Bin` / `Delete Permanently`
- 已落地刷新：删除成功后局部刷新榜单、概览统计、treemap 与 Inspector
- 已落地引导：默认根路径优先系统盘/首个卷，并提供盘符快捷扫描按钮

产品方向：专业、克制、高信息密度，但必须优先服务“找出最大占用并直接处理”的主目标。

## 2. 布局与尺寸

- 顶部工具栏：44px
- 左导航：188px
- 右 Inspector：300px
- 底部状态栏：26px
- 常规行高：28~32px

## 3. 页面职责

### Overview
- 负责卷空间摘要、最大文件夹、最大文件、扫描目标与盘符快捷扫描
- 必须显式区分：
  - `Scanned Size`
  - `Volume Used`
  - `Total Capacity`

### Live Scan
- 展示扫描中已发现的最大项与最近扫描到的文件
- 必须清楚表达“这是实时增量视图，不是最终结论”

### Treemap
- 展示大目录空间分布
- 标签只在可读区域显示，完整路径走 hover

### History
- 展示历史扫描记录与快照摘要

### Errors
- 展示错误分类与路径
- 从错误列表选中对象后，应直接在 Inspector 查看和处理

### Diagnostics
- 展示诊断 JSON 与导出入口

### Settings
- 展示语言、主题和本地化说明

## 4. 工具栏规范

- 空闲状态：
  - 主按钮：`Start Scan`
  - 次按钮：`Cancel` 可禁用
- 扫描状态：
  - 主按钮必须禁用或隐藏 `Start Scan`
  - 次按钮必须切换为 `Stop Scan`
- 不允许在扫描进行中继续显示可点击的 `Start Scan`

## 5. Inspector 规范

- Inspector 始终展示当前全局选中对象
- 选中对象来源可以是：
  - 排行榜
  - Live Scan 最近文件
  - Treemap
  - History
  - Errors
- Inspector 必须包含：
  - 名称/路径/大小
  - 来源上下文
  - 快速操作区

### Quick Actions

- `Move to Recycle Bin`
- `Delete Permanently`

要求：

- 危险动作不再进入独立页面
- 永久删除必须先经过确认层
- 删除成功后，当前选中对象必须从 Inspector 清除
- 删除成功后，相关榜单、概览统计和 treemap 必须即时局部刷新
- 回收站删除成功后应提示可撤销
- 删除失败后应给出针对性的失败提示

## 6. 排行榜与滚动区规范

- 最大文件夹、最大文件必须使用独立滚动区
- 每个滚动区必须拥有独立 widget/scroll ID，避免调试模式下重复 ID 红框
- 列表项点击后应联动 Inspector
- 滚动区内容允许中间截断路径，但必须保留大小与排序信息

## 7. Treemap 标签规范

1. tile 过小则不显示标签
2. 中等 tile 仅显示截断名称
3. 较大 tile 显示名称 + 人类可读大小
4. 完整路径统一通过 hover 展示

## 8. 扫描目标与首次引导

- 默认根路径应优先使用系统盘或首个卷挂载点，不得默认停留在 `.`
- 应提供盘符/卷快捷按钮，点击后直接开始扫描对应卷
- 文本输入框保留为高级入口，用于扫描任意目录
- 空状态提示需明确说明“优先点盘符按钮，也可手动输入目录”

## 9. 本地化与主题

- 默认依据系统语言环境推断语言
- 设置页手动选择优先级高于自动检测
- Windows 优先加载 `Microsoft YaHei / DengXian / SimHei / SimSun`
- 浅色与深色主题都必须保持完整 panel/surface 对比度，不能出现浅色控件叠黑底

## 10. 仍待补强的交互

- 更强的高风险永久删除警示表现
- 更丰富的排序/筛选
- UI 自动化交互回归
