import type { SwarmCandidateCard, SwarmExecutionSummary } from '../lib/types';

interface SwarmStrategyPanelProps {
  summary: SwarmExecutionSummary;
  candidates: SwarmCandidateCard[];
}

function humanState(state: SwarmCandidateCard['state']): string {
  return state.replace(/_/g, ' ');
}

export function SwarmStrategyPanel({ summary, candidates }: SwarmStrategyPanelProps) {
  const winner = candidates.find((candidate) => candidate.state === 'best') ?? null;
  const active = candidates.filter((candidate) => candidate.state === 'running' || candidate.state === 'queued');
  const failed = candidates.filter((candidate) => candidate.state === 'failed' || candidate.state === 'canceled');
  const completed = candidates.filter(
    (candidate) => candidate.state === 'best' || candidate.state === 'scored' || candidate.state === 'output_ready'
  );

  return (
    <aside className="swarm-strategy-panel">
      <div className="panel-title">Decision</div>

      <section className="decision-block winner-block">
        <div className="decision-label">Winner</div>
        {winner ? (
          <>
            <strong>{winner.candidateId}</strong>
            <div className="decision-copy">
              Selected as current best candidate for this execution.
            </div>
            <div className="decision-metrics">
              <span>latency {winner.metrics.latency ?? '-'}</span>
              <span>error {winner.metrics.errorRate ?? '-'}</span>
              <span>cpu {winner.metrics.cpu ?? '-'}</span>
            </div>
          </>
        ) : (
          <div className="decision-copy">No winner selected yet.</div>
        )}
      </section>

      <section className="decision-block">
        <div className="decision-label">Iteration Outcome</div>
        <div className="decision-stats">
          <span>completed {completed.length}</span>
          <span>active {active.length}</span>
          <span>failed {failed.length}</span>
        </div>
        <div className="decision-copy">
          {active.length > 0
            ? 'This iteration is still collecting outputs or waiting for candidate completion.'
            : failed.length > 0
              ? 'This iteration completed with mixed outcomes.'
              : 'This iteration completed without candidate failures.'}
        </div>
      </section>

      <section className="decision-block">
        <div className="decision-label">Candidate States</div>
        <div className="decision-list">
          {candidates.map((candidate) => (
            <div key={candidate.candidateId} className="decision-row">
              <span>{candidate.candidateId}</span>
              <span>{humanState(candidate.state)}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="decision-block">
        <div className="decision-label">Execution Health</div>
        <div className="decision-chip-list">
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
