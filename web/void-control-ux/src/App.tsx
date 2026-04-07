import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { RunsList } from './components/RunsList';
import { RunGraph } from './components/RunGraph';
import { NodeInspector } from './components/NodeInspector';
import { RunLogs } from './components/RunLogs';
import { LaunchRunModal } from './components/LaunchRunModal';
import { SwarmOverview } from './components/SwarmOverview';
import { SwarmGraph } from './components/SwarmGraph';
import { SwarmInspector } from './components/SwarmInspector';
import { baseUrl, cancelRun, createExecutionFromSpecText, getExecution, getExecutionEvents, getExecutions, getRun, getRunEvents, getRunStages, getRunTelemetrySamples, getRuns, getRuntimeHealth, launchRunFromSpecText, startRun } from './lib/api';
import { deriveCandidateCards, deriveIterationSummaries, deriveSwarmExecutionSummary } from './lib/orchestration';
import { useUiStore } from './store/ui';
import { defaultStageSelection, eventNodeId, filterEventsForStage, parseNodeId, resolveSelectedStage, runNodeId } from './lib/selectors';
import type { StageView } from './lib/types';

function normalizeStageStatuses(stages: StageView[], runStateRaw?: string): StageView[] {
  const runState = (runStateRaw ?? '').toLowerCase();
  const isTerminal = runState === 'succeeded' || runState === 'failed' || runState === 'cancelled' || runState === 'canceled';
  if (!isTerminal) return stages;

  return stages.map((stage) => {
    // If the run is already terminal, a stage stuck in queued is effectively skipped/blocked.
    if (stage.status === 'queued') {
      return { ...stage, status: 'skipped' };
    }
    return stage;
  });
}

function isHiddenTestRun(runIdRaw: string): boolean {
  const id = runIdRaw.toLowerCase();
  return id.startsWith('contract-') || id.includes('void_box_contract');
}

