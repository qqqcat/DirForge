# Project Context

## Project Type
Copilot Autonomous Engineer 模板仓库（用于提升 VS Code 中 Copilot Agent 的持续执行能力）。

## Primary Objective
将一次性响应式助手行为，提升为“目标驱动 + 循环推进 + 可恢复”的自治执行模式。

## Non-goals
- 不尝试绕过平台安全策略。
- 不追求无边界自动化（保持可控、可审计）。

## Quality Bar
- 每个关键任务需有可验证结果。
- 失败必须记录并分类。
- 重大决策必须写入 `memory/decisions.md`。

## Typical Workflow
用户给出目标 → 队列拆分 → 循环执行 → 验证修复 → 持久化记忆 → 输出结果。
