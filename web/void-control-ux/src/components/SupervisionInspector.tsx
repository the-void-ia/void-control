import type { ExecutionEvent, SupervisionExecutionSummary, SupervisionWorkerCard } from '../lib/types';

interface SupervisionInspectorProps {
  summary: SupervisionExecutionSummary;
  worker: SupervisionWorkerCard | null;
  events: ExecutionEvent[];
  onOpenRuntime: (runtimeRunId: string) => void;
}

function formatReviewStatus(value?: string | null): string {
  if (!value) return 'pending review';
  return value.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase();
}

export function SupervisionInspector({
  summary,
  worker,
  events,
  onOpenRuntime
}: SupervisionInspectorProps) {
  if (!worker) {
    return (
      <aside className="inspector-panel swarm-inspector">
        <div className="inspector-head">
          <div className="inspector-node-name">
            <strong>Supervision Inspector</strong>
          </div>
        </div>
        <div className="empty">No worker selected.</div>
      </aside>
    );
  }

  return (
    <aside className="inspector-panel swarm-inspector">
      <div className="inspector-head">
        <div className="inspector-node-name">
          <strong>{worker.workerId}</strong>
          <span className={`inspector-badge state-${worker.state}`}>{formatReviewStatus(worker.reviewStatus)}</span>
        </div>
      </div>

      <section className="inspector-section swarm-inspector-hero">
        <div className="swarm-inspector-hero-top">
          <div>
            <div className="inspector-section-title">Selected Worker</div>
            <div className="swarm-inspector-hero-name">{worker.workerId}</div>
            <div className="swarm-inspector-hero-copy">{summary.goal}</div>
          </div>
          <div className="swarm-inspector-hero-status">
            <span className={`inspector-badge state-${worker.state}`}>{formatReviewStatus(worker.reviewStatus)}</span>
          </div>
        </div>
        <div className="swarm-inspector-hero-metrics">
          <div className="swarm-inspector-metric">
            <span>Latency p99</span>
            <strong>{worker.metrics.latency ?? '-'}</strong>
          </div>
          <div className="swarm-inspector-metric">
            <span>Error rate</span>
            <strong>{worker.metrics.errorRate ?? '-'}</strong>
          </div>
          <div className="swarm-inspector-metric">
            <span>CPU</span>
            <strong>{worker.metrics.cpu ?? '-'}</strong>
          </div>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Supervisor</div>
        <div className="kv">
          <span>Role</span>
          <strong>{summary.supervisorRole}</strong>
        </div>
        <div className="kv">
          <span>Execution</span>
          <strong>{summary.executionId}</strong>
        </div>
        <div className="kv">
          <span>Status</span>
          <strong>{summary.status}</strong>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Review State</div>
        <div className="kv">
          <span>Worker role</span>
          <strong>{worker.role ?? '-'}</strong>
        </div>
        <div className="kv">
          <span>Review</span>
          <strong>{formatReviewStatus(worker.reviewStatus)}</strong>
        </div>
        <div className="kv">
          <span>Revision round</span>
          <strong>{worker.revisionRound}</strong>
        </div>
        {worker.reason && <div className="inspector-note">{worker.reason}</div>}
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Outcome</div>
        <div className="kv">
          <span>Approved workers</span>
          <strong>{summary.counts.approved}</strong>
        </div>
        <div className="kv">
          <span>Revisions requested</span>
          <strong>{summary.counts.revisionRequested + summary.counts.retryRequested}</strong>
        </div>
        <div className="kv">
          <span>Rejected workers</span>
          <strong>{summary.counts.rejected}</strong>
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Recent Events</div>
        <div className="decision-list">
          {events.slice(-6).reverse().map((event) => (
            <div key={`${event.seq}-${event.event_type}`} className="decision-row">
              <span>#{event.seq}</span>
              <span>{event.event_type.replace(/([a-z0-9])([A-Z])/g, '$1 $2')}</span>
            </div>
          ))}
          {events.length === 0 && <div className="inspector-note">No supervision events yet.</div>}
        </div>
      </section>

      <section className="inspector-section">
        <div className="inspector-section-title">Runtime</div>
        {worker.runtimeRunId ? (
          <button
            type="button"
            className="runtime-jump-btn swarm-runtime-link"
            onClick={() => onOpenRuntime(worker.runtimeRunId as string)}
          >
            <strong>Open Runtime Graph</strong>
            <span>{worker.runtimeRunId}</span>
          </button>
        ) : (
          <div className="inspector-note">No runtime handle yet.</div>
        )}
      </section>
    </aside>
  );
}
