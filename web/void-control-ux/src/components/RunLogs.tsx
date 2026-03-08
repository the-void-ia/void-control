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
  const rows = events.slice(-8).reverse();
  return (
    <div className="run-logs-panel">
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
