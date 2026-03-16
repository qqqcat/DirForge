# 工程设计要求（Engineering Requirements）

> 本文档是所有 skill、adapter、orchestrator 实现的强制性规范。  
> README 中的"核心工程约束"章节是摘要，本文件提供完整细节与示例。

---

## 目录

1. [分层架构](#1-分层架构)
2. [统一数据结构](#2-统一数据结构)
3. [错误分类与封装](#3-错误分类与封装)
4. [可测试性与离线单测](#4-可测试性与离线单测)
5. [可复用共享能力](#5-可复用共享能力)
6. [可观测性与埋点](#6-可观测性与埋点)
7. [扩展规则（新增工具/能力）](#7-扩展规则)
8. [合规自检清单](#8-合规自检清单)

---

## 1. 分层架构

### 1.1 层次定义

```
┌───────────────────────────────────────────────────────┐
│                   Interface Layer                      │
│  职责：接收原始用户输入，规范化为 Request，返回 Output  │
│  禁止：包含任何业务逻辑、直接调用外部 API               │
├───────────────────────────────────────────────────────┤
│                 Orchestration Layer                    │
│  职责：将 Request 拆解为 Plan，调度 skills，处理 retry  │
│  禁止：直接执行业务逻辑、直接操作外部资源               │
├───────────────────────────────────────────────────────┤
│                   Domain Layer                        │
│  职责：纯函数业务逻辑（解析、推理、转换、校验）          │
│  禁止：任何 I/O、网络调用、文件读写、随机/时间副作用     │
├───────────────────────────────────────────────────────┤
│                   Adapter Layer                       │
│  职责：封装所有外部依赖（LLM / HTTP / FS / DB / SDK）   │
│  禁止：包含业务逻辑、跨 adapter 直接调用               │
└───────────────────────────────────────────────────────┘
```

### 1.2 依赖方向规则

- 合法：`Interface → Orchestration → Domain`
- 合法：`Orchestration → Adapter（通过 Protocol）`
- **非法**：`Domain → Adapter`（直接调用）
- **非法**：`Adapter → Domain` 或 `Adapter → Orchestration`

### 1.3 目录结构对应

```
src/
  interface/          ← Interface Layer
  orchestrator/       ← Orchestration Layer
  domain/             ← Domain Layer（纯逻辑）
  adapters/           ← Adapter Layer（外部依赖封装）
    llm/
    http/
    filesystem/
    <new_tool>/       ← 新工具只在此处新增
  core/               ← 跨层共享工具（无业务逻辑）
  skills/             ← 各 skill 的编排入口
tests/
  unit/               ← 必须可离线运行
  integration/        ← 允许外部依赖，CI 可选
```

---

## 2. 统一数据结构

### 2.1 Request

```python
@dataclass
class Request:
    id: str                        # UUID v4，全链路唯一标识
    intent: str                    # 用户意图摘要（一句话）
    params: dict[str, Any]         # 结构化参数
    context: RequestContext        # 上下文快照（session、history 摘要）
    created_at: datetime

@dataclass
class RequestContext:
    session_id: str
    user_id: str | None
    history_summary: str | None    # 历史交互摘要，非原始历史
    metadata: dict[str, Any]
```

### 2.2 Plan

```python
@dataclass
class Plan:
    id: str
    request_id: str
    steps: list[PlanStep]
    created_at: datetime
    metadata: dict[str, Any]       # skill 名称、预估耗时等

@dataclass
class PlanStep:
    id: str
    skill: str                     # 对应 skills/ 目录的 skill 名
    action: str
    params: dict[str, Any]
    depends_on: list[str]          # 依赖的 step id 列表
    retry_policy: RetryPolicy
```

### 2.3 Result

```python
@dataclass
class Result:
    step_id: str
    status: Literal["success", "failure", "skipped"]
    output: Any                    # 结构化，类型由 skill 定义
    error: AgentError | None
    duration_ms: int
    trace_id: str
```

### 2.4 Output

```python
@dataclass
class Output:
    request_id: str
    content: str | dict | list     # 最终交付内容
    format: Literal["markdown", "json", "plain"]
    trace_id: str
    results: list[Result]          # 所有步骤结果（用于溯源）
    created_at: datetime
```

### 2.5 使用规范

- 所有跨层调用只允许传递以上四种结构。
- 禁止在函数签名中使用裸 `str`、`dict` 代替标准结构。
- 结构字段可以扩展，但不可删除现有必填字段。

---

## 3. 错误分类与封装

### 3.1 三类错误定义

```python
from dataclasses import dataclass, field
from typing import Literal
from datetime import datetime

@dataclass
class AgentError(Exception):
    kind: Literal["User", "Transient", "System"]
    code: str              # 形如 "INVALID_PARAM" / "LLM_TIMEOUT" / "BUG_PARSE_FAIL"
    message: str           # 人类可读，可直接展示
    context: dict          # 现场快照：request_id、step_id、input 摘要
    retryable: bool        # 自动派生：Transient=True，其余=False
    cause: Exception | None = None
    occurred_at: datetime = field(default_factory=datetime.utcnow)

class UserError(AgentError):
    """用户输入错误：参数缺失、格式非法、意图不明确。
    处理：立即返回错误信息，引导用户修复，不重试。"""
    def __init__(self, code: str, message: str, context: dict = {}, cause=None):
        super().__init__("User", code, message, context, retryable=False, cause=cause)

class TransientError(AgentError):
    """暂时性错误：网络超时、速率限制、服务临时不可用。
    处理：指数退避重试，最多 N 次（由 RetryPolicy 控制）。"""
    def __init__(self, code: str, message: str, context: dict = {}, cause=None):
        super().__init__("Transient", code, message, context, retryable=True, cause=cause)

class SystemError(AgentError):
    """系统级错误：代码 bug、配置缺失、不可恢复异常。
    处理：记录日志 + 告警，停止当前执行链，不重试。"""
    def __init__(self, code: str, message: str, context: dict = {}, cause=None):
        super().__init__("System", code, message, context, retryable=False, cause=cause)
```

### 3.2 错误码命名约定

| 前缀 | 类型 | 示例 |
|------|------|------|
| `INVALID_` | UserError | `INVALID_PARAM`, `INVALID_FORMAT` |
| `MISSING_` | UserError | `MISSING_INTENT`, `MISSING_REQUIRED_FIELD` |
| `TIMEOUT_` | TransientError | `TIMEOUT_LLM`, `TIMEOUT_HTTP` |
| `RATE_` | TransientError | `RATE_LIMIT_LLM` |
| `BUG_` | SystemError | `BUG_PARSE_FAIL`, `BUG_UNEXPECTED_STATE` |
| `CONFIG_` | SystemError | `CONFIG_MISSING_KEY` |

### 3.3 Orchestration 层统一处理

```python
def handle_error(error: AgentError, step: PlanStep, attempt: int) -> StepAction:
    if error.retryable and attempt < step.retry_policy.max_attempts:
        return StepAction.RETRY
    elif error.kind == "User":
        return StepAction.ABORT_WITH_MESSAGE   # 向用户说明
    else:
        return StepAction.ABORT_WITH_LOG       # 系统级停止
```

---

## 4. 可测试性与离线单测

### 4.1 Domain Layer 纯函数要求

```python
# ✅ 合规：纯函数，可直接断言
def parse_plan(raw_text: str, schema_version: str) -> Plan:
    """无副作用，无 I/O，相同输入永远得到相同输出。"""
    ...

# ❌ 违规：函数内直接调用 LLM
def parse_plan(raw_text: str) -> Plan:
    response = openai.complete(raw_text)   # 禁止
    ...
```

### 4.2 Adapter Protocol 注入模式

```python
from typing import Protocol

# 在 adapters/ 定义协议（接口）
class LLMAdapter(Protocol):
    def complete(self, prompt: str, **kwargs) -> str: ...
    def stream(self, prompt: str, **kwargs) -> Iterator[str]: ...

class FileSystemAdapter(Protocol):
    def read(self, path: str) -> str: ...
    def write(self, path: str, content: str) -> None: ...

# Orchestrator 只依赖协议
class Orchestrator:
    def __init__(self, llm: LLMAdapter, fs: FileSystemAdapter):
        self._llm = llm
        self._fs = fs
```

### 4.3 Fake 实现规范

```python
# 每个 Adapter 必须配套一个 Fake，放在 adapters/<name>/fake.py
class FakeLLM:
    """离线测试用，预设固定返回值。"""
    def __init__(self, responses: dict[str, str] | None = None):
        self._responses = responses or {}
        self.calls: list[str] = []            # 可断言调用历史

    def complete(self, prompt: str, **kwargs) -> str:
        self.calls.append(prompt)
        for key, val in self._responses.items():
            if key in prompt:
                return val
        return '{"steps": [], "status": "ok"}'   # 默认安全响应

    def stream(self, prompt: str, **kwargs) -> Iterator[str]:
        yield self.complete(prompt)
```

### 4.4 单测禁止项

| 禁止 | 替代方案 |
|------|---------|
| `import requests` 并真实发送 | 使用 `FakeHTTP` |
| 读取真实文件路径 | 使用 `FakeFS` 或 `tmp_path` fixture（仅集成测试） |
| 设置环境变量 `OPENAI_API_KEY` | 使用 `FakeLLM` |
| `time.sleep()` 超过 100ms | Mock `time.sleep` |
| 依赖网络 DNS 解析 | 使用 `FakeHTTP` |

### 4.5 CI 配置要求

当前模板仓库内置的是 `.github/workflows/template-validation.yml`，用于校验模板结构、关键入口和任务状态格式。
当该模板接入真实代码仓库后，必须继续补充真实的离线单元测试流水线，例如：

```yaml
# .github/workflows/test.yml（示例）
jobs:
  unit-test:
    runs-on: ubuntu-latest
    # 故意不设置任何 secrets，验证离线能力
    steps:
      - run: pytest tests/unit/ -v --tb=short
```

---

## 5. 可复用共享能力

### 5.1 core/ 模块清单

所有跨 skill 共用能力必须放在 `core/`，**禁止各 skill 单独实现**：

#### `core/parser.py` — 请求解析

```python
def parse_request(raw_input: str | dict) -> Request:
    """将任意原始输入规范化为 Request 结构。"""

def extract_intent(text: str) -> str:
    """从自然语言中提取核心意图，返回单行摘要。"""
```

#### `core/validator.py` — 参数校验

```python
def validate_request(req: Request, schema: dict) -> list[ValidationError]:
    """声明式 schema 校验，返回所有违规项（不抛异常）。"""

def require_fields(data: dict, fields: list[str]) -> None:
    """批量必填校验，缺失则抛 UserError。"""
```

#### `core/retry.py` — 重试机制

```python
@dataclass
class RetryPolicy:
    max_attempts: int = 3
    base_delay_ms: int = 500
    max_delay_ms: int = 10_000
    jitter: bool = True
    retryable_on: tuple[type[Exception], ...] = (TransientError,)

def with_retry(fn: Callable, policy: RetryPolicy) -> Any:
    """带指数退避 + jitter 的重试包装器。"""
```

#### `core/cache.py` — 缓存

```python
class TwoLevelCache:
    """内存（LRU）+ 磁盘二级缓存，键为请求内容 hash。"""
    def get(self, key: str) -> Any | None: ...
    def set(self, key: str, value: Any, ttl_seconds: int = 3600) -> None: ...
    def invalidate(self, key: str) -> None: ...
```

#### `core/logger.py` — 结构化日志

```python
def get_logger(layer: str) -> StructuredLogger:
    """返回绑定 layer 名称的结构化日志器，自动注入 trace_id。"""

class StructuredLogger:
    def info(self, event: str, **fields) -> None: ...
    def error(self, event: str, error: AgentError, **fields) -> None: ...
    def timed(self, event: str) -> ContextManager: ...  # 自动记录耗时
```

#### `core/formatter.py` — 输出格式化

```python
def format_output(output: Output, fmt: Literal["markdown", "json", "plain"]) -> str:
    """将 Output 结构序列化为目标格式字符串。"""
```

#### `core/errors.py` — 错误基类与三类子类

见第 3 节完整定义。

---

## 6. 可观测性与埋点

### 6.1 埋点位置清单

| 层 | 埋点时机 | 必含字段 |
|----|---------|---------|
| Interface | 收到请求、返回响应 | `trace_id`, `intent`, `duration_ms` |
| Orchestrator | Plan 生成、每步开始/结束/重试/跳过 | `trace_id`, `step_id`, `skill`, `attempt` |
| Domain | 关键计算节点（parse、validate、transform） | `trace_id`, `input_hash`, `output_hash` |
| Adapter | 每次外部调用前后 | `trace_id`, `adapter`, `status_code`, `duration_ms` |

### 6.2 日志格式

```json
{
  "ts": "2026-02-21T10:00:00.000Z",
  "trace_id": "550e8400-e29b-41d4-a716-446655440000",
  "layer": "orchestrator",
  "event": "step.start",
  "step_id": "step-001",
  "skill": "code_review",
  "attempt": 1,
  "input_summary": "PR #42, 3 files changed",
  "output_summary": null,
  "error": null,
  "duration_ms": null
}
```

### 6.3 trace_id 传播规则

- `trace_id` 由 Interface Layer 在收到 Request 时生成（UUID v4）。
- 所有下层调用必须透传 `trace_id`，**不允许在中途重新生成**。
- Adapter 层调用外部服务时，将 `trace_id` 注入请求头（`X-Trace-Id`）。

---

## 7. 扩展规则

### 7.1 新增外部工具（如接入新 LLM 或 API）

```
Step 1: 在 adapters/<tool_name>/ 创建以下文件：
  - adapter.py     ← 实现已有 Protocol
  - fake.py        ← 离线 Fake 实现
  - tests/         ← adapter 自身的单元测试（使用 Fake）

Step 2: 在 adapters/registry.py 注册：
  ADAPTERS["<tool_name>"] = MyToolAdapter

Step 3: 更新 docs/adapters.md（列出新 adapter 的能力与配置项）
```

**禁止：** 修改 `domain/`、`core/` 的任何现有文件。

### 7.2 新增 Skill

```
Step 1: 在 skills/<skill_name>/ 创建：
  - skill.py       ← 编排逻辑（调用 domain + adapters）
  - schema.py      ← 该 skill 的 Request 参数 schema
  - tests/         ← 使用 Fake adapter 的单元测试

Step 2: 在 skills/registry.py 注册：
  SKILLS["<skill_name>"] = MySkill

Step 3: 更新 docs/skills.md（描述 skill 功能、输入输出）
```

### 7.3 变更约束矩阵

| 变更类型 | 允许修改的目录 | 禁止修改 |
|---------|-------------|---------|
| 新增工具 | `adapters/` | `domain/`, `core/` |
| 新增 skill | `skills/`, `adapters/` | `domain/`, `core/` |
| 修复 bug | 出 bug 的具体文件 | 无关模块 |
| 更新数据结构 | `core/models.py` + 所有使用处 | — |
| 性能优化 | `core/cache.py`, `core/retry.py` | 不改接口签名 |

---

## 8. 合规自检清单

每次提交（PR）前逐项检查：

### 架构合规
- [ ] 新文件放在正确的层目录（interface/orchestrator/domain/adapter/core/skills）
- [ ] 没有跨层反向依赖（下层引用上层模块）
- [ ] 新增功能没有修改 domain/ 现有文件

### 数据结构合规
- [ ] 跨层调用使用 Request / Plan / Result / Output
- [ ] 没有裸 `dict` 或 `str` 跨层传递业务数据

### 错误处理合规
- [ ] 所有可预见异常已归类为 User / Transient / System
- [ ] 使用 `AgentError` 子类而非裸 `Exception`
- [ ] TransientError 有对应 RetryPolicy

### 测试合规
- [ ] 新增模块有对应单元测试文件
- [ ] 单元测试不使用真实网络/文件/API
- [ ] 新 Adapter 有对应 Fake 实现
- [ ] `pytest tests/unit/` 可在断网环境通过

### 可观测性合规
- [ ] 关键操作有结构化日志，含 trace_id
- [ ] Adapter 调用前后有耗时埋点
- [ ] 错误日志包含 AgentError 的 context 字段

### 可复用性合规
- [ ] 通用能力（重试/缓存/日志/校验）使用 core/ 而非自行实现
- [ ] 共享逻辑没有被复制粘贴到多个模块

---

> 如有约束无法满足，必须在 PR 描述中说明原因并获得审批，**不允许静默绕过**。
