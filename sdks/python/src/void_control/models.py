from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(slots=True)
class BridgeError(Exception):
    message: str
    code: str | None = None
    retryable: bool | None = None

    def __str__(self) -> str:
        return self.message


@dataclass(slots=True)
class TemplateSummary:
    id: str
    name: str
    execution_kind: str
    description: str

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "TemplateSummary":
        return cls(
            id=str(payload["id"]),
            name=str(payload["name"]),
            execution_kind=str(payload["execution_kind"]),
            description=str(payload["description"]),
        )


@dataclass(slots=True)
class TemplateDetail:
    id: str
    name: str
    execution_kind: str
    description: str
    inputs: dict[str, Any]
    workflow_template: str
    bindings: list[dict[str, Any]]

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "TemplateDetail":
        template = payload["template"]
        defaults = payload.get("defaults", {})
        compile_data = payload.get("compile", {})
        return cls(
            id=str(template["id"]),
            name=str(template["name"]),
            execution_kind=str(template["execution_kind"]),
            description=str(template["description"]),
            inputs=dict(payload.get("inputs", {})),
            workflow_template=str(defaults.get("workflow_template", "")),
            bindings=list(compile_data.get("bindings", [])),
        )


@dataclass(slots=True)
class CompiledTemplatePreview:
    goal: str
    workflow_template: str
    mode: str
    variation_source: str
    candidates_per_iteration: int
    candidate_overrides: list[dict[str, str]]
    overrides: dict[str, str]

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "CompiledTemplatePreview":
        return cls(
            goal=str(payload["goal"]),
            workflow_template=str(payload["workflow_template"]),
            mode=str(payload["mode"]),
            variation_source=str(payload["variation_source"]),
            candidates_per_iteration=int(payload["candidates_per_iteration"]),
            candidate_overrides=[dict(item) for item in payload.get("candidate_overrides", [])],
            overrides=dict(payload.get("overrides", {})),
        )


@dataclass(slots=True)
class TemplateDryRunResult:
    template_id: str
    execution_kind: str
    inputs: dict[str, Any]
    compiled: CompiledTemplatePreview

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "TemplateDryRunResult":
        template = payload["template"]
        return cls(
            template_id=str(template["id"]),
            execution_kind=str(template["execution_kind"]),
            inputs=dict(payload.get("inputs", {})),
            compiled=CompiledTemplatePreview.from_json(dict(payload["compiled"])),
        )


@dataclass(slots=True)
class TemplateExecutionResult:
    execution_id: str
    template_id: str
    execution_kind: str
    status: str
    goal: str

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "TemplateExecutionResult":
        template = payload["template"]
        return cls(
            execution_id=str(payload["execution_id"]),
            template_id=str(template["id"]),
            execution_kind=str(template["execution_kind"]),
            status=str(payload["status"]),
            goal=str(payload["goal"]),
        )


@dataclass(slots=True)
class ExecutionRecord:
    execution_id: str
    goal: str
    status: str

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "ExecutionRecord":
        return cls(
            execution_id=str(payload["execution_id"]),
            goal=str(payload["goal"]),
            status=str(payload["status"]),
        )


@dataclass(slots=True)
class ExecutionResult:
    best_candidate_id: str | None
    completed_iterations: int
    total_candidate_failures: int

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "ExecutionResult":
        best_candidate = payload.get("best_candidate_id")
        return cls(
            best_candidate_id=None if best_candidate is None else str(best_candidate),
            completed_iterations=int(payload.get("completed_iterations", 0)),
            total_candidate_failures=int(payload.get("total_candidate_failures", 0)),
        )


@dataclass(slots=True)
class ExecutionDetail:
    execution: ExecutionRecord
    progress: dict[str, Any]
    result: ExecutionResult
    candidates: list[dict[str, Any]]

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "ExecutionDetail":
        return cls(
            execution=ExecutionRecord.from_json(dict(payload["execution"])),
            progress=dict(payload.get("progress", {})),
            result=ExecutionResult.from_json(dict(payload.get("result", {}))),
            candidates=list(payload.get("candidates", [])),
        )


@dataclass(slots=True)
class BatchRunResult:
    kind: str
    run_id: str
    execution_id: str
    compiled_primitive: str
    status: str
    goal: str

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "BatchRunResult":
        return cls(
            kind=str(payload["kind"]),
            run_id=str(payload["run_id"]),
            execution_id=str(payload["execution_id"]),
            compiled_primitive=str(payload["compiled_primitive"]),
            status=str(payload["status"]),
            goal=str(payload["goal"]),
        )


@dataclass(slots=True)
class BatchRunDetail:
    kind: str
    run_id: str
    execution: ExecutionRecord
    progress: dict[str, Any]
    result: ExecutionResult
    candidates: list[dict[str, Any]]

    @classmethod
    def from_json(cls, payload: dict[str, Any]) -> "BatchRunDetail":
        return cls(
            kind=str(payload["kind"]),
            run_id=str(payload["run_id"]),
            execution=ExecutionRecord.from_json(dict(payload["execution"])),
            progress=dict(payload.get("progress", {})),
            result=ExecutionResult.from_json(dict(payload.get("result", {}))),
            candidates=list(payload.get("candidates", [])),
        )
