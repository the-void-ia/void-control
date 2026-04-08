import type { ExecutionEvent, SwarmCandidateCard, SwarmExecutionSummary, SwarmIterationSummary } from '../lib/types';

interface SwarmInspectorProps {
  summary: SwarmExecutionSummary;
  iteration: SwarmIterationSummary | null;
  candidate: SwarmCandidateCard | null;
  events: ExecutionEvent[];
  onOpenRuntime: (runtimeRunId: string) => void;
}

function formatStateLabel(state: SwarmCandidateCard['state']): string {
  return state.replace(/_/g, ' ');
}

export function SwarmInspector({
  summary,
  iteration,
  candidate,
  events,
  onOpenRuntime
}: SwarmInspectorProps) {
  if (!candidate) {
    return (
      <aside className="inspector-panel swarm-inspector">
        <div className="inspector-head">
          <div className="inspector-node-name">
            <strong>Swarm Inspector</strong>
          </div>
        </div>
        <div className="empty">No candidate selected.</div>
      </aside>
    );
  }

  return (
    <aside className="inspector-panel swarm-inspector">
      <div className="inspector-head">
        <div className="inspector-node-name">
          <strong>{candidate.candidateId}</strong>
          <span className={`inspector-badge state-${candidate.state}`}>{formatStateLabel(candidate.state)}</span>
        </div>
      </div>

      <section className="inspector-section swarm-inspector-hero">
        <div className="swarm-inspector-hero-top">
          <div>
            <div className="inspector-section-title">Selected Candidate</div>
            <div className="swarm-inspector-hero-name">{candidate.candidateId}</div>
            <div className="swarm-inspector-hero-copy">
              {summary.goal}
            </div>
          </div>
          <div className="swarm-inspector-hero-status">
            <span className={`inspector-badge state-${candidate.state}`}>{formatStateLabel(candidate.state)}</span>
          </div>
        </div>
        <div className="swarm-inspector-hero-metrics">
          <div className="swarm-inspector-metric">
            <span>Latency p99</span>
            <strong>{candidate.metrics.latency ?? '-'}</strong>
          </div>
          <div className="swarm-inspector-metric">
            <span>Error rate</span>
            <strong>{candidate.metrics.errorRate ?? '-'}</strong>
          </div>
          <div className="swarm-inspector-metric">
            <span>CPU</span>
            <strong>{candidate.metrics.cpu ?? '-'}</strong>
          </div>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Selection</div>
        <div className="kv">
          <span>Execution</span>
          <strong>{summary.executionId}</strong>
        </div>
        <div className="kv">
          <span>Iteration</span>
          <strong>{candidate.iterationLabel}</strong>
        </div>
        <div className="kv">
          <span>Mode</span>
          <strong>{summary.mode}</strong>
        </div>
        {candidate.reason && <div className="inspector-note">{candidate.reason}</div>}
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Metrics</div>
        <div className="kv">
          <span>Error rate</span>
          <strong>{candidate.metrics.errorRate ?? '-'}</strong>
        </div>
        <div className="kv">
          <span>CPU</span>
          <strong>{candidate.metrics.cpu ?? '-'}</strong>
        </div>
        <div className="kv">
          <span>Runtime run</span>
          <strong>{candidate.runtimeRunId ?? '-'}</strong>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Decision</div>
        <div className="kv">
          <span>Best candidate</span>
          <strong>{summary.bestCandidateId ?? '-'}</strong>
        </div>
        <div className="kv">
          <span>Execution status</span>
          <strong>{summary.status}</strong>
        </div>
        <div className="kv">
          <span>Completed iterations</span>
          <strong>{summary.completedIterations}</strong>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Recent Events</div>
        <div className="decision-list">
          {events.slice(-4).reverse().map((event) => (
            <div key={`${event.seq}-${event.event_type}`} className="decision-row">
              <span>#{event.seq}</span>
              <span>{event.event_type.replace(/([a-z0-9])([A-Z])/g, '$1 $2')}</span>
            </div>
          ))}
          {events.length === 0 && <div className="inspector-note">No orchestration events yet.</div>}
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Iteration Outcome</div>
        <div className="kv">
          <span>Candidates</span>
          <strong>{iteration?.candidateCount ?? 0}</strong>
        </div>
        <div className="kv">
          <span>Running</span>
          <strong>{iteration?.running ?? 0}</strong>
        </div>
        <div className="kv">
          <span>Scored</span>
          <strong>{iteration?.scored ?? 0}</strong>
        </div>
        <div className="kv">
          <span>Failed</span>
          <strong>{iteration?.failed ?? 0}</strong>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Runtime</div>
        {candidate.runtimeRunId ? (
          <button
            type="button"
            className="runtime-jump-btn swarm-runtime-link"
            onClick={() => onOpenRuntime(candidate.runtimeRunId as string)}
          >
            <strong>Open Runtime Graph</strong>
            <span>{candidate.runtimeRunId}</span>
          </button>
        ) : (
          <div className="inspector-note">No runtime handle yet.</div>
        )}
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Health</div>
        <div className="pill-row">
          {summary.healthChips.map((chip) => (
            <span key={chip.label} className={`swarm-chip tone-${chip.tone}`}>
              {chip.label}
            </span>
          ))}
        </div>
      </section>
    </aside>
  );
}
