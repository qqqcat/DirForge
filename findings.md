# Findings

## Source Analyzed
- `ChatGPT-Super Power 特点解析 (1).md`

## Extracted Baseline
- 需要四大核心：Agent Core / Task Queue / Memory / Loop。
- 目标行为：从一次性交互转为“目标驱动、循环完成”。
- 最低目录结构：`.github` + `memory` + `instructions` + `.vscode/settings.json`。

## Gaps To Improve Beyond Baseline
- 文档中的任务队列较静态，缺少状态字段与优先级。
- 缺少“质量闸门”与失败重试策略，容易反复修同类问题。
- 缺少“恢复机制”，会话中断后不易继续。
- 缺少清晰的 Definition of Done（完成标准）。

## Enhancement Strategy
- 引入状态机式任务队列（todo/in_progress/blocked/done）。
- 引入循环协议（计划→实现→验证→修复→总结→续跑）。
- 增加 memory 分类（goals/decisions/progress/bugs/lessons）。
- 增加检查清单与恢复说明。

## 2026-03-09 Repository Audit

### Key Findings
- 文档存在路径引用不一致的问题，尤其是 `task-queue.md`、`memory/progress.md` 和 `quality-gate.md` 的使用入口，有些地方省略了 `.github/` 前缀。
- README 和工程约束强调 CI / test / 可验证，但仓库本身缺少可直接运行的最小校验入口，导致“原则存在、执行缺位”。
- 新用户第一次接入时缺少一份专门的 quickstart，容易不知道应该先清空哪些模板内容、保留哪些状态文件。

### Changes Applied
- 新增 `scripts/validate-template.ps1`，自动检查模板关键文件、任务队列状态格式和文档入口。
- 新增 `.github/workflows/template-validation.yml`，为仓库提供最小 CI 校验闭环。
- 新增 `docs/quickstart.md`，补齐首次接入与每轮收尾动作。
- 修正 README 与 `docs/autonomous-workflow.md` 的关键路径引用，统一到实际仓库结构。
