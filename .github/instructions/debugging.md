# Debugging Protocol

## Root-Cause First
- 先定位触发条件与根因，再改代码。
- 禁止仅通过屏蔽异常或绕过路径“假修复”。

## 3-Strike Strategy
1. 第一次：精准修复最可能根因
2. 第二次：替代方案修复
3. 第三次：重审假设并升级问题

## Bug Logging
每次失败都记录到 `memory/bugs.md`：症状、根因、修复、预防。
