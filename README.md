# Copilot Devin-Style Autonomous System

本仓库是一个可直接复用的 **Copilot Autonomous Engineer 模板**，用于把 GitHub Copilot Agent 的行为从“单轮执行”升级到“持续自治循环”。

## 目标
在 VS Code 中实现接近 Devin / Cursor Agent 的执行体验：
- 自动拆解任务
- 自动循环执行与修复
- 自动验证（测试/构建）
- 自动记录决策与进度
- 自动从中断状态恢复

## 关键增强（相对参考文档的升级）
1. **状态化任务队列**：支持 `todo/in_progress/blocked/done`
2. **质量闸门**：完成前必须通过 Correctness / Verification / Consistency / Traceability
3. **恢复协议**：中断后可依据 memory 继续，不丢上下文
4. **失败纪律**：3-strike 调试策略，防止无限重复失败
5. **记忆体系扩展**：新增 `lessons.md`，复用经验模式
6. **模板自检能力**：提供 `scripts/validate-template.ps1` 与最小 CI，持续检查结构和关键路径

> 这些增强让“接近度”不只依赖提示词，而是依赖**可持续执行机制**，实际效果通常明显高于基础模板。
## 核心工程约束

> 以下约束适用于本系统所有 skill、adapter、orchestrator 的实现层。新增任何模块前，必须满足全部条件。

### 1. 模块化 & 分层架构

系统採用四层模型，各层职责严格分离：

```
┌─────────────────────────────────────┐
│           Interface Layer           │  ← 用户交互 / CLI / API 入口
├─────────────────────────────────────┤
│         Orchestration Layer         │  ← 任务分解、skill 调度、retry 策略
├─────────────────────────────────────┤
│           Domain Layer              │  ← 核心业务逻辑（纯函数，无副作用）
├─────────────────────────────────────┤
│           Adapter Layer             │  ← 外部依赖接口（网络/文件/API/LLM）
└─────────────────────────────────────┘
```

**约束：**
- 新增需求 / 新工具接入 **只允许**在 Adapter Layer 新增文件，不得修改 Domain 核心。
- 上层可调用下层，下层严禁反向依赖上层。
- 每个模块单一职责，对外暴露最小接口。

---

### 2. 统一数据结构

所有跨层传递的数据必须使用以下四种标准结构，禁止裸传原始字符串或无类型 dict：

| 结构 | 用途 | 必填字段 |
|------|------|---------|
| `Request` | 用户/调用方输入 | `id`, `intent`, `params`, `context` |
| `Plan` | Orchestrator 生成的执行计划 | `id`, `steps[]`, `dependencies`, `metadata` |
| `Result` | 单步执行结果 | `step_id`, `status`, `output`, `error?` |
| `Output` | 最终交付物 | `request_id`, `content`, `format`, `trace_id` |

> 详细字段定义见 [`docs/data-contracts.md`](docs/data-contracts.md)。

---

### 3. 错误分类与统一封装

所有错误必须归入以下三类，并通过统一错误结构体传递：

| 类型 | 定义 | 处理策略 |
|------|------|---------|
| `UserError` | 用户输入非法、参数缺失 | 立即返回，提示修复方式 |
| `TransientError` | 网络超时、速率限制、临时不可用 | 指数退避重试，最多 3 次 |
| `SystemError` | 代码 bug、配置错误、不可恢复异常 | 记录 + 报警，停止执行 |

```python
# 统一错误结构（伪代码示意）
@dataclass
class AgentError:
    kind: Literal["User", "Transient", "System"]
    code: str          # 机器可读错误码
    message: str       # 人类可读说明
    context: dict      # 现场快照（request_id、step、input 摘要）
    retryable: bool    # 派生自 kind
    cause: Exception | None
```

---

### 4. 可测试性 & 离线单测

**硬性要求：**
- Domain Layer 所有函数必须是**纯函数**（相同输入 → 相同输出，无副作用）。
- Adapter Layer 必须以**接口/协议**定义，测试时注入 Fake/Stub，不调用真实外部服务。
- **单元测试不得依赖**：网络连接、API 密钥、真实文件系统、数据库。
- CI pipeline 中必须存在 `test:unit` 任务，可在完全离线环境运行。

```python
# 正确示例：Domain 函数无外部依赖
def parse_plan(raw_text: str) -> Plan:
    ...

# 正确示例：Adapter 通过接口注入
class LLMAdapter(Protocol):
    def complete(self, prompt: str) -> str: ...

def create_orchestrator(llm: LLMAdapter) -> Orchestrator:
    ...

# 测试时使用 Fake
class FakeLLM:
    def complete(self, prompt: str) -> str:
        return '{"steps": []}'
```

---

### 5. 可复用共享能力

以下通用能力必须放入 `core/` 共享包，**禁止在各 skill 内重复实现**：

