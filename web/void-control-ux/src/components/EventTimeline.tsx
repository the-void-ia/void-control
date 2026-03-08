import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { RunEvent, TelemetrySample } from '../lib/types';

interface EventTimelineProps {
  events: RunEvent[];
  telemetry?: TelemetrySample[];
  selectedEventRef?: string | null;
  onSelectEvent?: (event: RunEvent) => void;
}

function chartBase(): Pick<EChartsOption, 'grid' | 'xAxis' | 'yAxis' | 'tooltip'> {
  return {
    grid: { left: 36, right: 12, top: 14, bottom: 28 },
    xAxis: {
      type: 'category',
      axisLine: { lineStyle: { color: '#334155' } },
      axisLabel: { color: '#94a3b8', fontSize: 11 }
    },
    yAxis: {
      type: 'value',
      axisLine: { show: false },
      splitLine: { lineStyle: { color: '#1f2937', type: 'dashed' } },
      axisLabel: { color: '#94a3b8', fontSize: 11 }
    },
    tooltip: {
      trigger: 'axis',
      backgroundColor: '#0b1222',
      borderColor: '#334155',
      textStyle: { color: '#e2e8f0' }
    }
  };
}

export function EventTimeline({
  events,
  telemetry = [],
  selectedEventRef = null,
  onSelectEvent
}: EventTimelineProps) {
  const latest = telemetry.length > 0 ? telemetry[telemetry.length - 1] : null;
  const latestHostCpu = latest?.host?.cpu_percent ?? 0;
  const latestHostRssMb = (latest?.host?.rss_bytes ?? 0) / (1024 * 1024);
  const latestGuestCpu = latest?.guest?.cpu_percent ?? 0;
  const latestGuestMemMb = (latest?.guest?.memory_used_bytes ?? 0) / (1024 * 1024);

  const hasTelemetry = telemetry.length > 0;

  const telemetryOption: EChartsOption = {
    ...chartBase(),
    xAxis: {
      ...(chartBase().xAxis as object),
      data: telemetry.slice(-32).map((s) => `#${s.seq}`)
    },
    yAxis: {
      ...(chartBase().yAxis as object),
      max: 100
    },
    series: [
      {
        name: 'host cpu',
        type: 'line',
        smooth: true,
        showSymbol: false,
        lineStyle: { width: 2, color: '#22d3ee' },
        areaStyle: {
          color: {
            type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: 'rgba(34, 211, 238, 0.35)' },
              { offset: 1, color: 'rgba(34, 211, 238, 0.03)' }
            ]
          }
        },
        data: telemetry.slice(-32).map((s) => Number((s.host?.cpu_percent ?? 0).toFixed(2)))
      }
    ]
  };

  const recent = events.slice(-25);
  const eventGapData = recent.map((e, idx) => {
    const currentTs = Date.parse(e.timestamp ?? '');
    const prevTs = idx > 0 ? Date.parse(recent[idx - 1]?.timestamp ?? '') : NaN;
    return Number.isFinite(currentTs) && Number.isFinite(prevTs)
      ? Math.max(0, currentTs - prevTs)
      : 0;
  });

  const eventsOption: EChartsOption = {
    ...chartBase(),
    xAxis: {
      ...(chartBase().xAxis as object),
      data: recent.map((e) => `#${e.seq}`)
    },
    series: [
      {
        name: 'event gap ms',
        type: 'bar',
        itemStyle: { color: '#22d3ee', borderRadius: [3, 3, 0, 0] },
        data: eventGapData
      }
    ]
  };

  const stageCpuMap = new Map<string, { sum: number; count: number }>();
  for (const sample of telemetry) {
    const cpu = sample.host?.cpu_percent;
    if (typeof cpu !== 'number') continue;
    const stage = sample.stage_name || 'unknown';
    const entry = stageCpuMap.get(stage) ?? { sum: 0, count: 0 };
    entry.sum += cpu;
    entry.count += 1;
    stageCpuMap.set(stage, entry);
  }

  const stageCpu = [...stageCpuMap.entries()]
    .map(([stage, v]) => ({ stage, avgCpu: v.count > 0 ? v.sum / v.count : 0 }))
    .sort((a, b) => b.avgCpu - a.avgCpu)
    .slice(0, 4);

  return (
    <div className="timeline-box">
      <div className="panel-title">Telemetry Timeline (Host CPU %)</div>
      <div className="telemetry-cards">
        <div className="telemetry-card"><span>host cpu</span><strong>{latestHostCpu.toFixed(1)}%</strong></div>
        <div className="telemetry-card"><span>host rss</span><strong>{latestHostRssMb.toFixed(1)} MB</strong></div>
        <div className="telemetry-card"><span>guest cpu</span><strong>{latestGuestCpu.toFixed(1)}%</strong></div>
        <div className="telemetry-card"><span>guest mem</span><strong>{latestGuestMemMb.toFixed(1)} MB</strong></div>
      </div>

      {stageCpu.length > 0 && (
        <div className="telemetry-stage-strip">
          {stageCpu.map((s) => (
            <span key={s.stage} className="stage-chip">{s.stage}: {s.avgCpu.toFixed(1)}%</span>
          ))}
        </div>
      )}

      {!hasTelemetry && <div className="telemetry-empty">No telemetry samples yet for this run.</div>}

      <div className="timeline-chart">
        <ReactECharts option={hasTelemetry ? telemetryOption : eventsOption} style={{ height: 220, width: '100%' }} notMerge lazyUpdate />
      </div>

      <div className="event-list">
        {events.slice(-12).reverse().map((event, idx) => (
          <button
            key={(event.event_id && event.event_id.length > 0) ? event.event_id : `${event.seq}-${idx}`}
            className={`event-row ${(selectedEventRef && (event.event_id === selectedEventRef || `${event.seq}` === selectedEventRef)) ? 'event-row-selected' : ''}`}
            style={{ animationDelay: `${Math.min(idx * 40, 320)}ms` }}
            onClick={() => onSelectEvent?.(event)}
          >
            <span className="event-seq">#{event.seq}</span>
            <span className="event-type">{event.event_type_v2 ?? event.event_type}</span>
            <span className="event-msg">{event.message ?? ''}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
