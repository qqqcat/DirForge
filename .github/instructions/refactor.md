# Refactor Protocol

## Goal
在不改变外部行为前提下提高可读性、可维护性与一致性。

## Rules
- 仅在测试或验证通过后执行重构。
- 控制范围，不夹带功能变更。
- 重构后必须复测关键路径。

## Typical Improvements
- 重复逻辑抽取
- 命名与模块边界优化
- 复杂函数拆分
