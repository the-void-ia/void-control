import type { SwarmCandidateCard, SwarmExecutionSummary, SwarmIterationSummary } from '../lib/types';
import { SwarmStrategyPanel } from './SwarmStrategyPanel';

interface SwarmOverviewProps {
  summary: SwarmExecutionSummary;
  iterations: SwarmIterationSummary[];
  candidates: SwarmCandidateCard[];
  selectedIteration: number;
  onSelectIteration: (iterationIndex: number) => void;
}

function stateLabel(state: SwarmCandidateCard['state']): string {
  return state.replace(/_/g, ' ');
}

export function SwarmOverview({
  summary,
  iterations,
  candidates,
  selectedIteration,
  onSelectIteration
}: SwarmOverviewProps) {
  return (
    <div className="swarm-console">
      <section className="swarm-console-header">
        <div className="swarm-console-copy">
          <div className="eyebrow">Swarm Console</div>
          <h2>{summary.goal}</h2>
          <div className="swarm-console-meta">
            <span>{summary.executionId}</span>
            <span>{summary.status}</span>
            <span>current iteration {summary.currentIterationLabel}</span>
          </div>
        </div>
        <div className="swarm-console-side">
          {summary.bestCandidateId ? (
            <div className="summary-kpi">
              <span>Best Candidate</span>
              <strong>{summary.bestCandidateId}</strong>
            </div>
          ) : (
            <div className="summary-kpi">
              <span>Best Candidate</span>
              <strong>Pending</strong>
            </div>
          )}
          <div className="decision-chip-list">
            {summary.healthChips.map((chip) => (
              <span key={chip.label} className={`swarm-chip tone-${chip.tone}`}>
                {chip.label}
              </span>
            ))}
          </div>
        </div>
      </section>

      <section className="iteration-rail">
        {iterations.length === 0 ? (
          <div className="empty">No iterations planned yet.</div>
        ) : (
          iterations.map((iteration) => (
            <button
              key={iteration.iterationIndex}
              type="button"
              className={`iteration-pill ${selectedIteration === iteration.iterationIndex ? 'active' : ''}`}
              onClick={() => onSelectIteration(iteration.iterationIndex)}
            >
              <strong>Iteration {iteration.iterationLabel}</strong>
              <span>{iteration.candidateCount} candidates</span>
              <span>{iteration.running} running</span>
              <span>{iteration.scored} scored</span>
              <span>{iteration.failed} failed</span>
            </button>
          ))
        )}
      </section>

      <div className="swarm-workspace">
        <section className="candidate-matrix">
          <div className="panel-title-row">
            <div className="panel-title">Candidates</div>
            <div className="matrix-caption">Iteration {selectedIteration + 1}</div>
          </div>
          {candidates.length === 0 ? (
            <div className="empty">No candidates for this iteration.</div>
          ) : (
            <div className="matrix-table">
              <div className="matrix-head">
                <span>Candidate</span>
                <span>State</span>
                <span>Latency</span>
                <span>Error</span>
                <span>CPU</span>
                <span>Decision</span>
                <span>Runtime</span>
              </div>
              {candidates.map((candidate) => (
                <div
                  key={candidate.candidateId}
                  className={`matrix-row ${candidate.state === 'best' ? 'is-best' : ''}`}
                >
                  <span className="matrix-candidate">{candidate.candidateId}</span>
                  <span className={`state-badge state-${candidate.state}`}>{stateLabel(candidate.state)}</span>
                  <span>{candidate.metrics.latency ?? '-'}</span>
                  <span>{candidate.metrics.errorRate ?? '-'}</span>
                  <span>{candidate.metrics.cpu ?? '-'}</span>
                  <span>{candidate.reason ?? (candidate.state === 'best' ? 'selected' : candidate.state === 'scored' ? 'completed' : '-')}</span>
                  <span className="matrix-runtime">{candidate.runtimeRunId ?? '-'}</span>
                </div>
              ))}
            </div>
          )}
        </section>

        <SwarmStrategyPanel summary={summary} candidates={candidates} />
      </div>
    </div>
  );
}
