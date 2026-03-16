import type { RunInspection } from '../lib/types';
import { getRunId } from '../lib/api';

interface RunsListProps {
  activeRuns: RunInspection[];
  terminalRuns: RunInspection[];
  selectedRunId: string | null;
  onSelect: (runId: string) => void;
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
  activeRuns,
  terminalRuns,
  selectedRunId,
  onSelect,
  onLaunch,
  hideTestRuns,
  onToggleHideTestRuns,
  stateFilter,
  onStateFilterChange
}: RunsListProps) {
  const all = [...activeRuns, ...terminalRuns];
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
        <h2>Runs</h2>
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
      <div className="runs-meta">active {activeRuns.length} | terminal {terminalRuns.length}</div>
      <ul className="run-list">
        {all.map((run) => {
          const runId = getRunId(run);
          const selected = runId === selectedRunId;
          const state = stateOf(run);
          return (
            <li key={runId}>
              <button
                className={`run-item ${selected ? 'selected' : ''} ${state === 'running' ? 'is-running' : ''}`}
                onClick={() => onSelect(runId)}
              >
                <div className="run-id">{runId}</div>
                <div className={`run-state state-${state}`}>{state}</div>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