| 能力 | 模块路径 | 说明 |
|------|---------|------|
| 请求解析 | `core/parser.py` | 统一输入规范化 |
| 参数校验 | `core/validator.py` | 声明式 schema 校验 |
| 重试机制 | `core/retry.py` | 指数退避 + jitter |
| 缓存 | `core/cache.py` | 内存/磁盘二级缓存 |
| 日志 | `core/logger.py` | 结构化日志，含 trace_id |
| 格式化输出 | `core/formatter.py` | Markdown/JSON/Plain 多格式 |
| 错误封装 | `core/errors.py` | `AgentError` 及三类子类 |

---

### 6. 可观测性 & 埋点规范

每一层都必须在以下位置记录结构化日志（含 `trace_id`）：

```
Interface   → 记录原始 input、请求开始/结束时间
Orchestrator → 记录 Plan 生成、每步开始/结束/重试
Domain      → 记录关键计算节点的输入/输出摘要
Adapter     → 记录每次外部调用的入参、响应码、耗时
```

**埋点字段标准：**

```json
{
  "trace_id": "uuid-v4",
  "layer": "orchestrator | domain | adapter",
  "event": "step.start | step.success | step.fail | retry",
  "step_id": "string",
  "duration_ms": 123,
  "input_summary": "...",
  "output_summary": "...",
  "error": null
}
```

---

### 7. 新增工具 / 能力的扩展规则

新增任何外部工具或能力时，执行以下**最小变更原则**：

```
1. 在 adapters/<tool_name>/  新建 adapter 文件（实现 Adapter Protocol）
2. 在 adapters/<tool_name>/  新建对应 Fake（用于单测）
3. 在 orchestrator/registry.py 注册新 adapter（一行配置）
4. 新增 skill 时，在 skills/<skill_name>/ 创建编排逻辑
5. 禁止因新增工具而修改 Domain Layer 任何现有文件
```

> 遵守此规则可保证：**旧 skill 的单元测试永远不会因新工具接入而失败**。

---

### 合规自检清单

在每个 PR/任务完成前，必须能回答 Yes 的问题：

- [ ] Domain Layer 有纯函数实现，无网络/文件/API 调用？
- [ ] 所有外部依赖通过 Protocol 注入，有对应 Fake？
- [ ] 单元测试在断网 + 无密钥环境下通过？
- [ ] 错误已归类为 User / Transient / System？
- [ ] 跨层数据使用 Request / Plan / Result / Output 标准结构？
- [ ] 新增能力未修改任何 Domain 现有文件？
- [ ] 日志含 trace_id 且结构化？

> 详细规范见 [`docs/engineering-requirements.md`](docs/engineering-requirements.md)

---
## 目录结构

```text
.github/
  copilot-agent.md
  devin-loop.md
  task-queue.md
  project-context.md
  workflows/
    template-validation.yml
  checklists/
    definition-of-done.md
    recovery.md
  memory/
    goals.md
    decisions.md
    progress.md
    bugs.md
    lessons.md
  instructions/
    autonomous.md
    planning.md
    execution.md
    debugging.md
    testing.md
    refactor.md
    git.md
    quality-gate.md
    resume.md
.vscode/settings.json
docs/autonomous-workflow.md
docs/engineering-requirements.md
docs/data-contracts.md
docs/quickstart.md
scripts/validate-template.ps1
task_plan.md
findings.md
progress.md
```

## 使用步骤
1. 先按 [`docs/quickstart.md`](docs/quickstart.md) 重置模板状态并填入真实项目上下文。
2. 在 `.github/task-queue.md` 写入当前目标与任务。
3. 在 `.github/project-context.md` 补充项目约束。
4. 按 `.github/devin-loop.md` 执行循环（实现→验证→修复→记录）。
5. 每轮更新 `.github/memory/progress.md` 与 `.github/memory/decisions.md`。
6. 运行 `pwsh ./scripts/validate-template.ps1`，确认模板结构与关键引用仍然有效。
7. 通过 `.github/instructions/quality-gate.md` 后再标记任务 `done`。

## 仓库自检

执行命令：

```powershell
pwsh ./scripts/validate-template.ps1
```

当前脚本会检查：
- 模板关键文件和目录是否齐全
- `.github/task-queue.md` 是否满足状态格式且最多只有一个 `in_progress`
- README、workflow 示例和 memory 文档是否保留关键入口

最小 CI 已内置在 `.github/workflows/template-validation.yml`。当你把这个模板接入真实代码仓库后，应在此基础上继续增加真实的单元测试和构建流水线。

## 推荐运行方式
- 每次对话开头先读取：`.github/task-queue.md` + `.github/memory/progress.md`
- 每次对话结尾写回：`.github/task-queue.md` + `.github/memory/*`
- 遇到中断时使用：`.github/instructions/resume.md`

## 边界说明
- 该模板能显著提升自治能力，但仍受平台安全与权限限制。
- 建议在真实项目中结合 CI、测试框架与 PR 流程使用，效果最佳。
