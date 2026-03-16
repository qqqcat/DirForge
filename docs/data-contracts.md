# Data Contracts — 统一数据结构定义

> 本文件是系统所有跨层数据传递的唯一权威定义。  
> 任何字段变更必须同步更新本文件，并同步修改所有使用位置。

---

## 概览

| 结构 | 流向 | 说明 |
|------|------|------|
| `Request` | Interface → Orchestrator | 用户输入的规范化结构 |
| `Plan` | Orchestrator 内部 | 执行计划，含步骤依赖 |
| `Result` | Domain/Adapter → Orchestrator | 单步骤执行结果 |
| `Output` | Orchestrator → Interface | 最终交付，含完整溯源链 |

---

## Request

```
Request
├── id: str                   必填 UUID-v4，全链路唯一
├── intent: str               必填 一句话意图摘要（< 200 字符）
├── params: dict[str, Any]    必填 结构化参数（各 skill 自定义 schema）
├── context: RequestContext   必填 上下文快照
└── created_at: datetime      必填 UTC 时间

RequestContext
├── session_id: str           必填
├── user_id: str | None       可选
├── history_summary: str|None 可选 历史摘要（非原始历史）
└── metadata: dict[str, Any]  可选 扩展字段
```

**约束：**
- `id` 由 Interface Layer 生成，全链路不可修改。
- `params` 内容由各 skill 的 `schema.py` 定义并校验，Interface Layer 不做假设。
- `history_summary` 长度不超过 2000 字符（超出需截断）。

---

## Plan

```
Plan
├── id: str                   必填 UUID-v4
├── request_id: str           必填 关联的 Request.id
├── steps: list[PlanStep]     必填 有序步骤列表
├── created_at: datetime      必填
└── metadata: dict[str, Any]  可选 skill 名称、预估耗时等

PlanStep
├── id: str                   必填 在本 Plan 内唯一，形如 "step-001"
├── skill: str                必填 对应 skills/registry.py 中的 skill 名
├── action: str               必填 该 skill 内的具体动作名
├── params: dict[str, Any]    必填 该步骤的输入参数
├── depends_on: list[str]     必填 依赖的 step id 列表（无依赖时为 []）
└── retry_policy: RetryPolicy 必填 重试策略

RetryPolicy
├── max_attempts: int         默认 3
├── base_delay_ms: int        默认 500
├── max_delay_ms: int         默认 10000
└── jitter: bool              默认 true
```

**约束：**
- `depends_on` 引用的 step id 必须在同一 Plan 中存在（Orchestrator 校验）。
- 循环依赖视为 SystemError，Plan 生成立即失败。
- `retry_policy` 默认值在 `core/retry.py` 中定义为常量。

---

## Result

```
Result
├── step_id: str              必填 对应 PlanStep.id
├── status: enum              必填 "success" | "failure" | "skipped"
├── output: Any               条件必填 status=success 时必填
├── error: AgentError | None  条件必填 status=failure 时必填
├── duration_ms: int          必填 该步骤实际耗时
└── trace_id: str             必填 与 Request.id 关联的 trace_id
```

**约束：**
- `output` 的类型由各 skill 在文档中声明，调用方需知晓类型再使用。
- `status=skipped` 用于 `depends_on` 的前置步骤失败导致本步骤未执行。
- `duration_ms` 包含重试耗时（所有 attempt 之和）。

---

## Output

```
Output
├── request_id: str           必填 关联的 Request.id
├── content: str|dict|list    必填 最终交付内容
├── format: enum              必填 "markdown" | "json" | "plain"
├── trace_id: str             必填
├── results: list[Result]     必填 所有步骤结果（含 skipped）
└── created_at: datetime      必填
```

**约束：**
- `content` 的类型与 `format` 必须一致：
  - `format=json` → `content` 必须是 `dict` 或 `list`。
  - 其余 → `content` 必须是 `str`。
- `results` 顺序与 `Plan.steps` 顺序一致，便于对照溯源。

---

## 版本控制

| 版本 | 日期 | 变更说明 |
|------|------|---------|
| v1.0 | 2026-02-21 | 初始定义 |

> 结构字段的 **删除或重命名** 视为破坏性变更，需更新版本号并通知所有使用方。  
> 新增可选字段不视为破坏性变更。
