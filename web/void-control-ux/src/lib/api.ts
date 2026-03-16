import type {
  RunEvent,
  RunInspection,
  RunStagesResponse,
  RunsListResponse,
  RunTelemetryResponse,
  StageOutputFile,
  StageView,
  TelemetrySample
} from './types';

const runtimeBaseUrl = (import.meta.env.VITE_VOID_BOX_BASE_URL as string | undefined) ?? '/api';
const controlBaseUrl =
  (import.meta.env.VITE_VOID_CONTROL_BASE_URL as string | undefined) ??
  'http://127.0.0.1:43210';

async function requestJsonAt<T>(base: string, path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${base}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...init
  });

  if (!res.ok) {
    let body = '';
    try {
      body = await res.text();
    } catch {
      body = '<no-body>';
    }
    throw new Error(`HTTP ${res.status} ${res.statusText}: ${body}`);
  }

  return (await res.json()) as T;
}

async function requestJson<T>(path: string, init?: RequestInit): Promise<T> {
  return requestJsonAt(runtimeBaseUrl, path, init);
}

async function requestText(path: string, init?: RequestInit): Promise<StageOutputFile> {
  const res = await fetch(`${runtimeBaseUrl}${path}`, init);

  if (!res.ok) {
    let body = '';
    try {
      body = await res.text();
    } catch {
      body = '<no-body>';
    }
    throw new Error(`HTTP ${res.status} ${res.statusText}: ${body}`);
  }

  return {
    content: await res.text(),
    contentType: res.headers.get('content-type') ?? 'text/plain',
    sizeBytes: Number(res.headers.get('content-length') ?? 0)
  };
}

export async function getRuns(state?: 'active' | 'terminal'): Promise<RunInspection[]> {
  const query = state ? `?state=${state}` : '';
  const body = await requestJson<RunsListResponse>(`/v1/runs${query}`);
  return body.runs ?? [];
}

export function getRunId(run: RunInspection): string {
  return run.id ?? run.run_id ?? 'unknown';
}

export async function getRun(runId: string): Promise<RunInspection> {
  return requestJson<RunInspection>(`/v1/runs/${encodeURIComponent(runId)}`);
}

export async function getRunEvents(runId: string, fromEventId?: string): Promise<RunEvent[]> {
  const query = fromEventId ? `?from_event_id=${encodeURIComponent(fromEventId)}` : '';
  const events = await requestJson<RunEvent[]>(`/v1/runs/${encodeURIComponent(runId)}/events${query}`);
  return events.map((event) => {
    const payloadData = (event.payload as { data?: unknown } | null | undefined)?.data;
    const normalizedPayloadText = typeof payloadData === 'string' ? payloadData.trim() : '';
    const type = event.event_type_v2 ?? event.event_type ?? '';
    if (
      type === 'LogChunk' &&
      normalizedPayloadText.length > 0 &&
      (event.message ?? '').toLowerCase() === 'stream chunk'
    ) {
      return { ...event, message: normalizedPayloadText };
    }
    return event;
  });
}

export async function getRunStages(runId: string): Promise<StageView[]> {
  const body = await requestJson<RunStagesResponse>(`/v1/runs/${encodeURIComponent(runId)}/stages`);
  return body.stages ?? [];
}

export async function getRunTelemetry(runId: string, fromSeq = 0): Promise<RunTelemetryResponse> {
  return requestJson<RunTelemetryResponse>(`/v1/runs/${encodeURIComponent(runId)}/telemetry?from_seq=${fromSeq}`);
}

export async function getRunTelemetrySamples(runId: string): Promise<TelemetrySample[]> {
  const body = await getRunTelemetry(runId, 0);
  return body.samples ?? [];
}

export async function getStageOutputFile(runId: string, stageName: string): Promise<StageOutputFile> {
  return requestText(
    `/v1/runs/${encodeURIComponent(runId)}/stages/${encodeURIComponent(stageName)}/output-file`
  );
}

export async function cancelRun(runId: string, reason: string): Promise<{ run_id: string; state: string; terminal_event_id?: string }> {
  return requestJson(`/v1/runs/${encodeURIComponent(runId)}/cancel`, {
    method: 'POST',
    body: JSON.stringify({ reason })
  });
}

export async function startRun(file: string, runId?: string): Promise<{ run_id?: string; runId?: string; attempt_id?: number; state?: string }> {
  const body: Record<string, string> = { file };
  if (runId && runId.trim().length > 0) body.run_id = runId.trim();
  return requestJson('/v1/runs', {
    method: 'POST',
    body: JSON.stringify(body)
  });
}

export async function launchRunFromSpecText(params: {
  specText: string;
  runId?: string;
  file?: string;
  specFormat?: 'yaml' | 'json';
}): Promise<{ run_id?: string; attempt_id?: number; state?: string; file?: string }> {
  return requestJsonAt(controlBaseUrl, '/v1/launch', {
    method: 'POST',
    body: JSON.stringify({
      run_id: params.runId,
      file: params.file,
      spec_text: params.specText,
      spec_format: params.specFormat
    })
  });
}

export { runtimeBaseUrl as baseUrl, controlBaseUrl };
