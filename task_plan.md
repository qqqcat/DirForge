# Task Plan

## Goal
对 DirOtter 当前代码、交互和文档进行一次基于实测结果的项目评估，并把所有关键文档同步到与代码一致的状态。

## Assessment Scope
- [x] 验证工作区构建与测试状态（`cargo check --workspace`、`cargo test --workspace`）
- [x] 审视扫描主链路：并发扫描、聚合、完成态、取消与错误处理
- [x] 审视 UI 主流程：Overview / Live Scan / Treemap / History / Errors / Diagnostics / Settings
- [x] 审视删除链路：Inspector 内回收站删除、永久删除、审计与局部刷新
- [x] 识别文档漂移：过时的 `Operations` 页面、旧删除流程、旧导航描述
- [x] 更新主文档：`README.md`
- [x] 更新综合评估：`docs/dirotter-comprehensive-assessment.md`
- [x] 更新设计与使用文档：`docs/dirotter-sdd.md`、`docs/dirotter-ui-component-spec.md`、`docs/dirotter-install-usage.md`、`docs/quickstart.md`
- [x] 更新工作记录：`findings.md`、`progress.md`

## Current Assessment Summary
- 当前阶段：`Production Readiness`，已具备稳定的端到端可运行主链路。
- 当前优势：扫描链路稳定、UI 可用性明显提升、删除动作已进入 Inspector、删除后主视图支持局部刷新、启动即提供盘符快捷扫描。
- 当前新增能力：删除执行改为后台线程，确认窗会先关闭，再转入顶部横幅/Inspector 后台任务提示；Windows 回收站删除增加系统回收站二次校验；选中文件夹后可查看该范围内的最大文件。
- 当前主要风险：真实删除的用户感知仍偏粗粒度，后台任务目前只有阶段性提示而没有字节级进度；缺少更完整的 UI 集成回归与回收站实机验证。

## Completed In This Round
1. 已为永久删除增加确认层，并补充更清晰的失败/撤销提示。
2. 已为删除后的局部刷新补充 UI 侧自动化回归验证（默认根路径与局部树重建测试）。
3. 已加强真实删除在权限不足、文件占用、目录删除等场景下的测试矩阵。
4. 已优化首次启动默认根路径与空状态引导，并新增盘符快捷按钮，点击即可直接扫描对应卷。
5. 已把真实删除改为后台执行，并改成“确认窗立即关闭 + 后台任务持续显示”的非阻塞方案，避免删除大文件时 UI 线程同步阻塞。
6. 已为 Windows 回收站删除增加“是否进入系统回收站”的二次校验，避免仅以 API 返回值判定成功。
7. 已将“最大文件”榜单改为支持选中文件夹上下文，优先展示该目录内部的最大文件。
8. 已移除窗口标题、导航和提示横幅中的 `relay-1` 调试标记，恢复正式产品展示。

## Next Priorities
1. 继续增强删除中的用户反馈，评估是否能提供阶段性进度、Explorer 回收站联动提示或可取消语义。
2. 引入更接近真实点击流的 UI 集成测试，而不仅是 UI 逻辑单测。
3. 持续扩展真实删除跨平台边界样本，尤其是权限、锁定、系统目录和回收站可见性。
4. 继续优化盘符按钮、文件夹上下文分析和自定义目录输入的视觉层级，减少首次使用成本。
