import { useMemo } from 'react';
import type { RunEvent, StageView, TelemetrySample } from '../lib/types';
import {
  filterEventsForStage,
  indexTelemetryByStage,
  latestTelemetryByStage,
  parseNodeId,
  resolveSelectedStage,
  type SelectedNodeType
} from '../lib/selectors';

interface NodeInspectorProps {
  runId: string;
  selectedNodeId: string | null;
  selectedNodeType: SelectedNodeType;
  isPinned: boolean;
  stages: StageView[];
  events: RunEvent[];
  telemetry: TelemetrySample[];
  onClearSelection: () => void;
  onTogglePinned: () => void;
}

function fmtTime(ts?: string | null): string {
  if (!ts) return '-';
  const d = new Date(ts);
  return Number.isNaN(d.getTime()) ? ts : d.toLocaleTimeString();
}

function fmtMs(v?: number | null): string {
  if (typeof v !== 'number') return '-';
  if (v < 1000) return `${v} ms`;
  return `${(v / 1000).toFixed(2)} s`;
}

function fmtMb(v?: number): string {
  if (typeof v !== 'number') return '-';
  return `${(v / (1024 * 1024)).toFixed(1)} MB`;
}

function fmtGuestMemUsed(sample: TelemetrySample | null | undefined): string {
  if (!sample?.guest) return '-';
  const used = sample.guest.memory_used_bytes;
  const total = sample.guest.memory_total_bytes;
  if (typeof used !== 'number') return '-';
  if (typeof total === 'number' && total > 0) {
    const pct = (used / total) * 100;
    return `${fmtMb(used)} (${pct.toFixed(0)}%)`;
  }
  return fmtMb(used);
}

function hasGuestMetrics(sample: TelemetrySample | null | undefined): boolean {
  if (!sample?.guest) return false;
  return (
    typeof sample.guest.cpu_percent === 'number' ||
    typeof sample.guest.memory_used_bytes === 'number' ||
    typeof sample.guest.memory_total_bytes === 'number'
  );
}