export function App() {
  const [hideTestRuns, setHideTestRuns] = useState(true);
  const [runStateFilter, setRunStateFilter] = useState<'all' | 'running' | 'failed' | 'succeeded' | 'cancelled'>('all');
  const [isLaunchOpen, setIsLaunchOpen] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchPending, setLaunchPending] = useState(false);
  const [selectedExecutionId, setSelectedExecutionId] = useState<string | null>(null);
  const [pendingLaunchedExecutionId, setPendingLaunchedExecutionId] = useState<string | null>(null);
  const [selectedSwarmIteration, setSelectedSwarmIteration] = useState<number>(0);
  const [selectedSwarmCandidateId, setSelectedSwarmCandidateId] = useState<string | null>(null);
  const selectedRunId = useUiStore((s) => s.selectedRunId);
  const selectedNodeId = useUiStore((s) => s.selectedNodeId);
  const selectedNodeType = useUiStore((s) => s.selectedNodeType);
  const isSelectionPinned = useUiStore((s) => s.isSelectionPinned);
  const setSelectedRunId = useUiStore((s) => s.setSelectedRunId);
  const setSelectedNode = useUiStore((s) => s.setSelectedNode);
  const clearSelectedNode = useUiStore((s) => s.clearSelectedNode);
  const setSelectionPinned = useUiStore((s) => s.setSelectionPinned);
  const setLastSeen = useUiStore((s) => s.setLastSeenEvent);
  const prevRunRef = useRef<string | null>(null);

  const runtimeHealth = useQuery({
    queryKey: ['runtime-health'],
    queryFn: () => getRuntimeHealth(),
    refetchInterval: 5000
  });

  const activeRuns = useQuery({
    queryKey: ['runs', 'active'],
    queryFn: () => getRuns('active'),
    refetchInterval: 2500,
    enabled: runtimeHealth.data === true
  });

  const terminalRuns = useQuery({
    queryKey: ['runs', 'terminal'],
    queryFn: () => getRuns('terminal'),
    refetchInterval: 5000,
    enabled: runtimeHealth.data === true
  });

  const executions = useQuery({
    queryKey: ['executions'],
    queryFn: () => getExecutions(),
    refetchInterval: 2500
  });

  const runDetail = useQuery({
    queryKey: ['run', selectedRunId],
    queryFn: () => getRun(selectedRunId as string),
    enabled: !!selectedRunId,
    refetchInterval: 2000
  });

  const executionDetail = useQuery({
    queryKey: ['execution', selectedExecutionId],
    queryFn: () => getExecution(selectedExecutionId as string),
    enabled: !!selectedExecutionId,
    refetchInterval: 2000
  });

  const executionEvents = useQuery({
    queryKey: ['execution-events', selectedExecutionId],
    queryFn: () => getExecutionEvents(selectedExecutionId as string),
    enabled: !!selectedExecutionId,
    refetchInterval: 1500
  });

  const events = useQuery({
    queryKey: ['events', selectedRunId],
    queryFn: () => getRunEvents(selectedRunId as string),
    enabled: !!selectedRunId,
    refetchInterval: 1200
  });

  const stages = useQuery({
    queryKey: ['stages', selectedRunId],
    queryFn: () => getRunStages(selectedRunId as string),
    enabled: !!selectedRunId,
    refetchInterval: 1500
  });

  const telemetry = useQuery({
    queryKey: ['telemetry', selectedRunId],
    queryFn: () => getRunTelemetrySamples(selectedRunId as string),
    enabled: !!selectedRunId,
    refetchInterval: 1500
  });

  const cancelMutation = useMutation({
    mutationFn: async () => cancelRun(selectedRunId as string, 'cancelled from dashboard')
  });

  const launchMutation = useMutation({
    mutationFn: async ({ file, runId }: { file: string; runId?: string }) => startRun(file, runId)
  });

  const filteredActiveRuns = useMemo(
    () =>
      (activeRuns.data ?? []).filter((run) => {
        if (!hideTestRuns) return true;
        const id = (run.id ?? run.run_id ?? '').trim();
        return !isHiddenTestRun(id);
      }),
    [activeRuns.data, hideTestRuns]
  );

  const filteredTerminalRuns = useMemo(
    () =>
      (terminalRuns.data ?? []).filter((run) => {
        if (!hideTestRuns) return true;
        const id = (run.id ?? run.run_id ?? '').trim();
        return !isHiddenTestRun(id);
      }),
    [terminalRuns.data, hideTestRuns]
  );

  const visibleActiveRuns = useMemo(
    () =>
      filteredActiveRuns.filter((run) => {
        if (runStateFilter === 'all') return true;
        const state = (run.status ?? run.state ?? 'unknown').toString().toLowerCase();
        return state === runStateFilter;
      }),
    [filteredActiveRuns, runStateFilter]
  );

  const visibleTerminalRuns = useMemo(
    () =>
      filteredTerminalRuns.filter((run) => {
        if (runStateFilter === 'all') return true;
        const state = (run.status ?? run.state ?? 'unknown').toString().toLowerCase();
        return state === runStateFilter;
      }),
    [filteredTerminalRuns, runStateFilter]
  );

  const resolvedRunId = useMemo(() => {
    if (selectedExecutionId) return null;
    if (selectedRunId) return selectedRunId;
    const firstActive = visibleActiveRuns[0];
    const firstTerminal = visibleTerminalRuns[0];
    return (firstActive?.id ?? firstActive?.run_id ?? firstTerminal?.id ?? firstTerminal?.run_id ?? null) as string | null;
  }, [selectedExecutionId, selectedRunId, visibleActiveRuns, visibleTerminalRuns]);

  const resolvedExecutionId = useMemo(() => {
    if (selectedExecutionId) return selectedExecutionId;
    if (selectedRunId) return null;
    return executions.data?.[0]?.execution_id ?? null;
  }, [executions.data, selectedExecutionId, selectedRunId]);

  const eventList = events.data ?? [];
  const executionError =
    (executions.error as Error | null)?.message
    ?? (executionDetail.error as Error | null)?.message
    ?? (executionEvents.error as Error | null)?.message;
  const runtimeDetailError =
    (runDetail.error as Error | null)?.message
    ?? (events.error as Error | null)?.message
    ?? (stages.error as Error | null)?.message
    ?? (telemetry.error as Error | null)?.message;
  const detailError = resolvedExecutionId ? executionError : runtimeDetailError;

  const normalizedStages = useMemo(
    () => normalizeStageStatuses(stages.data ?? [], (runDetail.data?.status ?? runDetail.data?.state)?.toString()),
    [stages.data, runDetail.data?.status, runDetail.data?.state]
  );
  const parsedSelected = useMemo(() => parseNodeId(selectedNodeId), [selectedNodeId]);
  const selectedStage = useMemo(
    () => resolveSelectedStage(selectedNodeId, normalizedStages),
    [selectedNodeId, normalizedStages]
  );
  const selectedEventRef = parsedSelected?.type === 'event' ? parsedSelected.eventRef : null;
  const scopedEvents = useMemo(() => {
    if (parsedSelected?.type === 'stage' && selectedStage) {
      return filterEventsForStage(eventList, selectedStage);
    }
    return eventList;
  }, [parsedSelected?.type, selectedStage, eventList]);

  const swarmSummary = useMemo(
    () => (executionDetail.data ? deriveSwarmExecutionSummary(executionDetail.data) : null),
    [executionDetail.data]
  );
  const swarmIterations = useMemo(
    () => (executionDetail.data ? deriveIterationSummaries(executionDetail.data) : []),
    [executionDetail.data]
  );
  const selectedIterationIndex = useMemo(() => {
    if (swarmIterations.length === 0) return 0;
    if (swarmIterations.some((iteration) => iteration.iterationIndex === selectedSwarmIteration)) {
      return selectedSwarmIteration;
    }
    return swarmIterations[swarmIterations.length - 1]?.iterationIndex ?? 0;
  }, [swarmIterations, selectedSwarmIteration]);
  const swarmCandidates = useMemo(
    () => (executionDetail.data ? deriveCandidateCards(executionDetail.data, selectedIterationIndex) : []),
    [executionDetail.data, selectedIterationIndex]
  );
  const selectedSwarmCandidate = useMemo(
    () => swarmCandidates.find((candidate) => candidate.candidateId === selectedSwarmCandidateId) ?? swarmCandidates[0] ?? null,
    [swarmCandidates, selectedSwarmCandidateId]
  );

  useEffect(() => {
    if (!selectedExecutionId && !selectedRunId && resolvedExecutionId) {
      setSelectedExecutionId(resolvedExecutionId);
      return;
    }
    if (!selectedExecutionId && !selectedRunId && resolvedRunId) {
      setSelectedRunId(resolvedRunId);
    }
  }, [selectedExecutionId, selectedRunId, resolvedExecutionId, resolvedRunId, setSelectedRunId]);

  useEffect(() => {
    if (!pendingLaunchedExecutionId) return;
    if (!(executions.data ?? []).some((execution) => execution.execution_id === pendingLaunchedExecutionId)) return;
    clearSelectedNode();
    setSelectedRunId(null);
    setSelectedSwarmIteration(0);
    setSelectedSwarmCandidateId(null);
    setSelectedExecutionId(pendingLaunchedExecutionId);
    setPendingLaunchedExecutionId(null);
  }, [pendingLaunchedExecutionId, executions.data, clearSelectedNode, setSelectedRunId]);

  useEffect(() => {
    if (!resolvedExecutionId) return;
    setSelectedSwarmIteration((current) => {
      if (swarmIterations.some((iteration) => iteration.iterationIndex === current)) return current;
      return swarmIterations[swarmIterations.length - 1]?.iterationIndex ?? 0;
    });
  }, [resolvedExecutionId, swarmIterations]);

  useEffect(() => {
    if (!selectedSwarmCandidate && swarmCandidates.length > 0) {
      setSelectedSwarmCandidateId(swarmCandidates[0].candidateId);
      return;
    }
    if (
      selectedSwarmCandidateId &&
      swarmCandidates.length > 0 &&
      !swarmCandidates.some((candidate) => candidate.candidateId === selectedSwarmCandidateId)
    ) {
      setSelectedSwarmCandidateId(swarmCandidates[0].candidateId);
    }
  }, [selectedSwarmCandidate, selectedSwarmCandidateId, swarmCandidates]);

  useEffect(() => {
    if (resolvedRunId && eventList.length > 0) {
      setLastSeen(resolvedRunId, eventList[eventList.length - 1]?.event_id);
    }
  }, [resolvedRunId, eventList, setLastSeen]);

  useEffect(() => {
    if (!resolvedRunId) {
      clearSelectedNode();
      prevRunRef.current = null;
      return;
    }
    if (isSelectionPinned) return;

    const parsed = parseNodeId(selectedNodeId);
    const runChanged = prevRunRef.current !== resolvedRunId;
    const nodeMatchesRun = parsed?.runId === resolvedRunId;
    const stageList = normalizedStages;
    const runState = (runDetail.data?.status ?? runDetail.data?.state ?? '').toString().toLowerCase();
    const failedTerminalEvent = runState === 'failed'
      ? [...eventList].reverse().find((event) =>
          event.level === 'error'
          || (event.event_type_v2 ?? event.event_type).toLowerCase().includes('failed')
        ) ?? null
      : null;
    const promoteRunRoot = parsed?.type === 'run' && stageList.length > 0;
    const needsDefault = runChanged || !selectedNodeId || !nodeMatchesRun || promoteRunRoot;

    if (needsDefault) {
      const latestEvent = eventList.length > 0 ? eventList[eventList.length - 1] : null;
      setSelectedNode(
        failedTerminalEvent
          ? eventNodeId(resolvedRunId, failedTerminalEvent.event_id || `${failedTerminalEvent.seq}`)
          : stageList.length > 0
          ? defaultStageSelection(resolvedRunId, stageList)
          : latestEvent
            ? eventNodeId(resolvedRunId, latestEvent.event_id || `${latestEvent.seq}-latest`)
            : runNodeId(resolvedRunId),
        failedTerminalEvent ? 'event' : stageList.length > 0 ? 'stage' : (latestEvent ? 'event' : 'run')
      );
    }
    prevRunRef.current = resolvedRunId;
  }, [
    resolvedRunId,
    selectedNodeId,
    isSelectionPinned,
    runDetail.data?.status,
    runDetail.data?.state,
    stages.data,
    normalizedStages,
    eventList,
    setSelectedNode,
    clearSelectedNode
  ]);

  const onLaunchRun = async (params: { file: string; runId?: string; specText?: string }) => {
    setLaunchError(null);
    setLaunchPending(true);
    try {
      let result: { run_id?: string; runId?: string; attempt_id?: number; state?: string } | null = null;
      const hasSpecText = Boolean(params.specText && params.specText.trim().length > 0);
      if (hasSpecText) {
        const specText = params.specText as string;
        const looksLikeExecutionSpec =
          /^\s*mode\s*:\s*/m.test(specText)
          && /^\s*goal\s*:\s*/m.test(specText);

        try {
          if (looksLikeExecutionSpec) {
            const execution = await createExecutionFromSpecText({ specText });
            setPendingLaunchedExecutionId(execution.execution_id);
            await executions.refetch();
            clearSelectedNode();
            setSelectedRunId(null);
            setSelectedSwarmIteration(0);
            setSelectedSwarmCandidateId(null);
            setSelectedExecutionId(execution.execution_id);
            setIsLaunchOpen(false);
            return;
          }

          result = await launchRunFromSpecText({
            specText,
            runId: params.runId,
            file: params.file,
            specFormat: specText.trimStart().startsWith('{') ? 'json' : 'yaml'
          });
        } catch (error) {
          const msg = error instanceof Error ? error.message : String(error);
          const bridgeUnavailable =
            msg.includes('Failed to fetch') ||
            msg.includes('ECONNREFUSED') ||
            msg.includes('HTTP 404') ||
            msg.includes('HTTP 405');
          if (!bridgeUnavailable && looksLikeExecutionSpec) {
            throw error;
          }
          if (!bridgeUnavailable) {
            result = await launchRunFromSpecText({
              specText,
              runId: params.runId,
              file: params.file,
              specFormat: specText.trimStart().startsWith('{') ? 'json' : 'yaml'
            });
          } else {
            result = await launchMutation.mutateAsync({ file: params.file, runId: params.runId });
          }
        }
      } else {
        result = await launchMutation.mutateAsync({ file: params.file, runId: params.runId });
      }
      const createdRunId = result.run_id ?? ('runId' in result ? result.runId : undefined) ?? params.runId;
      await Promise.all([activeRuns.refetch(), terminalRuns.refetch()]);
            if (createdRunId) {
        setSelectedRunId(createdRunId);
      }
      setIsLaunchOpen(false);
    } catch (error) {
      const msg = error instanceof Error ? error.message : String(error);
      setLaunchError(msg);
    } finally {
      setLaunchPending(false);
    }
  };

  const onSelectEvent = (event: { event_id?: string; seq: number; run_id?: string }) => {
    if (!resolvedRunId) return;
    if (!('run_id' in event)) return;
    const ref = (event.event_id ?? '').trim();
    setSelectedNode(eventNodeId(resolvedRunId, ref.length > 0 ? ref : `${event.seq}`), 'event');
  };

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="logo" aria-label="Void Control logo">
            <svg viewBox="0 0 36 36" className="logo-mark" aria-hidden="true">
              <defs>
                <linearGradient id="void-logo-shell" x1="0%" y1="0%" x2="100%" y2="100%">
                  <stop offset="0%" stopColor="#dff7ff" />
                  <stop offset="48%" stopColor="#6fd8ff" />
                  <stop offset="100%" stopColor="#143761" />
                </linearGradient>
                <linearGradient id="void-logo-core" x1="18%" y1="8%" x2="82%" y2="88%">
                  <stop offset="0%" stopColor="#f1fdff" />
                  <stop offset="58%" stopColor="#7ed7a6" />
                  <stop offset="100%" stopColor="#1a4d7e" />
                </linearGradient>
              </defs>
              <rect x="5" y="5" width="26" height="26" rx="7" fill="#081327" stroke="#8fe8ff" strokeWidth="1.4" />
              <rect x="4" y="4" width="28" height="28" rx="8" fill="none" stroke="rgba(34,211,238,0.28)" strokeWidth="1" />
              <path
                d="M10 18.2 17.2 9.8 29 14.3 25 27 12.4 25.1Z"
                fill="url(#void-logo-shell)"
                stroke="#dff7ff"
                strokeWidth="0.8"
              />
              <path
                d="M15.3 13.3 23.1 15.7 20.3 23 12.9 20.7Z"
                fill="url(#void-logo-core)"
                className="logo-mark-accent"
              />
              <path d="M17.2 9.8 18.6 18.2 29 14.3" fill="rgba(255,255,255,0.18)" />
              <circle cx="18.5" cy="18.1" r="1.8" fill="#f4feff" />
            </svg>
          </span>
          <div>
            <div className="brand-name">Void Control</div>
            <div className="brand-sub">orchestration explorer</div>
          </div>
        </div>
        <div className="meta">daemon: {baseUrl}</div>
      </header>

      <main className="layout">
        <RunsList
          executions={executions.data ?? []}
          activeRuns={visibleActiveRuns}
          terminalRuns={visibleTerminalRuns}
          runtimeUnavailable={runtimeHealth.data === false}
          selectedId={resolvedExecutionId ?? resolvedRunId}
          selectedKind={resolvedExecutionId ? 'execution' : resolvedRunId ? 'run' : null}
          onSelectExecution={(executionId) => {
            setSelectedExecutionId(executionId);
            setSelectedRunId(null);
            clearSelectedNode();
          }}
          onSelectRun={(runId) => {
            setSelectedExecutionId(null);
            setSelectedRunId(runId);
          }}
          onLaunch={() => {
            setLaunchError(null);
            setIsLaunchOpen(true);
          }}
          hideTestRuns={hideTestRuns}
          onToggleHideTestRuns={() => setHideTestRuns((v) => !v)}
          stateFilter={runStateFilter}
          onStateFilterChange={setRunStateFilter}
        />

        <section className="detail-panel">
          {detailError && (
            <div className="error-banner">
              <strong>Connection error:</strong> {detailError}
            </div>
          )}
          <div className="toolbar">
            <div>
              <strong>{resolvedExecutionId ? 'Execution' : 'Run'}:</strong> {resolvedExecutionId ?? resolvedRunId ?? '-'}
              <span className="state-pill">
                {resolvedExecutionId
                  ? (executionDetail.data?.execution.status ?? 'unknown').toString()
                  : (runDetail.data?.status ?? runDetail.data?.state ?? 'unknown').toString()}
              </span>
            </div>
            <div className="ops-actions">
              <button
                className="danger"
                disabled={!resolvedRunId || cancelMutation.isPending || !!resolvedExecutionId}
                onClick={() => cancelMutation.mutate()}
              >
                Cancel Run
              </button>
            </div>
          </div>

          {!resolvedRunId && !resolvedExecutionId ? (
            <div className="empty">No runs yet. Start one from terminal and refresh.</div>
          ) : resolvedExecutionId ? (
            <div className="detail-grid swarm-detail-grid">
              {swarmSummary ? (
                <>
                  <div className="center-panel">
                    <SwarmGraph
                      summary={swarmSummary}
                      iterations={swarmIterations}
                      candidates={swarmCandidates}
                      selectedCandidateId={selectedSwarmCandidate?.candidateId ?? null}
                      onSelectCandidate={setSelectedSwarmCandidateId}
                    />
                    <RunLogs
                      events={executionEvents.data ?? []}
                      selectedEventRef={null}
                    />
                  </div>
                  <SwarmInspector
                    summary={swarmSummary}
                    iteration={swarmIterations.find((iteration) => iteration.iterationIndex === selectedIterationIndex) ?? null}
                    candidate={selectedSwarmCandidate}
                    events={executionEvents.data ?? []}
                    onOpenRuntime={(runtimeRunId) => {
                      setSelectedExecutionId(null);
                      setSelectedRunId(runtimeRunId);
                    }}
                  />
                </>
              ) : (
                <div className="empty">Loading execution graph.</div>
              )}
            </div>
          ) : (
            <div className="detail-grid">
              <div className="center-panel">
                {(eventList.length === 0 && (stages.data ?? []).length === 0) ? (
                  <div className="graph-box graph-empty runtime-empty-state">
                    <div className="panel-title">Runtime Pending</div>
                    <div className="runtime-empty-copy">
                      No stages or events are available for this run yet.
                    </div>
                    <div className="empty">
                      The runtime graph and event log will appear after the daemon reports stage or event data.
                    </div>
                  </div>
                ) : (
                  <>
                    <RunGraph
                      runId={resolvedRunId as string}
                      events={scopedEvents}
                      stages={normalizedStages}
                      selectedNodeId={selectedNodeId}
                      onSelectNode={setSelectedNode}
                    />
                    <RunLogs
                      events={scopedEvents}
                      selectedEventRef={selectedEventRef}
                      onSelectEvent={onSelectEvent}
                    />
                  </>
                )}
              </div>

              <NodeInspector
                runId={resolvedRunId as string}
                selectedNodeId={selectedNodeId}
                selectedNodeType={selectedNodeType}
                isPinned={isSelectionPinned}
                stages={normalizedStages}
                events={eventList}
                telemetry={telemetry.data ?? []}
                onClearSelection={() => {
                  clearSelectedNode();
                  setSelectedNode(runNodeId(resolvedRunId as string), 'run');
                }}
                onTogglePinned={() => setSelectionPinned(!isSelectionPinned)}
              />
            </div>
          )}
        </section>
      </main>

      <LaunchRunModal
        open={isLaunchOpen}
        isSubmitting={launchPending || launchMutation.isPending}
        submitError={launchError ?? (launchMutation.error instanceof Error ? launchMutation.error.message : null)}
        onClose={() => setIsLaunchOpen(false)}
        onSubmit={onLaunchRun}
      />
    </div>
  );
}
