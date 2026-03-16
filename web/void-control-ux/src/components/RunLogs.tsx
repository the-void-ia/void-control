import { useEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import type { RunEvent } from '../lib/types';

interface RunLogsProps {
  events: RunEvent[];
  selectedEventRef?: string | null;
  onSelectEvent?: (event: RunEvent) => void;
}

function formatTime(ts?: string): string {
  if (!ts) return '--:--:--';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return '--:--:--';
  return d.toLocaleTimeString();
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
          const type = event.event_type_v2 ?? event.event_type;
          const isError = event.level === 'error' || /failed|error/i.test(type);
          return (
            <button
              className={`run-log-row ${(selectedEventRef && (event.event_id === selectedEventRef || `${event.seq}` === selectedEventRef)) ? 'run-log-row-selected' : ''}`}
              key={`${event.event_id || event.seq}-${idx}`}
              onClick={() => onSelectEvent?.(event)}
            >
              <span className="run-log-time">{formatTime(event.timestamp)}</span>
              <span className={`run-log-type ${isError ? 'run-log-type-error' : ''}`}>{type}</span>
              <span className="run-log-msg">{event.message ?? ''}</span>
            </button>
          );
        })}
        {rows.length === 0 && <div className="run-log-empty">No events yet.</div>}
      </div>
    </div>
  );
}
