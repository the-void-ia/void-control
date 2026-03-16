export type RunState =
  | 'pending'
  | 'starting'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'canceled';

export interface RunInspection {
  id?: string;
  run_id?: string;
  status?: RunState;
  state?: RunState;
  attempt_id?: number;
  active_stage_count?: number;
  active_microvm_count?: number;
  started_at?: string;
  updated_at?: string;
  terminal_reason?: string | null;
  exit_code?: number | null;
}

export interface RunsListResponse {
  runs: RunInspection[];
}

export interface RunEvent {
  event_id: string;
  event_type: string;
  event_type_v2?: string | null;
  run_id: string;
  attempt_id: number;
  timestamp: string;
  seq: number;
  stage_name?: string | null;
  group_id?: string | null;
  box_name?: string | null;
  stream?: string | null;
  level?: string;
  message?: string;
  payload?: Record<string, unknown> | null;
}

export type StageStatus = 'queued' | 'running' | 'succeeded' | 'failed' | 'skipped';

export interface StageView {
  stage_name: string;
  box_name?: string | null;
  group_id: string;
  depends_on: string[];
  status: StageStatus;
  stage_attempt: number;
  started_at?: string | null;
  completed_at?: string | null;
  duration_ms?: number | null;
  exit_code?: number | null;
}

export interface RunStagesResponse {
  run_id: string;
  attempt_id: number;
  updated_at: string;
  stages: StageView[];
}

export interface TelemetrySample {
  seq: number;
  timestamp_ms: number;
  timestamp?: string | null;
  stage_name: string;
  guest?: {
    cpu_percent?: number;
    memory_used_bytes?: number;
    memory_total_bytes?: number;
    net_rx_bytes?: number;
    net_tx_bytes?: number;
    procs_running?: number;
    open_fds?: number;
  } | null;
  host?: {
    rss_bytes?: number;
    cpu_percent?: number;
    io_read_bytes?: number;
    io_write_bytes?: number;
  } | null;
}

export interface RunTelemetryResponse {
  run_id: string;
  attempt_id: number;
  next_seq: number;
  samples: TelemetrySample[];
}

export interface StageOutputFile {
  content: string;
  contentType: string;
  sizeBytes: number;
}
