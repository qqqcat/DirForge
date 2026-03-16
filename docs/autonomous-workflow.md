# Autonomous Workflow Example

## Input Goal
实现一个可部署的后端 API 服务。

## How This Template Runs
1. 在 `.github/task-queue.md` 写入目标与任务
2. 按 `.github/devin-loop.md` 执行循环
3. 按 `.github/instructions/quality-gate.md` 检查是否可完成
4. 将结果写入 `.github/memory/*`
5. 运行 `scripts/validate-template.ps1`，确认模板结构和关键入口仍然有效

## Iteration Example
- Round 1: 架构与目录初始化
- Round 2: 核心功能实现
- Round 3: 测试与错误修复
- Round 4: 重构与文档
- Round 5: 提交与PR说明草案

## Recovery Example
会话中断后，按 `.github/instructions/resume.md` + `.github/checklists/recovery.md` 继续。