export function NodeInspector({
  runId,
  selectedNodeId,
  selectedNodeType,
  isPinned,
  stages,
  events,
  telemetry,
  onClearSelection,
  onTogglePinned
}: NodeInspectorProps) {
  const parsed = parseNodeId(selectedNodeId);
  const selectedStage = resolveSelectedStage(selectedNodeId, stages);

  const telemetryByStage = useMemo(() => indexTelemetryByStage(telemetry), [telemetry]);
  const latestByStage = useMemo(() => latestTelemetryByStage(telemetry), [telemetry]);

  const stageTelemetry = selectedStage ? (telemetryByStage.get(selectedStage.stage_name) ?? []) : [];
  const latestSample = selectedStage ? latestByStage.get(selectedStage.stage_name) : null;
  const latestGuestStageSample = selectedStage
    ? [...stageTelemetry].reverse().find((s) => hasGuestMetrics(s)) ?? null
    : null;
  const stageEvents = useMemo(() => filterEventsForStage(events, selectedStage), [events, selectedStage]);

  const eventRef = parsed?.type === 'event' ? parsed.eventRef : null;
  const selectedEvent =
    parsed?.type === 'event'
      ? events.find((e, i) => e.event_id === eventRef || `${e.seq}-${i}` === eventRef) ?? null
      : null;

  const nodeMissing = Boolean(parsed && parsed.runId === runId && parsed.type === 'stage' && !selectedStage);

  const header = (() => {
    if (!parsed) return 'No node selected';
    if (parsed.type === 'run') return `Run ${parsed.runId}`;
    if (parsed.type === 'event') return selectedEvent ? (selectedEvent.event_type_v2 ?? selectedEvent.event_type) : `Event ${parsed.eventRef}`;
    if (parsed.type === 'stage') return selectedStage?.stage_name ?? parsed.stageName;
    return 'Node';
  })();

  const status = (() => {
    if (nodeMissing) return 'terminated';
    if (selectedStage) return selectedStage.status;
    if (selectedEvent) return selectedEvent.level ?? 'info';
    if (parsed?.type === 'run') return 'run';
    return 'unknown';
  })();

  const tinySeries = stageTelemetry
    .filter((s) => typeof s.guest?.cpu_percent === 'number')
    .slice(-20);
  const maxCpu = Math.max(1, ...tinySeries.map((s) => s.guest?.cpu_percent ?? 0));
  const latestTelemetry = telemetry.length > 0 ? telemetry[telemetry.length - 1] : null;
  const latestGuestRunSample = [...telemetry].reverse().find((s) => hasGuestMetrics(s)) ?? null;
  const firstEventTs = events.length > 0 ? events[0]?.timestamp : null;
  const lastEventTs = events.length > 0 ? events[events.length - 1]?.timestamp : null;

  return (
    <aside className="inspector-panel">
      <div className="inspector-head">
        <div>
          <div className="panel-title">Node Inspector</div>
          <div className="inspector-node-name">
            {header} <span className={`inspector-badge status-${status}`}>{status}</span>
          </div>
        </div>
        <div className="inspector-actions">
          <button className="inspector-btn" onClick={onTogglePinned}>{isPinned ? 'Unpin' : 'Pin'}</button>
          <button className="inspector-btn" onClick={onClearSelection}>Clear</button>
        </div>
      </div>

      <section className="inspector-section">
        <div className="inspector-section-title">Overview</div>
        <div className="kv"><span>Type</span><strong>{selectedNodeType ?? 'none'}</strong></div>
        <div className="kv"><span>Status</span><strong className={`status-${status}`}>{status}</strong></div>
        {nodeMissing && <div className="inspector-note">Terminated / No longer present in latest snapshot.</div>}
      </section>

      {selectedStage && (
        <>
          <section className="inspector-section">
            <div className="inspector-section-title">State & Timing</div>
            <div className="kv"><span>Attempt</span><strong>{selectedStage.stage_attempt}</strong></div>
            <div className="kv"><span>Started</span><strong>{fmtTime(selectedStage.started_at)}</strong></div>
            <div className="kv"><span>Completed</span><strong>{fmtTime(selectedStage.completed_at)}</strong></div>
            <div className="kv"><span>Total Duration</span><strong>{fmtMs(selectedStage.duration_ms)}</strong></div>
            <div className="kv"><span>Exit Code</span><strong>{selectedStage.exit_code ?? '-'}</strong></div>
          </section>

          <section className="inspector-section">
            <div className="inspector-section-title">Dependencies</div>
            {selectedStage.depends_on.length === 0 ? (
              <div className="inspector-note">No upstream dependencies.</div>
            ) : (
              <div className="pill-row">
                {selectedStage.depends_on.map((dep) => <span key={dep} className="stage-chip">{dep}</span>)}
              </div>
            )}
          </section>

          <section className="inspector-section">
            <div className="inspector-section-title">Metrics</div>
            {latestGuestStageSample ? (
              <>
                <div className="metrics-grid">
                  <div className="metric-box"><span>Guest CPU</span><strong>{(latestGuestStageSample.guest?.cpu_percent ?? 0).toFixed(1)}%</strong></div>
                  <div className="metric-box"><span>Guest Mem</span><strong>{fmtGuestMemUsed(latestGuestStageSample)}</strong></div>
                </div>
              </>
            ) : (
              <div className="inspector-note">No guest telemetry samples for this node yet.</div>
            )}
            {tinySeries.length > 0 && (
              <div className="sparkline">
                {tinySeries.map((s) => {
                  const h = Math.max(4, Math.round(((s.guest?.cpu_percent ?? 0) / maxCpu) * 46));
                  return <span key={s.seq} style={{ height: `${h}px` }} />;
                })}
              </div>
            )}
            {latestSample && !latestGuestStageSample && (
              <div className="inspector-note">Daemon host telemetry is available, but guest metrics were not reported for this stage.</div>
            )}
            {latestSample?.host && (
              <div className="metrics-subgrid">
                <div className="metric-subbox"><span>Host CPU</span><strong>{(latestSample.host.cpu_percent ?? 0).toFixed(1)}%</strong></div>
                <div className="metric-subbox"><span>Host RSS</span><strong>{fmtMb(latestSample.host.rss_bytes)}</strong></div>
              </div>
            )}
          </section>

          <section className="inspector-section">
            <div className="inspector-section-title">Recent Events</div>
            <div className="inspector-events">
              {stageEvents.slice(-8).reverse().map((event) => (
                <div className="inspector-event-row" key={event.event_id || `${event.seq}`}>
                  <span>#{event.seq}</span>
                  <span>{event.event_type_v2 ?? event.event_type}</span>
                </div>
              ))}
              {stageEvents.length === 0 && <div className="inspector-note">No stage-scoped events yet.</div>}
            </div>
          </section>
        </>
      )}

      {parsed?.type === 'event' && selectedEvent && (
        <section className="inspector-section">
          <div className="inspector-section-title">Event Details</div>
          <div className="kv"><span>Seq</span><strong>#{selectedEvent.seq}</strong></div>
          <div className="kv"><span>Timestamp</span><strong>{fmtTime(selectedEvent.timestamp)}</strong></div>
          <div className="inspector-note">{selectedEvent.message ?? 'No message'}</div>
        </section>
      )}

      {parsed?.type === 'run' && (
        <>
          <section className="inspector-section">
            <div className="inspector-section-title">State & Timing</div>
            <div className="kv"><span>Events</span><strong>{events.length}</strong></div>
            <div className="kv"><span>Stages</span><strong>{stages.length}</strong></div>
            <div className="kv"><span>Started</span><strong>{fmtTime(firstEventTs)}</strong></div>
            <div className="kv"><span>Updated</span><strong>{fmtTime(lastEventTs)}</strong></div>
          </section>

          <section className="inspector-section">
            <div className="inspector-section-title">Metrics</div>
            {latestGuestRunSample ? (
              <>
                <div className="metrics-grid">
                  <div className="metric-box"><span>Guest CPU</span><strong>{(latestGuestRunSample.guest?.cpu_percent ?? 0).toFixed(1)}%</strong></div>
                  <div className="metric-box"><span>Guest Mem</span><strong>{fmtGuestMemUsed(latestGuestRunSample)}</strong></div>
                </div>
              </>
            ) : (
              <div className="inspector-note">No guest telemetry samples for this run yet.</div>
            )}
            {latestTelemetry && !latestGuestRunSample && (
              <div className="inspector-note">Daemon host telemetry is available, but guest metrics were not reported.</div>
            )}
            {latestTelemetry?.host && (
              <div className="metrics-subgrid">
                <div className="metric-subbox"><span>Host CPU</span><strong>{(latestTelemetry.host.cpu_percent ?? 0).toFixed(1)}%</strong></div>
                <div className="metric-subbox"><span>Host RSS</span><strong>{fmtMb(latestTelemetry.host.rss_bytes)}</strong></div>
              </div>
            )}
          </section>

          <section className="inspector-section">
            <div className="inspector-section-title">Recent Events</div>
            <div className="inspector-events">
              {events.slice(-8).reverse().map((event) => (
                <div className="inspector-event-row" key={event.event_id || `${event.seq}`}>
                  <span>#{event.seq}</span>
                  <span>{event.event_type_v2 ?? event.event_type}</span>
                </div>
              ))}
              {events.length === 0 && <div className="inspector-note">No events yet.</div>}
            </div>
          </section>
        </>
      )}
    </aside>
  );
}
