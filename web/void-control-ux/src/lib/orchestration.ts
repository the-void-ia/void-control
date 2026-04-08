import type {
  ExecutionCandidate,
  ExecutionDetailResponse,
  SwarmCandidateCard,
  SwarmExecutionSummary,
  SwarmHealthChip,
  SwarmIterationSummary
} from './types';

function formatMetricValue(value: number): string {
  if (!Number.isFinite(value)) return '-';
  if (Math.abs(value) >= 100) return value.toFixed(0);
  if (Math.abs(value) >= 10) return value.toFixed(1);
  return value.toFixed(3);
}

function deriveHealthChips(detail: ExecutionDetailResponse): SwarmHealthChip[] {
  const chips: SwarmHealthChip[] = [];
  const progress = detail.progress;
  const result = detail.result;

  if ((progress?.failed_candidate_count ?? 0) > 0 || (result?.total_candidate_failures ?? 0) > 0) {
    chips.push({ label: 'Partial Failures', tone: 'warn' });
  }
  if ((progress?.running_candidate_count ?? 0) > 0 && (progress?.candidate_output_count ?? 0) === 0) {
    chips.push({ label: 'Awaiting Outputs', tone: 'neutral' });
  }
  if (detail.execution.result_best_candidate_id || detail.result?.best_candidate_id) {
    chips.push({ label: 'Winner Selected', tone: 'good' });
  }
  if (chips.length === 0) {
    chips.push({ label: 'Healthy', tone: 'good' });
  }
  return chips;
}

function deriveCandidateState(
  candidate: ExecutionCandidate,
  bestCandidateId?: string | null
): SwarmCandidateCard['state'] {
  if (bestCandidateId && candidate.candidate_id === bestCandidateId) return 'best';
  if (candidate.status === 'Failed') return 'failed';
  if (candidate.status === 'Canceled') return 'canceled';
  if (candidate.status === 'Running') return 'running';
  if (candidate.status === 'Queued') return 'queued';
  if (candidate.status === 'Completed') {
    return Object.keys(candidate.metrics ?? {}).length > 0 ? 'scored' : 'output_ready';
  }
  return 'rejected';
}

function extractMetric(candidate: ExecutionCandidate, key: string): string | null {
  const value = candidate.metrics?.[key];
  return typeof value === 'number' ? formatMetricValue(value) : null;
}

export function deriveSwarmExecutionSummary(detail: ExecutionDetailResponse): SwarmExecutionSummary {
  const candidates = detail.candidates ?? [];
  const bestCandidateId = detail.result?.best_candidate_id ?? detail.execution.result_best_candidate_id ?? null;
  const counts = {
    queued: candidates.filter((candidate) => candidate.status === 'Queued').length,
    running: candidates.filter((candidate) => candidate.status === 'Running').length,
    outputReady: candidates.filter((candidate) => candidate.status === 'Completed').length,
    scored: candidates.filter((candidate) => candidate.status === 'Completed' && Object.keys(candidate.metrics ?? {}).length > 0).length,
    failed: candidates.filter((candidate) => candidate.status === 'Failed').length,
    completed: candidates.filter((candidate) => candidate.status === 'Completed').length
  };

  return {
    executionId: detail.execution.execution_id,
    mode: detail.execution.mode,
    goal: detail.execution.goal,
    status: detail.execution.status,
    completedIterations: detail.result?.completed_iterations ?? detail.execution.completed_iterations ?? 0,
    currentIterationLabel: Math.max(
      1,
      (detail.result?.completed_iterations ?? detail.execution.completed_iterations ?? 0) + 1
    ),
    bestCandidateId,
    counts,
    healthChips: deriveHealthChips(detail)
  };
}

export function deriveIterationSummaries(detail: ExecutionDetailResponse): SwarmIterationSummary[] {
  const byIteration = new Map<number, ExecutionCandidate[]>();
  for (const candidate of detail.candidates ?? []) {
    const group = byIteration.get(candidate.iteration) ?? [];
    group.push(candidate);
    byIteration.set(candidate.iteration, group);
  }

  return [...byIteration.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([iterationIndex, candidates]) => ({
      iterationIndex,
      iterationLabel: iterationIndex + 1,
      candidateCount: candidates.length,
      queued: candidates.filter((candidate) => candidate.status === 'Queued').length,
      running: candidates.filter((candidate) => candidate.status === 'Running').length,
      outputReady: candidates.filter((candidate) => candidate.status === 'Completed').length,
      scored: candidates.filter((candidate) => candidate.status === 'Completed' && Object.keys(candidate.metrics ?? {}).length > 0).length,
      failed: candidates.filter((candidate) => candidate.status === 'Failed').length,
      completed: candidates.filter((candidate) => candidate.status === 'Completed').length,
      bestCandidateId: detail.result?.best_candidate_id ?? detail.execution.result_best_candidate_id ?? null
    }));
}

export function deriveCandidateCards(
  detail: ExecutionDetailResponse,
  iterationIndex: number
): SwarmCandidateCard[] {
  const bestCandidateId = detail.result?.best_candidate_id ?? detail.execution.result_best_candidate_id ?? null;

  return (detail.candidates ?? [])
    .filter((candidate) => candidate.iteration === iterationIndex)
    .map((candidate) => ({
      candidateId: candidate.candidate_id,
      iterationIndex: candidate.iteration,
      iterationLabel: candidate.iteration + 1,
      runtimeRunId: candidate.runtime_run_id ?? null,
      state: deriveCandidateState(candidate, bestCandidateId),
      metrics: {
        latency: extractMetric(candidate, 'latency_p99_ms'),
        errorRate: extractMetric(candidate, 'error_rate'),
        cpu: extractMetric(candidate, 'cpu_pct')
      },
      reason:
        candidate.status === 'Failed'
          ? 'Failed before scoring'
          : candidate.status === 'Canceled'
            ? 'Canceled before completion'
            : Object.keys(candidate.metrics ?? {}).length === 0 && candidate.status === 'Completed'
              ? 'Output collected, score pending'
              : null
    }))
    .sort((a, b) => {
      const rank = (state: SwarmCandidateCard['state']) => {
        switch (state) {
          case 'best':
            return 0;
          case 'running':
            return 1;
          case 'scored':
            return 2;
          case 'output_ready':
            return 3;
          case 'queued':
            return 4;
          case 'rejected':
            return 5;
          case 'failed':
          case 'canceled':
            return 6;
          default:
            return 7;
        }
      };
      const diff = rank(a.state) - rank(b.state);
      if (diff !== 0) return diff;
      return a.candidateId.localeCompare(b.candidateId);
    });
}
