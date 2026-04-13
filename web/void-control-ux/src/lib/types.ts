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

export interface ExecutionInspection {
  execution_id: string;
  mode: string;
  goal: string;
  status: string;
  result_best_candidate_id?: string | null;
  completed_iterations?: number;
  failure_counts?: {
    total_candidate_failures?: number;
  } | null;
}

export interface ExecutionProgress {
  completed_iterations?: number;
  scoring_history_len?: number;
  event_count?: number;
  last_event?: string | null;
  candidate_queue_count?: number;
  candidate_dispatch_count?: number;
  candidate_output_count?: number;
  queued_candidate_count?: number;
  running_candidate_count?: number;
  completed_candidate_count?: number;
  failed_candidate_count?: number;
  canceled_candidate_count?: number;
  event_type_counts?: Record<string, number>;
}

export interface ExecutionResult {
  best_candidate_id?: string | null;
  completed_iterations?: number;
  total_candidate_failures?: number;
}

export interface ExecutionDetailResponse {
  execution: ExecutionInspection;
  progress?: ExecutionProgress;
  result?: ExecutionResult;
  candidates: ExecutionCandidate[];
}

export interface ExecutionsListResponse {
  executions: ExecutionInspection[];
}

export interface ExecutionEvent {
  seq: number;
  event_type: string;
  timestamp?: string | null;
  message?: string | null;
  payload?: Record<string, unknown> | null;
}

export interface ExecutionEventsResponse {
  execution_id: string;
  events: ExecutionEvent[];
}

export interface ExecutionCandidate {
  execution_id: string;
  candidate_id: string;
  created_seq: number;
  iteration: number;
  status: 'Queued' | 'Running' | 'Completed' | 'Failed' | 'Canceled' | string;
  runtime_run_id?: string | null;
  overrides?: Record<string, string>;
  succeeded?: boolean | null;
  metrics: Record<string, number>;
  review_status?: 'PendingReview' | 'Approved' | 'RevisionRequested' | 'RetryRequested' | 'Rejected' | string | null;
  revision_round?: number;
}

export interface SwarmHealthChip {
  label: string;
  tone: 'neutral' | 'good' | 'warn' | 'bad';
}

export interface SwarmIterationSummary {
  iterationIndex: number;
  iterationLabel: number;
  candidateCount: number;
  queued: number;
  running: number;
  outputReady: number;
  scored: number;
  failed: number;
  completed: number;
  bestCandidateId?: string | null;
}

export interface SwarmCandidateCard {
  candidateId: string;
  iterationIndex: number;
  iterationLabel: number;
  runtimeRunId?: string | null;
  state:
    | 'queued'
    | 'running'
    | 'output_ready'
    | 'scored'
    | 'failed'
    | 'best'
    | 'rejected'
    | 'canceled';
  metrics: {
    latency?: string | null;
    errorRate?: string | null;
    cpu?: string | null;
  };
  reason?: string | null;
}

export interface SwarmExecutionSummary {
  executionId: string;
  mode: string;
  goal: string;
  status: string;
  completedIterations: number;
  currentIterationLabel: number;
  bestCandidateId?: string | null;
  counts: {
    queued: number;
    running: number;
    outputReady: number;
    scored: number;
    failed: number;
    completed: number;
  };
  healthChips: SwarmHealthChip[];
}

export interface SupervisionWorkerCard {
  workerId: string;
  iterationIndex: number;
  iterationLabel: number;
  runtimeRunId?: string | null;
  state:
    | 'queued'
    | 'running'
    | 'approved'
    | 'revision_requested'
    | 'retry_requested'
    | 'rejected'
    | 'failed'
    | 'canceled';
  reviewStatus?: string | null;
  revisionRound: number;
  metrics: {
    latency?: string | null;
    errorRate?: string | null;
    cpu?: string | null;
  };
  role?: string | null;
  reason?: string | null;
}

export interface SupervisionExecutionSummary {
  executionId: string;
  mode: string;
  goal: string;
  status: string;
  completedIterations: number;
  supervisorRole: string;
  approvedWorkerId?: string | null;
  counts: {
    queued: number;
    running: number;
    approved: number;
    revisionRequested: number;
    retryRequested: number;
    rejected: number;
    failed: number;
    completed: number;
  };
  healthChips: SwarmHealthChip[];
}
