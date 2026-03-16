import type { RunEvent, StageView, TelemetrySample } from './types';

export type SelectedNodeType = 'run' | 'stage' | 'event' | null;

export function stageNodeId(runId: string, stageName: string, stageIndex: number): string {
  return `stage:${runId}:${stageName}:${stageIndex}`;
}

export function runNodeId(runId: string): string {
  return `run:${runId}`;
}

export function eventNodeId(runId: string, eventIdOrIndex: string | number): string {
  return `event:${runId}:${eventIdOrIndex}`;
}

export function parseNodeId(nodeId: string | null):
  | { type: 'run'; runId: string }
  | { type: 'stage'; runId: string; stageName: string; stageIndex: number }
  | { type: 'event'; runId: string; eventRef: string }
  | null {
  if (!nodeId) return null;

  if (nodeId.startsWith('run:')) {
    return { type: 'run', runId: nodeId.slice(4) };
  }

  if (nodeId.startsWith('stage:')) {
    const parts = nodeId.split(':');
    if (parts.length < 4) return null;
    const runId = parts[1];
    const stageIndexRaw = parts[parts.length - 1];
    const stageIndex = Number(stageIndexRaw);
    if (!Number.isFinite(stageIndex)) return null;
    const stageName = parts.slice(2, parts.length - 1).join(':');
    return { type: 'stage', runId, stageName, stageIndex };
  }

  if (nodeId.startsWith('event:')) {
    const parts = nodeId.split(':');
    if (parts.length < 3) return null;
    return {
      type: 'event',
      runId: parts[1],
      eventRef: parts.slice(2).join(':')
    };
  }

  return null;
}

export function defaultStageSelection(runId: string, stages: StageView[]): string {
  if (stages.length === 0) return runNodeId(runId);
  const runningIndex = stages.findIndex((s) => s.status === 'running');
  const index = runningIndex >= 0 ? runningIndex : 0;
  const stage = stages[index];
  return stageNodeId(runId, stage.stage_name, index);
}

export function resolveSelectedStage(nodeId: string | null, stages: StageView[]): StageView | null {
  const parsed = parseNodeId(nodeId);
  if (!parsed || parsed.type !== 'stage') return null;
  return stages[parsed.stageIndex] ?? stages.find((s) => s.stage_name === parsed.stageName) ?? null;
}

export function indexTelemetryByStage(telemetry: TelemetrySample[]): Map<string, TelemetrySample[]> {
  const out = new Map<string, TelemetrySample[]>();
  for (const sample of telemetry) {
    const key = sample.stage_name || 'unknown';
    const arr = out.get(key) ?? [];
    arr.push(sample);
    out.set(key, arr);
  }
  return out;
}

export function latestTelemetryByStage(telemetry: TelemetrySample[]): Map<string, TelemetrySample> {
  const out = new Map<string, TelemetrySample>();
  for (const sample of telemetry) {
    out.set(sample.stage_name || 'unknown', sample);
  }
  return out;
}

export function filterEventsForStage(events: RunEvent[], stage: StageView | null): RunEvent[] {
  if (!stage) return [];
  return events.filter((event) => {
    if (event.stage_name && event.stage_name === stage.stage_name) return true;
    if (stage.box_name && event.box_name && event.box_name === stage.box_name) return true;

    const payload = event.payload ?? {};
    const payloadStage = typeof payload.stage_name === 'string' ? payload.stage_name : null;
    const payloadBox = typeof payload.box_name === 'string' ? payload.box_name : null;

    return payloadStage === stage.stage_name || (Boolean(stage.box_name) && payloadBox === stage.box_name);
  });
}

export function rollingEventsPerSec(events: RunEvent[], windowMs = 30_000): number {
  if (events.length === 0 || windowMs <= 0) return 0;

  let latestTs = Number.NEGATIVE_INFINITY;
  const times: number[] = [];

  for (const event of events) {
    const ts = Date.parse(event.timestamp ?? '');
    if (!Number.isFinite(ts)) continue;
    latestTs = Math.max(latestTs, ts);
    times.push(ts);
  }

  if (!Number.isFinite(latestTs) || times.length === 0) return 0;

  const floor = latestTs - windowMs;
  const inWindow = times.filter((ts) => ts >= floor && ts <= latestTs).length;
  return Number((inWindow / (windowMs / 1000)).toFixed(1));
}

export function rollingEventsPerSecSeries(
  events: RunEvent[],
  windowMs = 30_000,
  buckets = 18
): number[] {
  if (windowMs <= 0 || buckets <= 0) return [];

  const times: number[] = [];
  let latestTs = Number.NEGATIVE_INFINITY;
  for (const event of events) {
    const ts = Date.parse(event.timestamp ?? '');
    if (!Number.isFinite(ts)) continue;
    latestTs = Math.max(latestTs, ts);
    times.push(ts);
  }
  if (!Number.isFinite(latestTs)) return Array.from({ length: buckets }, () => 0);

  const start = latestTs - windowMs;
  const bucketMs = windowMs / buckets;
  const counts = Array.from({ length: buckets }, () => 0);

  for (const ts of times) {
    if (ts < start || ts > latestTs) continue;
    const raw = Math.floor((ts - start) / bucketMs);
    const idx = Math.max(0, Math.min(buckets - 1, raw));
    counts[idx] += 1;
  }

  return counts.map((v) => Number((v / (bucketMs / 1000)).toFixed(2)));
}
