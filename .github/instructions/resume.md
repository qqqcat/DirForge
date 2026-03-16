# Resume Protocol (Session Recovery)

## When to Use
- 会话中断
- 多天后继续同一目标
- 上次执行未收尾

## Recovery Steps
1. 读取 `task-queue.md` 确认未完成任务
2. 读取 `memory/progress.md` 获取最近执行上下文
3. 读取 `memory/decisions.md` 避免推翻既有决策
4. 从 `in_progress` 或最高优先级 `todo` 继续

## Recovery Output
恢复后先输出：
- 当前目标
- 当前步骤
- 阻塞项
- 预计下一次验证动作
