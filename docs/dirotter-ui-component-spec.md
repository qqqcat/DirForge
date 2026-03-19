# DirOtter UI 组件规格（2026-03-18）

## 1. 当前实现映射

- 已提供页面：Overview / Live Scan / Treemap / History / Errors / Diagnostics / Settings
- 已移除页面：Operations
- 已落地布局：顶部工具栏、左导航、中央工作区、右侧 Inspector、底部状态栏
- 已落地能力：中文字体回退、人类可读大小/计数格式、treemap 标签阈值、页面级滚动、对称 gutter
- 已落地动作：Inspector 内 `Move to Recycle Bin` / `Delete Permanently` / `Open File Location`
- 已落地刷新：删除成功后局部刷新榜单、概览统计、treemap 与 Inspector
- 已落地反馈：删除确认窗口会先关闭，再转为顶部横幅、Inspector 任务卡片与状态栏提示
- 已落地引导：默认根路径优先系统盘/首个卷，并提供盘符快捷扫描按钮
- 已落地扫描体验：扫描入口已切换为三档用户模式，不再暴露 `batch / snapshot` 数值控件
- 已落地上下文：选中文件夹后，“最大文件”榜单会切换到该目录范围

产品方向：专业、克制、高信息密度，但必须优先服务“找出最大占用并直接处理”的主目标。

## 2. 布局与尺寸

- 顶部工具栏：56px
- 左导航：188px
- 右 Inspector：300px
- 底部状态栏：26px
- 常规行高：28~32px

### 页面内容区

- 主内容区使用统一最大宽度约束
- 主内容区左右两侧使用显式对称 gutter
- `Overview / Live Scan / History / Errors / Diagnostics / Settings` 使用页面级纵向滚动
- 不允许通过固定高度主卡或固定高度榜单来“强行对齐”页面

## 3. 页面职责

### Overview

- 负责卷空间摘要、最大文件夹、最大文件、扫描目标与盘符快捷扫描
- 第一排主区应以显式两列布局组织“扫描目标”和“卷空间摘要”
- 扫描目标卡必须优先展示用户模式，而不是技术参数
- 必须显式区分：
  - `Scanned Size`
  - `Volume Used`
  - `Total Capacity`

### Live Scan

- 展示扫描中已发现的最大项与最近扫描到的文件
- 必须清楚表达“这是实时增量视图，不是最终结论”
- 选中文件夹后，“最大文件”榜单应优先展示该目录内部的最大文件，而非始终显示整盘结果
- 排行榜不应只露出一小段可操作区；页面滚动应优先于卡片内部固定高度裁切

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
- 顶部标题旁的状态胶囊必须使用当前语言实时本地化文本

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

- `Open File Location`
- `Move to Recycle Bin`
- `Delete Permanently`

要求：

- 危险动作不再进入独立页面
- 永久删除必须先经过确认层
- 点击确认删除后，确认窗口必须立即关闭，再以后台任务形式给出可感知反馈
- 删除成功后，当前选中对象必须从 Inspector 清除
- 删除成功后，相关榜单、概览统计和 treemap 必须即时局部刷新
- 回收站删除成功后应提示可撤销
- Windows 回收站删除不应只依赖 API 返回值，需有系统回收站可见性的二次确认
- 删除失败后应给出针对性的失败提示

## 6. 排行榜与滚动区规范

- Overview / Live Scan 顶部 Top-N 卡片不得再嵌套固定高度 mini-scroll
- 榜单当前条目应直接进入页面内容流，由外层页面滚动承担浏览
- 列表项点击后应联动 Inspector
- 路径允许中间截断，但必须保留大小与排序信息
- 如果存在独立滚动区，必须拥有独立 widget/scroll ID，避免调试模式下重复 ID 红框

## 7. Treemap 标签规范

1. tile 过小则不显示标签
2. 中等 tile 仅显示截断名称
3. 较大 tile 显示名称 + 人类可读大小
4. 完整路径统一通过 hover 展示

## 8. 扫描目标与首次引导

- 默认根路径应优先使用系统盘或首个卷挂载点，不得默认停留在 `.`
- 应提供盘符/卷快捷按钮，点击后直接开始扫描对应卷
- 文本输入框保留为高级入口，用于扫描任意目录
- 扫描参数不再直接暴露给普通用户，必须改为模式化选择：
  - `快速扫描（推荐）`
  - `深度扫描`
  - `超大硬盘模式`
- 必须明确说明三档模式都会完整扫描当前范围，只是扫描节奏与界面刷新方式不同
- 空状态提示需明确说明“优先点盘符按钮，也可手动输入目录”

## 9. 本地化与主题

- 默认依据系统语言环境推断语言
- 设置页手动选择优先级高于自动检测
- Windows UI 字体优先采用 `Segoe UI / Segoe UI Variable` 风格，中文继续回退到 `Microsoft YaHei / DengXian / SimHei / SimSun`
- 浅色与深色主题都必须保持完整 panel/surface 对比度，不能出现浅色控件叠黑底

### DirOtter 色彩语义

- 主品牌色：`River Teal`
  - `#2F7F86`
  - Hover：`#276D73`
  - Active：`#1F5C61`
- 深色基础色：`Deep Slate`
  - App background：`#11181C`
  - Panel background：`#182227`
  - Elevated panel：`#1F2C32`
  - Text primary：`#EAF2F4`
  - Text secondary：`#A8BBC1`
  - Accent：`#4BA3AC`
- 浅色基础色：
  - Window background：偏灰白，避免纯白刺眼
  - Surface / divider 基础：`Mist Gray` `#E8EEF0`
  - 主文本：`Deep Slate` `#1E2A30`
- 轻暖辅助色：`Sand Accent` `#D8C6A5`
  - 仅用于空状态、少量品牌点缀或说明性元素，不得大面积覆盖分析界面

## 10. 仍待补强的交互

- 更正式的 12-column 页面栅格
- 更强的视觉回归保护
- 删除中的更细粒度阶段或进度表达
- `Overview / Live Scan / Treemap` 更统一的版式节奏
