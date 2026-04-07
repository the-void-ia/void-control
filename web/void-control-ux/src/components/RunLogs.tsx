import { useEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import type { ExecutionEvent, RunEvent } from '../lib/types';

interface RunLogsProps {
  events: Array<RunEvent | ExecutionEvent>;
  selectedEventRef?: string | null;
  onSelectEvent?: (event: RunEvent | ExecutionEvent) => void;
}

function formatTime(ts?: string): string {
  if (!ts) return '--:--:--';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return '--:--:--';
  return d.toLocaleTimeString();
}

function isRuntimeEvent(event: RunEvent | ExecutionEvent): event is RunEvent {
  return 'run_id' in event;
}

function eventRef(event: RunEvent | ExecutionEvent): string {
  if (isRuntimeEvent(event)) return event.event_id || `${event.seq}`;
  return `${event.seq}`;
}

function eventTypeLabel(event: RunEvent | ExecutionEvent): string {
  if (isRuntimeEvent(event)) return event.event_type_v2 ?? event.event_type;
  return event.event_type;
}

function eventMessage(event: RunEvent | ExecutionEvent): string {
  if (isRuntimeEvent(event)) return event.message ?? '';
  return event.message ?? event.event_type.replace(/([a-z0-9])([A-Z])/g, '$1 $2');
}

export function RunLogs({ events, selectedEventRef = null, onSelectEvent }: RunLogsProps) {
  const [panelHeight, setPanelHeight] = useState(162);
  const dragStateRef = useRef<{ startY: number; startHeight: number } | null>(null);
  const rows = events.slice(-8).reverse();

  useEffect(() => {
    function onPointerMove(event: PointerEvent) {
      const dragState = dragStateRef.current;
      if (!dragState) return;
      const delta = dragState.startY - event.clientY;
      setPanelHeight(Math.max(118, Math.min(300, dragState.startHeight + delta)));
    }

    function onPointerUp() {
      dragStateRef.current = null;
      document.body.classList.remove('is-resizing-logs');
    }

    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    return () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
    };
  }, []);

  function onResizeStart(event: ReactPointerEvent<HTMLButtonElement>) {
    dragStateRef.current = { startY: event.clientY, startHeight: panelHeight };
    document.body.classList.add('is-resizing-logs');
  }

  return (
    <div className="run-logs-panel" style={{ height: `${panelHeight}px` }}>
      <button
        type="button"
        className="run-logs-resize-handle"
        aria-label="Resize event log panel"
        onPointerDown={onResizeStart}
      />
      <div className="run-logs-track" />
      <div className="run-logs-table">
        {rows.map((event, idx) => {
          const type = eventTypeLabel(event);
          const isError = ('level' in event && event.level === 'error') || /failed|error/i.test(type);
          const ref = eventRef(event);
          return (
            <button
              className={`run-log-row ${(selectedEventRef && ref === selectedEventRef) ? 'run-log-row-selected' : ''}`}
              key={`${ref}-${idx}`}
              onClick={() => onSelectEvent?.(event)}
            >
              <span className="run-log-time">{isRuntimeEvent(event) ? formatTime(event.timestamp) : `#${event.seq}`}</span>
              <span className={`run-log-type ${isError ? 'run-log-type-error' : ''}`}>{type}</span>
              <span className="run-log-msg">{eventMessage(event)}</span>
            </button>
          );
        })}
        {rows.length === 0 && <div className="run-log-empty">No events yet.</div>}
      </div>
    </div>
  );
}
