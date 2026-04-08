import { useEffect, useMemo, useRef, useState } from 'react';
import { getStageOutputFile } from '../lib/api';
import type { RunEvent, StageView, TelemetrySample } from '../lib/types';
import {
  filterEventsForStage,
  indexTelemetryByStage,
  parseNodeId,
  rollingEventsPerSec,
  rollingEventsPerSecSeries,
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

function fmtBytes(v?: number | null): string {
  if (typeof v !== 'number' || Number.isNaN(v) || v < 0) return '-';
  if (v < 1024) return `${v} B`;
  if (v < 1024 * 1024) return `${(v / 1024).toFixed(1)} KB`;
  return `${(v / (1024 * 1024)).toFixed(2)} MB`;
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

function hasHostMetrics(sample: TelemetrySample | null | undefined): boolean {
  if (!sample?.host) return false;
  return typeof sample.host.cpu_percent === 'number' || typeof sample.host.rss_bytes === 'number';
}

function buildSparklinePoints(series: number[], width: number, height: number): string {
  const values = series.length > 1 ? series : [0, ...(series.length === 1 ? [series[0]] : [0])];
  const maxValue = Math.max(0.1, ...values);
  const innerWidth = Math.max(1, width - 2);
  const innerHeight = Math.max(1, height - 4);

  return values
    .map((value, idx) => {
      const x = 1 + (idx / Math.max(1, values.length - 1)) * innerWidth;
      const y = 2 + (1 - value / maxValue) * innerHeight;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(' ');
}

function detectAbsolutePath(message: string): string | null {
  const match = message.match(/(?:^|[\s(`"'[])(((?:\/[\w.\-]+)+\/?[\w.\-]*))(?=$|[\s)`"',.:;\]])/);
  return match?.[1] ?? null;
}

function resolveEventStageName(event: RunEvent | null, allEvents: RunEvent[]): string | null {
  if (!event) return null;
  if (typeof event.stage_name === 'string' && event.stage_name.trim().length > 0) return event.stage_name;

  const payload = event.payload as Record<string, unknown> | null | undefined;
  const payloadStageName = payload?.stage_name;
  if (typeof payloadStageName === 'string' && payloadStageName.trim().length > 0) return payloadStageName;

  const index = allEvents.findIndex((candidate, i) => {
    if (event.event_id && candidate.event_id === event.event_id) return true;
    return `${candidate.seq}-${i}` === `${event.seq}-${i}`;
  });
  if (index === -1) return null;

  for (let i = index - 1; i >= 0; i -= 1) {
    const stageName = allEvents[i]?.stage_name;
    if (typeof stageName === 'string' && stageName.trim().length > 0) return stageName;
  }

  for (let i = index + 1; i < allEvents.length; i += 1) {
    const stageName = allEvents[i]?.stage_name;
    if (typeof stageName === 'string' && stageName.trim().length > 0) return stageName;
  }

  return null;
}

function candidateStageNames(event: RunEvent | null, allEvents: RunEvent[]): string[] {
  if (!event) return [];
  const names: string[] = [];

  const push = (value: string | null | undefined) => {
    if (!value || !value.trim().length || names.includes(value)) return;
    names.push(value);
  };

  push(event.stage_name);

  const payload = event.payload as Record<string, unknown> | null | undefined;
  if (typeof payload?.stage_name === 'string') push(payload.stage_name);

  const index = allEvents.findIndex((candidate, i) => {
    if (event.event_id && candidate.event_id === event.event_id) return true;
    return `${candidate.seq}-${i}` === `${event.seq}-${i}`;
  });

  if (index !== -1) {
    for (let i = index - 1; i >= 0; i -= 1) push(allEvents[i]?.stage_name);
    for (let i = index + 1; i < allEvents.length; i += 1) push(allEvents[i]?.stage_name);
  }

  return names;
}

function formatOutputContent(content: string, contentType: string): string {
  if (!content.trim().length) return content;
  if (!contentType.toLowerCase().includes('json')) return content;
  try {
    return `${JSON.stringify(JSON.parse(content), null, 2)}\n`;
  } catch {
    return content;
  }
}

function triggerDownload(filename: string, content: string, contentType: string) {
  const blob = new Blob([content], { type: contentType || 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

function fileNameFromPath(path: string): string {
  const parts = path.split('/').filter(Boolean);
  return parts[parts.length - 1] ?? 'output.json';
}

function EventMessageNote({
  message,
  eventRef,
  compact = false
}: {
  message: string;
  eventRef: string;
  compact?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const [copied, setCopied] = useState<'message' | null>(null);
  const messageRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setExpanded(false);
    setCopied(null);
  }, [eventRef]);

  useEffect(() => {
    const node = messageRef.current;
    if (!node) return;
    const nextOverflowing = node.scrollHeight > node.clientHeight + 2;
    setIsOverflowing(nextOverflowing);
  }, [message, expanded]);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(null), 1400);
    return () => window.clearTimeout(timer);
  }, [copied]);

  async function copyText(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied('message');
    } catch {
      setCopied(null);
    }
  }

  return (
    <div className="inspector-message-block">
      <div
        ref={messageRef}
        className={`inspector-note inspector-message-note ${expanded ? 'expanded' : 'clamped'}`}
      >
        {message}
      </div>
      <div className="inspector-message-actions">
        {isOverflowing && !compact && (
          <button type="button" className="inspector-inline-btn" onClick={() => setExpanded((v) => !v)}>
            {expanded ? 'Collapse' : 'Expand'}
          </button>
        )}
        <button type="button" className="inspector-inline-btn" onClick={() => copyText(message)}>
          {copied === 'message' ? 'Copied' : 'Copy'}
        </button>
      </div>
    </div>
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
  const [outputPreview, setOutputPreview] = useState<{
    stageName: string;
    path: string;
    fileName: string;
    content: string;
    contentType: string;
    sizeBytes: number;
  } | null>(null);
  const [outputLoading, setOutputLoading] = useState(false);
  const [outputError, setOutputError] = useState<string | null>(null);
  const [outputCopied, setOutputCopied] = useState(false);
  const [resolvedOutputStageName, setResolvedOutputStageName] = useState<string | null>(null);
  const [resolvedOutputContent, setResolvedOutputContent] = useState<{ content: string; contentType: string; sizeBytes: number } | null>(null);
  const [outputAvailabilityState, setOutputAvailabilityState] = useState<'idle' | 'checking' | 'available' | 'missing'>('idle');
  const parsed = parseNodeId(selectedNodeId);
  const selectedStage = resolveSelectedStage(selectedNodeId, stages);

  const telemetryByStage = useMemo(() => indexTelemetryByStage(telemetry), [telemetry]);

  const stageTelemetry = selectedStage ? (telemetryByStage.get(selectedStage.stage_name) ?? []) : [];
  const unlabeledGuestTelemetry = useMemo(
    () =>
      telemetry.filter((s) => hasGuestMetrics(s) && (!s.stage_name || s.stage_name === 'unknown')),
    [telemetry]
  );
  const stageGuestTelemetry = useMemo(
    () => stageTelemetry.filter((s) => hasGuestMetrics(s)),
    [stageTelemetry]
  );
  const unlabeledHostTelemetry = useMemo(
    () => telemetry.filter((s) => hasHostMetrics(s) && (!s.stage_name || s.stage_name === 'unknown')),
    [telemetry]
  );
  const stageHostTelemetry = useMemo(
    () => stageTelemetry.filter((s) => hasHostMetrics(s)),
    [stageTelemetry]
  );
  const selectedGuestTelemetry = useMemo(
    () => (stageGuestTelemetry.length > 0 ? stageGuestTelemetry : unlabeledGuestTelemetry),
    [stageGuestTelemetry, unlabeledGuestTelemetry]
  );
  const selectedHostTelemetry = useMemo(
    () => (stageHostTelemetry.length > 0 ? stageHostTelemetry : unlabeledHostTelemetry),
    [stageHostTelemetry, unlabeledHostTelemetry]
  );
  const latestGuestStageSample = selectedStage
    ? [...selectedGuestTelemetry].reverse()[0] ?? null
    : null;
  const latestHostStageSample = selectedStage
    ? [...selectedHostTelemetry].reverse()[0] ?? null
    : null;
  const stageEvents = useMemo(() => filterEventsForStage(events, selectedStage), [events, selectedStage]);
  const stageEventsPerSec = useMemo(() => rollingEventsPerSec(stageEvents), [stageEvents]);
  const runEventsPerSec = useMemo(() => rollingEventsPerSec(events), [events]);
  const stageEventsSeries = useMemo(() => rollingEventsPerSecSeries(stageEvents), [stageEvents]);
  const runEventsSeries = useMemo(() => rollingEventsPerSecSeries(events), [events]);

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

  const latestGuestRunSample = [...telemetry].reverse().find((s) => hasGuestMetrics(s)) ?? null;
  const latestHostRunSample = [...telemetry].reverse().find((s) => hasHostMetrics(s)) ?? null;
  const firstEventTs = events.length > 0 ? events[0]?.timestamp : null;
  const lastEventTs = events.length > 0 ? events[events.length - 1]?.timestamp : null;
  const contextEventsSeries = parsed?.type === 'stage' ? stageEventsSeries : runEventsSeries;
  const contextEventsPoints = useMemo(
    () => buildSparklinePoints(contextEventsSeries, 220, 26),
    [contextEventsSeries]
  );
  const selectedEventMessage = selectedEvent?.message ?? 'No message';
  const selectedEventPath = useMemo(
    () => (selectedEvent ? detectAbsolutePath(selectedEventMessage) : null),
    [selectedEvent, selectedEventMessage]
  );
  const outputStageCandidates = useMemo(
    () => candidateStageNames(selectedEvent, events),
    [selectedEvent, events]
  );
  const referencesStructuredOutputFailure = useMemo(
    () =>
      Boolean(
        selectedEvent
        && /structured output malformed|structured output missing|result\.json/i.test(selectedEventMessage)
      ),
    [selectedEvent, selectedEventMessage]
  );
  const canProbeOutputFile =
    selectedEventPath === '/workspace/output.json'
    || (referencesStructuredOutputFailure && outputStageCandidates.length > 0);
  const canOpenOutputFile = outputAvailabilityState === 'available' && Boolean(resolvedOutputStageName);

  useEffect(() => {
    setOutputPreview(null);
    setOutputLoading(false);
    setOutputError(null);
    setOutputCopied(false);
    setResolvedOutputStageName(null);
    setResolvedOutputContent(null);
    setOutputAvailabilityState('idle');
  }, [selectedEvent?.event_id, selectedEvent?.seq]);

  useEffect(() => {
    if (!outputCopied) return;
    const timer = window.setTimeout(() => setOutputCopied(false), 1400);
    return () => window.clearTimeout(timer);
  }, [outputCopied]);

  useEffect(() => {
    let cancelled = false;

    async function checkOutputAvailability() {
      if (!canProbeOutputFile) return;
      setOutputAvailabilityState('checking');
      setOutputError(null);

      for (const stageName of outputStageCandidates) {
        try {
          const result = await getStageOutputFile(runId, stageName);
          if (cancelled) return;
          setResolvedOutputStageName(stageName);
          setResolvedOutputContent(result);
          setOutputAvailabilityState('available');
          return;
        } catch {
          // try next candidate
        }
      }

      if (!cancelled) {
        setResolvedOutputStageName(null);
        setResolvedOutputContent(null);
        setOutputAvailabilityState('missing');
      }
    }

    void checkOutputAvailability();
    return () => {
      cancelled = true;
    };
  }, [canProbeOutputFile, outputStageCandidates, runId]);

  async function openOutputPreview() {
    if (!resolvedOutputStageName || !resolvedOutputContent) return;
    setOutputLoading(true);
    setOutputError(null);
    try {
      setOutputPreview({
        stageName: resolvedOutputStageName,
        path: '/workspace/output.json',
        fileName: fileNameFromPath('/workspace/output.json'),
        content: formatOutputContent(resolvedOutputContent.content, resolvedOutputContent.contentType),
        contentType: resolvedOutputContent.contentType,
        sizeBytes:
          resolvedOutputContent.sizeBytes > 0
            ? resolvedOutputContent.sizeBytes
            : new Blob([resolvedOutputContent.content]).size
      });
    } finally {
      setOutputLoading(false);
    }
  }

  async function copyOutputContent() {
    if (!outputPreview) return;
    try {
      await navigator.clipboard.writeText(outputPreview.content);
      setOutputCopied(true);
    } catch {
      setOutputCopied(false);
    }
  }

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
            {latestHostStageSample && !latestGuestStageSample && (
              <div className="inspector-note">Daemon host telemetry is available, but guest metrics were not reported for this stage.</div>
            )}
            {latestGuestStageSample && stageGuestTelemetry.length === 0 && unlabeledGuestTelemetry.length > 0 && (
              <div className="inspector-note">Guest telemetry is present but not stage-labeled by daemon; showing run-level guest metrics.</div>
            )}
            <div className="metrics-grid metrics-grid-three">
              <div className="metric-box" title="Events/s (rolling 30s)">
                <span>Events/s</span>
                <strong>{stageEventsPerSec.toFixed(1)}</strong>
              </div>
              <div className="metric-box"><span>Host CPU</span><strong>{typeof latestHostStageSample?.host?.cpu_percent === 'number' ? `${latestHostStageSample.host.cpu_percent.toFixed(1)}%` : '-'}</strong></div>
              <div className="metric-box"><span>Host RSS</span><strong>{fmtMb(latestHostStageSample?.host?.rss_bytes)}</strong></div>
            </div>
            <div className="metric-mini-sparkline" title="Events/s trend (rolling 30s)">
              <svg viewBox="0 0 220 26" preserveAspectRatio="none" aria-hidden="true">
                <polyline className="metric-mini-sparkline-glow" points={contextEventsPoints} />
                <polyline className="metric-mini-sparkline-line" points={contextEventsPoints} />
              </svg>
            </div>
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
          <div className="kv" title="Events/s (rolling 30s)"><span>Events/s</span><strong>{runEventsPerSec.toFixed(1)}</strong></div>
          <EventMessageNote
            eventRef={selectedEvent.event_id ?? `${selectedEvent.seq}`}
            message={selectedEventMessage}
            compact={canProbeOutputFile}
          />
          <div className="inspector-message-actions">
            {canOpenOutputFile && (
              <button
                type="button"
                className="inspector-inline-btn"
                onClick={openOutputPreview}
                disabled={outputLoading}
              >
                {outputLoading ? 'Loading...' : 'Open output.json'}
              </button>
            )}
          </div>
          {canProbeOutputFile && outputAvailabilityState === 'missing' && (
            <div className="inspector-note">
              This log references <code>/workspace/output.json</code>, but the daemon did not publish a retrievable output artifact for this run.
            </div>
          )}
          {outputError && <div className="inspector-note inspector-note-error">{outputError}</div>}
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
            {latestHostRunSample && !latestGuestRunSample && (
              <div className="inspector-note">Daemon host telemetry is available, but guest metrics were not reported.</div>
            )}
            <div className="metrics-grid metrics-grid-three">
              <div className="metric-box" title="Events/s (rolling 30s)">
                <span>Events/s</span>
                <strong>{runEventsPerSec.toFixed(1)}</strong>
              </div>
              <div className="metric-box"><span>Host CPU</span><strong>{typeof latestHostRunSample?.host?.cpu_percent === 'number' ? `${latestHostRunSample.host.cpu_percent.toFixed(1)}%` : '-'}</strong></div>
              <div className="metric-box"><span>Host RSS</span><strong>{fmtMb(latestHostRunSample?.host?.rss_bytes)}</strong></div>
            </div>
            <div className="metric-mini-sparkline" title="Events/s trend (rolling 30s)">
              <svg viewBox="0 0 220 26" preserveAspectRatio="none" aria-hidden="true">
                <polyline className="metric-mini-sparkline-glow" points={contextEventsPoints} />
                <polyline className="metric-mini-sparkline-line" points={contextEventsPoints} />
              </svg>
            </div>
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

      {outputPreview && (
        <div className="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="output-preview-title">
          <div className="output-preview-modal">
            <div className="launch-modal-head">
              <div>
                <h3 id="output-preview-title">{outputPreview.fileName}</h3>
                <div className="output-preview-path">
                  {outputPreview.stageName} - {outputPreview.path}
                </div>
              </div>
              <button className="inspector-btn" onClick={() => setOutputPreview(null)}>Close</button>
            </div>
            <div className="output-preview-meta">
              <span className="output-preview-chip">{outputPreview.contentType}</span>
              <span className="output-preview-chip">{fmtBytes(outputPreview.sizeBytes)}</span>
            </div>
            <div className="output-preview-body">
              <pre className="output-preview-pre">{outputPreview.content}</pre>
            </div>
            <div className="output-preview-actions">
              <button className="inspector-btn" onClick={copyOutputContent}>
                {outputCopied ? 'Copied' : 'Copy'}
              </button>
              <button
                className="launch-primary-btn"
                onClick={() =>
                  triggerDownload(
                    outputPreview.fileName,
                    outputPreview.content,
                    outputPreview.contentType
                  )
                }
              >
                Download
              </button>
            </div>
          </div>
        </div>
      )}
    </aside>
  );
}
