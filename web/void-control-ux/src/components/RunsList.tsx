import type { ExecutionInspection, RunInspection } from '../lib/types';
import { getRunId } from '../lib/api';

interface RunsListProps {
  executions: ExecutionInspection[];
  activeRuns: RunInspection[];
  terminalRuns: RunInspection[];
  runtimeUnavailable: boolean;
  selectedId: string | null;
  selectedKind: 'execution' | 'run' | null;
  onSelectExecution: (executionId: string) => void;
  onSelectRun: (runId: string) => void;
  onLaunch: () => void;
  hideTestRuns: boolean;
  onToggleHideTestRuns: () => void;
  stateFilter: 'all' | 'running' | 'failed' | 'succeeded' | 'cancelled';
  onStateFilterChange: (state: 'all' | 'running' | 'failed' | 'succeeded' | 'cancelled') => void;
}

function stateOf(run: RunInspection): string {
  return (run.status ?? run.state ?? 'unknown').toString();
}

export function RunsList({
  executions,
  activeRuns,
  terminalRuns,
  runtimeUnavailable,
  selectedId,
  selectedKind,
  onSelectExecution,
  onSelectRun,
  onLaunch,
  hideTestRuns,
  onToggleHideTestRuns,
  stateFilter,
  onStateFilterChange
}: RunsListProps) {
  const all = [...activeRuns, ...terminalRuns];
  const items: Array<
    | { kind: 'execution'; id: string; execution: ExecutionInspection }
    | { kind: 'run'; id: string; run: RunInspection }
  > = [
    ...executions.map((execution) => ({
      kind: 'execution' as const,
      id: execution.execution_id,
      execution
    })),
    ...all.map((run) => ({
      kind: 'run' as const,
      id: getRunId(run),
      run
    }))
  ];
  const statePills: Array<{ value: 'all' | 'running' | 'failed' | 'succeeded' | 'cancelled'; label: string }> = [
    { value: 'all', label: 'All' },
    { value: 'running', label: 'Running' },
    { value: 'failed', label: 'Failed' },
    { value: 'succeeded', label: 'Succeeded' },
    { value: 'cancelled', label: 'Cancelled' }
  ];
  return (
    <aside className="runs-panel">
      <div className="runs-head">
        <h2>Executions</h2>
        <button className="runs-filter-btn" onClick={onToggleHideTestRuns}>
          {hideTestRuns ? 'Show Tests' : 'Hide Tests'}
        </button>
      </div>
      <button className="launch-box-btn" type="button" onClick={onLaunch}>+ Launch Box</button>
      <div className="runs-state-pills" role="tablist" aria-label="Filter runs by state">
        {statePills.map((pill) => (
          <button
            key={pill.value}
            type="button"
            className={`runs-state-pill ${stateFilter === pill.value ? 'active' : ''}`}
            aria-pressed={stateFilter === pill.value}
            onClick={() => onStateFilterChange(pill.value)}
          >
            {pill.label}
          </button>
        ))}
      </div>
      <div className="runs-meta">
        active {activeRuns.length} | terminal {terminalRuns.length}
        {runtimeUnavailable && <span className="runs-warning">runtime unavailable</span>}
      </div>
      <ul className="run-list">
        {items.map((item) => {
          if (item.kind === 'execution') {
            const execution = item.execution;
            const selected = selectedKind === 'execution' && execution.execution_id === selectedId;
            return (
              <li key={execution.execution_id}>
                <button
                  className={`run-item execution-item ${selected ? 'selected' : ''}`}
                  onClick={() => onSelectExecution(execution.execution_id)}
                >
                  <div className="run-item-top">
                    <div className="run-id">{execution.execution_id}</div>
                    <div className={`run-state state-${execution.status.toLowerCase()}`}>{execution.status}</div>
                  </div>
                  <div className="run-item-sub">swarm • {execution.mode}</div>
                  <div className="run-item-sub clamp-2">{execution.goal}</div>
                  <div className="run-item-sub">
                    iter {execution.completed_iterations ?? 0} • best {execution.result_best_candidate_id ?? '-'}
                  </div>
                </button>
              </li>
            );
          }

          const run = item.run;
          const runId = getRunId(run);
          const selected = selectedKind === 'run' && runId === selectedId;
          const state = stateOf(run);
          return (
            <li key={runId}>
              <button
                className={`run-item ${selected ? 'selected' : ''} ${state === 'running' ? 'is-running' : ''}`}
                onClick={() => onSelectRun(runId)}
              >
                <div className="run-item-top">
                  <div className="run-id">{runId}</div>
                  <div className={`run-state state-${state}`}>{state}</div>
                </div>
                <div className="run-item-sub">runtime</div>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
