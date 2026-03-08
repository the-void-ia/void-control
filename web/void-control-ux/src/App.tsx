import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { RunsList } from './components/RunsList';
import { RunGraph } from './components/RunGraph';
import { NodeInspector } from './components/NodeInspector';
import { RunLogs } from './components/RunLogs';
import { EventTimeline } from './components/EventTimeline';
import { LaunchRunModal } from './components/LaunchRunModal';
import { baseUrl, cancelRun, getRun, getRunEvents, getRunStages, getRunTelemetrySamples, getRuns, launchRunFromSpecText, startRun } from './lib/api';
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

export function App() {
  const [hideTestRuns, setHideTestRuns] = useState(true);
  const [isLaunchOpen, setIsLaunchOpen] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchPending, setLaunchPending] = useState(false);
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

  const activeRuns = useQuery({
    queryKey: ['runs', 'active'],
    queryFn: () => getRuns('active'),
    refetchInterval: 2500
  });

  const terminalRuns = useQuery({
    queryKey: ['runs', 'terminal'],
    queryFn: () => getRuns('terminal'),
    refetchInterval: 5000
  });

  const runDetail = useQuery({
    queryKey: ['run', selectedRunId],
    queryFn: () => getRun(selectedRunId as string),
    enabled: !!selectedRunId,
    refetchInterval: 2000
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
        const id = (run.id ?? run.run_id ?? '').toLowerCase();
        return !id.startsWith('contract-') && !id.startsWith('run-') && !id.includes('void_box_contract');
      }),
    [activeRuns.data, hideTestRuns]
  );

  const filteredTerminalRuns = useMemo(
    () =>
      (terminalRuns.data ?? []).filter((run) => {
        if (!hideTestRuns) return true;
        const id = (run.id ?? run.run_id ?? '').toLowerCase();
        return !id.startsWith('contract-') && !id.startsWith('run-') && !id.includes('void_box_contract');
      }),
    [terminalRuns.data, hideTestRuns]
  );

  const resolvedRunId = useMemo(() => {
    if (selectedRunId) return selectedRunId;
    const firstActive = filteredActiveRuns[0];
    const firstTerminal = filteredTerminalRuns[0];
    return (firstActive?.id ?? firstActive?.run_id ?? firstTerminal?.id ?? firstTerminal?.run_id ?? null) as string | null;
  }, [selectedRunId, filteredActiveRuns, filteredTerminalRuns]);

  const eventList = events.data ?? [];
  const listError = (activeRuns.error as Error | null)?.message ?? (terminalRuns.error as Error | null)?.message;
  const detailError = (runDetail.error as Error | null)?.message
    ?? (events.error as Error | null)?.message
    ?? (stages.error as Error | null)?.message
    ?? (telemetry.error as Error | null)?.message;

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

  useEffect(() => {
    if (!selectedRunId && resolvedRunId) {
      setSelectedRunId(resolvedRunId);
    }
  }, [selectedRunId, resolvedRunId, setSelectedRunId]);

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
    const promoteRunRoot = parsed?.type === 'run' && stageList.length > 0;
    const needsDefault = runChanged || !selectedNodeId || !nodeMatchesRun || promoteRunRoot;

    if (needsDefault) {
      const latestEvent = eventList.length > 0 ? eventList[eventList.length - 1] : null;
      setSelectedNode(
        stageList.length > 0
          ? defaultStageSelection(resolvedRunId, stageList)
          : latestEvent
            ? eventNodeId(resolvedRunId, latestEvent.event_id || `${latestEvent.seq}-latest`)
            : runNodeId(resolvedRunId),
        stageList.length > 0 ? 'stage' : (latestEvent ? 'event' : 'run')
      );
    }
    prevRunRef.current = resolvedRunId;
  }, [
    resolvedRunId,
    selectedNodeId,
    isSelectionPinned,
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
      const result = params.specText && params.specText.trim().length > 0
        ? await launchRunFromSpecText({
            specText: params.specText,
            runId: params.runId,
            file: params.file,
            specFormat: params.specText.trimStart().startsWith('{') ? 'json' : 'yaml'
          })
        : await launchMutation.mutateAsync({ file: params.file, runId: params.runId });
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

  const onSelectEvent = (event: { event_id: string; seq: number }) => {
    if (!resolvedRunId) return;
    const ref = (event.event_id ?? '').trim();
    setSelectedNode(eventNodeId(resolvedRunId, ref.length > 0 ? ref : `${event.seq}`), 'event');
  };

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="logo" aria-label="Void Control logo">
            <span className="logo-v">V</span>
            <span className="logo-seven">7</span>
          </span>
          <div>
            <div className="brand-name">void-control</div>
            <div className="brand-sub">orchestration explorer</div>
          </div>
        </div>
        <div className="meta">daemon: {baseUrl}</div>
      </header>

      <main className="layout">
        <RunsList
          activeRuns={filteredActiveRuns}
          terminalRuns={filteredTerminalRuns}
          selectedRunId={resolvedRunId}
          onSelect={setSelectedRunId}
          onLaunch={() => {
            setLaunchError(null);
            setIsLaunchOpen(true);
          }}
          hideTestRuns={hideTestRuns}
          onToggleHideTestRuns={() => setHideTestRuns((v) => !v)}
        />

        <section className="detail-panel">
          {(listError || detailError) && (
            <div className="error-banner">
              <strong>Connection error:</strong> {listError ?? detailError}
            </div>
          )}
          <div className="toolbar">
            <div>
              <strong>Run:</strong> {resolvedRunId ?? '-'}
              <span className="state-pill">{(runDetail.data?.status ?? runDetail.data?.state ?? 'unknown').toString()}</span>
            </div>
            <div className="ops-actions">
              <button
                className="danger"
                disabled={!resolvedRunId || cancelMutation.isPending}
                onClick={() => cancelMutation.mutate()}
              >
                Cancel Run
              </button>
            </div>
          </div>

          {!resolvedRunId ? (
            <div className="empty">No runs yet. Start one from terminal and refresh.</div>
          ) : (
            <div className="detail-grid">
              <div className="center-panel">
                {(eventList.length === 0 && (stages.data ?? []).length === 0) ? (
                  <div className="graph-box graph-empty">
                    <div className="panel-title">Execution Graph</div>
                    <div className="empty">No stages/events found for this run yet.</div>
                  </div>
                ) : (
                  <RunGraph
                    runId={resolvedRunId}
                    events={scopedEvents}
                    stages={normalizedStages}
                    selectedNodeId={selectedNodeId}
                    onSelectNode={setSelectedNode}
                  />
                )}
                <EventTimeline
                  events={scopedEvents}
                  telemetry={telemetry.data ?? []}
                  selectedEventRef={selectedEventRef}
                  onSelectEvent={onSelectEvent}
                />
                <RunLogs
                  events={scopedEvents}
                  selectedEventRef={selectedEventRef}
                  onSelectEvent={onSelectEvent}
                />
              </div>

              <NodeInspector
                runId={resolvedRunId}
                selectedNodeId={selectedNodeId}
                selectedNodeType={selectedNodeType}
                isPinned={isSelectionPinned}
                stages={normalizedStages}
                events={eventList}
                telemetry={telemetry.data ?? []}
                onClearSelection={() => {
                  clearSelectedNode();
                  setSelectedNode(runNodeId(resolvedRunId), 'run');
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
