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
  onToggleHideTestRuns
}: RunsListProps) {
  const all = [...activeRuns, ...terminalRuns];
  return (
    <aside className="runs-panel">
      <div className="runs-head">
        <h2>Runs</h2>
        <button className="runs-filter-btn" onClick={onToggleHideTestRuns}>
          {hideTestRuns ? 'Show Tests' : 'Hide Tests'}
        </button>
      </div>
      <button className="launch-box-btn" type="button" onClick={onLaunch}>+ Launch Box</button>
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
