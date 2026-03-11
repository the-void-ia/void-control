import ReactECharts from 'echarts-for-react';
import type { EChartsOption } from 'echarts';
import type { RunEvent, TelemetrySample } from '../lib/types';
import { rollingEventsPerSec } from '../lib/selectors';

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
  const eventsPerSec = rollingEventsPerSec(events);

  const hasTelemetry = telemetry.length > 0;
  const samples = telemetry.slice(-32);
  const fallbackLen = Math.max(10, Math.min(24, events.length || 12));
  const chartLabels = hasTelemetry
    ? samples.map((s) => `#${s.seq}`)
    : Array.from({ length: fallbackLen }, (_, i) => `#${i + 1}`);
  const chartData = hasTelemetry
    ? samples.map((s) => Number((s.host?.cpu_percent ?? 0).toFixed(2)))
    : Array.from({ length: fallbackLen }, () => 0);

  const telemetryOption: EChartsOption = {
    ...chartBase(),
    xAxis: {
      ...(chartBase().xAxis as object),
      data: chartLabels
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
        lineStyle: { width: 2, color: hasTelemetry ? '#22d3ee' : '#334155', type: hasTelemetry ? 'solid' : 'dashed' },
        areaStyle: hasTelemetry
          ? {
              color: {
                type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                colorStops: [
                  { offset: 0, color: 'rgba(34, 211, 238, 0.35)' },
                  { offset: 1, color: 'rgba(34, 211, 238, 0.03)' }
                ]
              }
            }
          : undefined,
        data: chartData
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
        <div className="telemetry-card" title="Events/s (rolling 30s)">
          <span>events/s</span>
          <strong>{eventsPerSec.toFixed(1)}</strong>
        </div>
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
        <ReactECharts option={telemetryOption} style={{ height: 170, width: '100%' }} notMerge lazyUpdate />
      </div>

    </div>
  );
}
