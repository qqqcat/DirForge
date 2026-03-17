# Task Plan

## Goal
对 DirOtter 当前代码、交互和文档进行一轮基于实测结果的整体评估，并把所有关键文档同步到与 2026-03-17 代码状态一致。

## Assessment Scope
- [x] 验证工作区构建与测试状态（`cargo check --workspace`、`cargo test --workspace`）
- [x] 审视扫描主链路：并发扫描、聚合、完成态、取消与错误处理
- [x] 审视 UI 主流程：Overview / Live Scan / Treemap / History / Errors / Diagnostics / Settings
- [x] 审视删除链路：Inspector 内回收站删除、永久删除、审计、后台任务与局部刷新
- [x] 审视页面级布局系统：内容宽度、gutter、滚动策略、固定高度移除
- [x] 更新主文档：`README.md`
- [x] 更新综合评估：`docs/dirotter-comprehensive-assessment.md`
- [x] 更新设计与使用文档：`docs/dirotter-sdd.md`、`docs/dirotter-ui-component-spec.md`、`docs/dirotter-install-usage.md`、`docs/quickstart.md`
- [x] 更新工作记录：`findings.md`、`progress.md`

## Current Assessment Summary
- 当前阶段：`Production Readiness`，主链路已稳定可运行。
- 当前优势：扫描链路稳定、删除流程已进入 Inspector、删除后主视图支持局部刷新、默认根路径和盘符快捷扫描已落地。
- 当前新增布局能力：标题状态胶囊已改为内部状态枚举并实时本地化；主要内容页已切到页面级纵向滚动、对称 gutter 和统一最大内容宽度。
- 当前新增交互能力：选中文件夹后可查看目录上下文下的最大文件；删除确认为后台任务模式，确认窗提交后立即关闭。
- 当前主要风险：UI 仍缺一个覆盖 Overview / Live Scan / Treemap 的正式统一栅格体系；删除中的反馈仍偏粗粒度；视觉回归仍主要靠人工截图检查。

## Completed In This Round
1. 已将状态胶囊从硬编码字符串改为内部 `AppStatus` 枚举，解决多语言状态显示问题。
2. 已将 `Overview / Live Scan / History / Errors / Diagnostics / Settings` 切换为页面级纵向滚动。
3. 已移除首页和实时扫描页中依赖固定高度的主卡与榜单滚动盒，改为自然高度流式布局。
4. 已为主内容区引入统一最大宽度和对称 gutter 计算。
5. 已完成 2026-03-17 全量复验：`cargo check --workspace`、`cargo test --workspace` 均通过。
6. 已同步更新 README、综合评估、系统设计、UI 规格、安装指南、快速上手与工作记录。

## Next Priorities
1. 为 `Overview / Live Scan / Treemap` 建立统一的正式栅格体系，而不是按页单独微调。
2. 引入最小视觉回归或截图对比，保护留白、对齐和列表高度不再反复回归。
3. 继续增强删除中的用户反馈，评估阶段性进度、Explorer 回收站联动提示或可取消语义。
4. 持续扩展真实删除跨平台边界样本，尤其是权限、锁定、系统目录和回收站可见性。

