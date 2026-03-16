# Quality Gate

任务标记为 `done` 前必须通过以下门禁：

1. Correctness Gate
- 需求已覆盖
- 关键边界已处理

2. Verification Gate
- 至少一次相关验证通过
- 无未解释失败

3. Consistency Gate
- 风格与仓库保持一致
- 无无关改动

4. Traceability Gate
- `task-queue.md` 状态已更新
- `memory/progress.md` 与 `decisions.md` 已记录
