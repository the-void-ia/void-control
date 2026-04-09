import type { SupervisionExecutionSummary, SupervisionWorkerCard, SwarmCandidateCard, SwarmExecutionSummary, SwarmIterationSummary } from '../lib/types';
import { SwarmGraph } from './SwarmGraph';

interface SupervisionGraphProps {
  summary: SupervisionExecutionSummary;
  workers: SupervisionWorkerCard[];
  selectedWorkerId: string | null;
  onSelectWorker: (workerId: string) => void;
}

function mapWorkerState(state: SupervisionWorkerCard['state']): SwarmCandidateCard['state'] {
  switch (state) {
    case 'approved':
      return 'best';
    case 'revision_requested':
    case 'retry_requested':
      return 'output_ready';
    case 'rejected':
      return 'rejected';
    case 'failed':
      return 'failed';
    case 'canceled':
      return 'canceled';
    case 'running':
      return 'running';
    case 'queued':
    default:
      return 'queued';
  }
}

export function SupervisionGraph({
  summary,
  workers,
  selectedWorkerId,
  onSelectWorker
}: SupervisionGraphProps) {
  const mappedSummary: SwarmExecutionSummary = {
    executionId: summary.executionId,
    mode: summary.mode,
    goal: summary.goal,
    status: summary.status,
    completedIterations: summary.completedIterations,
    currentIterationLabel: Math.max(1, summary.completedIterations + 1),
    bestCandidateId: summary.approvedWorkerId ?? null,
    counts: {
      queued: summary.counts.queued,
      running: summary.counts.running,
      outputReady: summary.counts.revisionRequested + summary.counts.retryRequested,
      scored: summary.counts.approved,
      failed: summary.counts.failed + summary.counts.rejected,
      completed: summary.counts.completed
    },
    healthChips: summary.healthChips
  };

  const iteration: SwarmIterationSummary = {
    iterationIndex: workers[0]?.iterationIndex ?? 0,
    iterationLabel: workers[0]?.iterationLabel ?? 1,
    candidateCount: workers.length,
    queued: summary.counts.queued,
    running: summary.counts.running,
    outputReady: summary.counts.revisionRequested + summary.counts.retryRequested,
    scored: summary.counts.approved,
    failed: summary.counts.failed + summary.counts.rejected,
    completed: summary.counts.completed,
    bestCandidateId: summary.approvedWorkerId ?? null
  };

  const candidates: SwarmCandidateCard[] = workers.map((worker) => ({
    candidateId: worker.workerId,
    iterationIndex: worker.iterationIndex,
    iterationLabel: worker.iterationLabel,
    runtimeRunId: worker.runtimeRunId ?? null,
    state: mapWorkerState(worker.state),
    metrics: worker.metrics,
    reason:
      worker.reviewStatus
        ? `${worker.reviewStatus.replace(/([a-z0-9])([A-Z])/g, '$1 $2')} · round ${worker.revisionRound}`
        : worker.reason ?? null
  }));

  return (
    <SwarmGraph
      summary={mappedSummary}
      iterations={[iteration]}
      candidates={candidates}
      selectedCandidateId={selectedWorkerId}
      onSelectCandidate={onSelectWorker}
    />
  );
}
