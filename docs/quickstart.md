# Quick Start

## Goal
用最短路径把这个模板接入到一个真实项目或新的工作会话里，并保证状态可恢复、结构可校验。

## 5-Minute Setup
1. 复制本仓库后，先通读 `README.md` 和 `.github/copilot-agent.md`，确认执行边界。
2. 根据当前项目实际情况重写 `.github/project-context.md`，不要保留模板化空话。
3. 在 `.github/task-queue.md` 中只保留当前目标相关的任务，并确保最多只有 1 个 `in_progress`。
4. 用当前项目的事实更新 `.github/memory/goals.md`、`.github/memory/progress.md`、`.github/memory/decisions.md`。
5. 运行 `pwsh ./scripts/validate-template.ps1`，先确认模板结构和关键路径没有漂移。

## First Prompt Pattern
把下面这些文件一起交给 Agent 作为启动上下文：

- `.github/copilot-agent.md`
- `.github/devin-loop.md`
- `.github/project-context.md`
- `.github/task-queue.md`
- `.github/memory/progress.md`

可直接使用的启动语句：

```text
先读取上述文件，识别当前 in_progress / blocked 项，给出本轮实施计划，然后按实现 -> 验证 -> 修复 -> 记录的循环继续推进。
```

## End-Of-Round Checklist
- 更新 `.github/task-queue.md` 中的任务状态。
- 追加 `.github/memory/progress.md` 的结果和下一步。
- 记录需要长期保留的决策到 `.github/memory/decisions.md`。
- 如有失败案例，写入 `.github/memory/bugs.md` 或 `.github/memory/lessons.md`。
- 再运行一次 `pwsh ./scripts/validate-template.ps1`，避免路径和结构被改坏。

## When To Extend This Template
- 项目已经有真实代码，需要把 `.github/workflows/template-validation.yml` 扩展为实际测试流水线。
- 项目存在多 Agent / 多角色协作，需要补充 skill registry、adapter registry 或运行手册。
- 你需要可执行脚本，而不仅是文档流程时，应在真实项目中补充 `src/`、`tests/` 和 CI。
