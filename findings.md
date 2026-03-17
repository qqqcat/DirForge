# Findings

## 2026-03-16 Project Assessment

## Verification
- `cargo check --workspace`：通过
- `cargo test --workspace`：通过

## Current Strengths
- 扫描链路已形成并发 worker + 聚合线程 + 有界发布通道的稳定流水线，取消、错误和完成态都可回归验证。
- UI 已从“调试面板”演进为面向用户的磁盘分析器：Overview / Live Scan / Treemap / History / Errors / Diagnostics / Settings 结构清晰。
- 中文字体回退、人类可读大小格式化、treemap 标签阈值和排行榜滚动都已落地，基础可读性达标。
- 删除动作已进入右侧 Inspector，可直接执行“移到回收站 / 永久删除”，不再依赖孤立的 `Operations` 页面。
- 删除成功后已支持局部刷新：Inspector、排行榜、概览统计、treemap 会同步更新，不必等待下一次全盘重扫。
- 永久删除已增加确认层，删除结果会给出更明确的撤销或失败提示。
- 启动默认根路径不再是 `.`，并新增盘符快捷扫描按钮，降低首次使用门槛。

## Document Drift Fixed
- 旧文档仍在描述 `Operations` 页面与“模拟执行优先”的单独流程，已与现状不符。
- `task_plan.md`、`findings.md` 原先仍是历史模板仓库内容，已改为 DirForge 当前评估与工作记录。
- 使用文档中“扫描结束后去操作页执行动作”的描述已过期，需要统一到 Inspector 内动作模型。

## Current Risks
- 真实删除跨平台边界覆盖仍有限，尤其是权限不足、占用锁、系统目录与回收站失败回退场景。
- 当前主要依赖 Rust 单测与集成测试，缺少自动化 UI 交互回归，像重复 widget ID 这类问题仍要靠人工发现。
- 当前 UI 自动化回归仍以逻辑级单测为主，尚未覆盖真实点击流和弹窗交互。

## Recommended Next Steps
1. 增加真实 UI 集成回归，覆盖盘符快捷扫描、删除确认和删除后刷新。
2. 把真实删除边界测试扩展到更多真实平台场景。
3. 继续优化高风险路径的确认文案和警示表现。
