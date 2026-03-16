# Decisions Log

## Template
- Date:
- Decision:
- Why:
- Impact:
- Revisit Trigger:

## 2026-02-16
- Decision: 使用状态化任务队列（todo/in_progress/blocked/done）。
- Why: 静态清单无法支撑持续执行与恢复。
- Impact: 提升续跑能力与过程透明度。
- Revisit Trigger: 若项目规模非常小，可降级为简单 checklist。

## 2026-02-16
- Decision: 引入质量闸门与 3-strike 失败协议。
- Why: 减少重复失败与“假完成”。
- Impact: 提高稳定性，降低回归风险。
- Revisit Trigger: 若任务是一次性 PoC，可简化闸门。

## 2026-03-09
- Decision: 为模板仓库加入 `scripts/validate-template.ps1` 与最小 CI workflow。
- Why: 当前仓库主要是文档模板，最缺的是可执行自检入口；先解决结构一致性，才能支撑后续真实项目接入。
- Impact: 模板从“只描述流程”提升为“可以持续校验自身结构”，更适合作为仓库脚手架复用。
- Revisit Trigger: 当仓库引入真实代码和测试后，应把校验重点从结构完整性扩展到单元测试、构建和 lint。

## 2026-03-09
- Decision: 单独增加 `docs/quickstart.md`，而不是继续把上手说明堆进 README。
- Why: README 已承载愿景、约束和目录说明；首次接入动作需要更短、更直接、更可执行的说明。
- Impact: 降低首次使用成本，也减少模板使用者误改状态文件的概率。
- Revisit Trigger: 若后续模板演化为带 CLI 的脚手架，可将 quickstart 收敛为命令式初始化流程。
