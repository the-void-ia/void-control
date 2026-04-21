export class BridgeError extends Error {
  constructor(message, { code = null, retryable = null } = {}) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
    this.retryable = retryable;
  }
}

export function toTemplateSummary(payload) {
  return {
    id: String(payload.id),
    name: String(payload.name),
    executionKind: String(payload.execution_kind),
    description: String(payload.description)
  };
}

export function toTemplateDetail(payload) {
  return {
    id: String(payload.template.id),
    name: String(payload.template.name),
    executionKind: String(payload.template.execution_kind),
    description: String(payload.template.description),
    inputs: payload.inputs ?? {},
    workflowTemplate: String(payload.defaults?.workflow_template ?? ""),
    bindings: payload.compile?.bindings ?? []
  };
}

export function toTemplateDryRunResult(payload) {
  return {
    templateId: String(payload.template.id),
    executionKind: String(payload.template.execution_kind),
    inputs: payload.inputs ?? {},
    compiled: {
      goal: String(payload.compiled.goal),
      workflowTemplate: String(payload.compiled.workflow_template),
      mode: String(payload.compiled.mode),
      variationSource: String(payload.compiled.variation_source),
      candidatesPerIteration: Number(payload.compiled.candidates_per_iteration),
      candidateOverrides: payload.compiled.candidate_overrides ?? [],
      overrides: payload.compiled.overrides ?? {}
    }
  };
}

export function toTemplateExecutionResult(payload) {
  return {
    executionId: String(payload.execution_id),
    templateId: String(payload.template.id),
    executionKind: String(payload.template.execution_kind),
    status: String(payload.status),
    goal: String(payload.goal)
  };
}

export function toExecutionDetail(payload) {
  return {
    execution: {
      executionId: String(payload.execution.execution_id),
      goal: String(payload.execution.goal),
      status: String(payload.execution.status)
    },
    progress: payload.progress ?? {},
    result: {
      bestCandidateId:
        payload.result?.best_candidate_id == null
          ? null
          : String(payload.result.best_candidate_id),
      completedIterations: Number(payload.result?.completed_iterations ?? 0),
      totalCandidateFailures: Number(payload.result?.total_candidate_failures ?? 0)
    },
    candidates: payload.candidates ?? []
  };
}
