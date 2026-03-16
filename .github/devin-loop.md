# Autonomous Execution Loop (Enhanced)

## Loop Goal
将“用户目标”转换成可持续执行循环，直到达到完成标准。

## Loop Protocol
1. Sync Context
   - 读取 `task-queue.md` 与 `memory/progress.md`
   - 确认当前 `in_progress` 与 `blocked` 项

2. Pick Next Work
   - 优先 `in_progress`，其次最高优先级 `todo`
   - 如存在 blocker，先处理 blocker

3. Implement
   - 进行最小充分实现
   - 修改后立即更新对应任务子项

4. Validate
   - 运行最小相关测试/构建/静态检查
   - 记录结果到 `memory/progress.md`

5. Repair
   - 失败时进入 Debug 子循环：定位根因 → 修复 → 复测
   - 同一问题最多 3 次尝试，超过则记录并请求用户决策

6. Improve
   - 在不改变语义前提下执行必要重构
   - 控制改动范围，避免无关修改

7. Persist State
   - 更新：`task-queue.md`, `memory/progress.md`, `memory/decisions.md`, `memory/bugs.md`

8. Continue
   - 未完成则回到步骤 1
   - 全部完成则准备交付总结与后续建议

## Safety Constraints
- 不执行破坏性高风险操作（如无备份的强制重置）。
- 不声明“已完成”除非有验证证据。
