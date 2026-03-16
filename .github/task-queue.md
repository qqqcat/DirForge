# Task Queue (Stateful)

## Status Legend
- `todo`: 待执行
- `in_progress`: 正在执行（同一时间仅 1 项）
- `blocked`: 被阻塞
- `done`: 完成且通过验证

## Priority Legend
- P0: 阻塞主目标
- P1: 核心能力
- P2: 增强能力
- P3: 文档与整理

## Active Goal
提升模板仓库的可执行性与可审计性，补齐文档到实际落地之间的缺口。

## Queue
- [done][P1] 增加模板结构自检脚本与最小 CI workflow
- [done][P1] 修正文档路径引用并补充 quickstart 指引
- [done][P2] 回写审计结果到 findings/task_plan/progress/memory
- [done][P1] 建立执行规划文件（task_plan/findings/progress）
- [done][P1] 构建 `.github` 核心控制文件（agent/loop/queue/context）
- [done][P1] 构建 `memory` 持久化结构
- [done][P1] 构建 `instructions` 执行规范
- [done][P2] 增加质量闸门与恢复机制文档
- [done][P2] 增加示例工作流与模板说明
- [done][P3] 完善 README 与使用指南
- [todo][P2] 在真实项目中接入测试/CI流水线并验证自治闭环

## Blockers
- 无
