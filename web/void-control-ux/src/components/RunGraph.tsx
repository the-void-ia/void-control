import { useEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import Graph from 'graphology';
import Sigma from 'sigma';
import type { RunEvent, StageView } from '../lib/types';
import { eventNodeId, parseNodeId, runNodeId, stageNodeId, type SelectedNodeType } from '../lib/selectors';

interface RunGraphProps {
  runId: string;
  events: RunEvent[];
  stages?: StageView[];
  selectedNodeId?: string | null;
  onSelectNode?: (nodeId: string, nodeType: Exclude<SelectedNodeType, null>) => void;
}

function compactType(t: string): string {
  return t.replace(/\./g, ' ').replace(/([A-Z])/g, ' $1').trim();
}

function groupIndex(groupId: string): number {
  const m = /^g(\d+)$/.exec(groupId);
  return m ? Number(m[1]) : 9999;
}

function stageColor(status: string): string {
  if (status === 'running') return '#38bdf8';
  if (status === 'succeeded') return '#22c55e';
  if (status === 'failed') return '#ef4444';
  if (status === 'queued') return '#94a3b8';
  if (status === 'skipped') return '#64748b';
  return '#94a3b8';
}

function drawRoundedRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.lineTo(x + w - radius, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + radius);
  ctx.lineTo(x + w, y + h - radius);
  ctx.quadraticCurveTo(x + w, y + h, x + w - radius, y + h);
  ctx.lineTo(x + radius, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - radius);
  ctx.lineTo(x, y + radius);
  ctx.quadraticCurveTo(x, y, x + radius, y);
  ctx.closePath();
}

function drawAgentBoxAnchor(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  radius: number,
  nodeColor: string,
  selected: boolean
) {
  const box = Math.max(10.5, radius * 1.7);
  const outer = box + (selected ? 3.2 : 1.8);
  const inner = box * 0.58;

  ctx.save();
  ctx.translate(x, y);

  ctx.save();
  ctx.shadowColor = '#22d3ee';
  ctx.shadowBlur = selected ? 34 : 20;
  ctx.fillStyle = selected ? '#22d3ee' : blendHex(nodeColor, 0.26);
  drawRoundedRect(ctx, -outer, -outer, outer * 2, outer * 2, outer * 0.34);
  ctx.globalAlpha = 0.14;
  ctx.fill();
  ctx.restore();

  const shellGrad = ctx.createLinearGradient(-outer, -outer, outer, outer);
  shellGrad.addColorStop(0, blendHex(nodeColor, 0.55));
  shellGrad.addColorStop(0.5, '#0c1930');
  shellGrad.addColorStop(1, '#93c5fd');
  ctx.fillStyle = shellGrad;
  ctx.strokeStyle = selected ? '#a5f3fc' : '#7dd3fc';
  ctx.lineWidth = selected ? 2 : 1.3;
  drawRoundedRect(ctx, -outer, -outer, outer * 2, outer * 2, outer * 0.34);
  ctx.fill();
  ctx.stroke();

  const coreGrad = ctx.createLinearGradient(-inner, -inner, inner, inner);
  coreGrad.addColorStop(0, '#e0fbff');
  coreGrad.addColorStop(0.55, blendHex(nodeColor, 0.18));
  coreGrad.addColorStop(1, '#061428');
  ctx.fillStyle = coreGrad;
  ctx.beginPath();
  ctx.moveTo(-inner * 0.1, -inner);
  ctx.lineTo(inner * 0.86, -inner * 0.16);
  ctx.lineTo(inner * 0.46, inner);
  ctx.lineTo(-inner * 0.72, inner * 0.58);
  ctx.lineTo(-inner, -inner * 0.28);
  ctx.closePath();
  ctx.fill();
  ctx.strokeStyle = 'rgba(217, 249, 255, 0.5)';
  ctx.lineWidth = 1;
  ctx.stroke();

  ctx.fillStyle = 'rgba(236, 254, 255, 0.42)';
  ctx.beginPath();
  ctx.moveTo(-inner * 0.16, -inner * 0.9);
  ctx.lineTo(inner * 0.56, -inner * 0.08);
  ctx.lineTo(-inner * 0.06, inner * 0.08);
  ctx.closePath();
  ctx.fill();

  ctx.fillStyle = 'rgba(236, 254, 255, 0.78)';
  ctx.beginPath();
  ctx.arc(inner * 0.1, inner * 0.02, Math.max(1.2, box * 0.13), 0, Math.PI * 2);
  ctx.fill();

  ctx.restore();
}

function drawNodeCardLabel(ctx: CanvasRenderingContext2D, data: Record<string, unknown>) {
  const label = typeof data.label === 'string' ? data.label : '';
  if (!label) return;
  const x = Number(data.x ?? 0);
  const y = Number(data.y ?? 0);
  const nodeColor = typeof data.color === 'string' ? data.color : '#64748b';
  const status = typeof data.status === 'string' ? data.status : '';
  const kind = typeof data.kind === 'string' ? data.kind : 'stage';
  const selected = Boolean(data.selected);
  const lines = label.split('\n');
  const title = lines[0] ?? '';
  const sub = lines[1] ?? '';
  const titleColor = '#e6eefc';
  const subColor = status === 'succeeded'
    ? '#86efac'
    : status === 'failed'
      ? '#fca5a5'
      : status === 'running'
        ? '#93c5fd'
        : '#94a3b8';

  const isRun = kind === 'run';
  const pointRadius = isRun ? 11.5 : 10.2;
  const runCardHeight = sub ? 28 : 22;

  ctx.save();
  ctx.font = '600 11px Space Grotesk, sans-serif';
  const titleWidth = ctx.measureText(title).width;
  ctx.font = '700 10px Space Grotesk, sans-serif';
  const subWidth = sub ? ctx.measureText(sub).width : 0;
  const width = Math.max(titleWidth, subWidth) + 30;
  const left = isRun ? x + 8 : x - width / 2;
  const top = isRun ? y - pointRadius - runCardHeight - 10 : y - 38;

  const innerFill = selected
    ? 'rgba(9, 20, 38, 0.97)'
    : 'rgba(8, 18, 34, 0.93)';
  const stroke = selected ? blendHex(nodeColor, 0.3) : nodeColor;

  if (isRun) {
    ctx.save();
    ctx.shadowColor = nodeColor;
    ctx.shadowBlur = 34;
    drawRoundedRect(ctx, left - 2.5, top - 2.5, width + 5, runCardHeight + 5, 10);
    ctx.strokeStyle = stroke;
    ctx.lineWidth = 1.5;
    ctx.globalAlpha = 0.85;
    ctx.stroke();
    ctx.restore();
  }

  if (kind === 'stage') {
    drawAgentBoxAnchor(ctx, x, y, pointRadius, nodeColor, selected);
    ctx.font = '600 11px Space Grotesk, sans-serif';
    ctx.fillStyle = titleColor;
    const titleTextWidth = ctx.measureText(title).width;
    const titleLeft = x - titleTextWidth / 2;
    drawRoundedRect(ctx, titleLeft - 8, y - 54, titleTextWidth + 16, 22, 8);
    ctx.fillStyle = 'rgba(8, 18, 34, 0.9)';
    ctx.fill();
    ctx.strokeStyle = selected ? blendHex(nodeColor, 0.22) : 'rgba(125, 211, 252, 0.38)';
    ctx.lineWidth = 1;
    ctx.globalAlpha = 0.88;
    ctx.stroke();
    ctx.globalAlpha = 1;
    ctx.save();
    ctx.shadowColor = selected ? nodeColor : 'rgba(125, 211, 252, 0.42)';
    ctx.shadowBlur = selected ? 14 : 8;
    ctx.fillStyle = titleColor;
    ctx.fillText(title, titleLeft, y - 40);
    ctx.restore();

    if (sub) {
      ctx.font = '700 10px Space Grotesk, sans-serif';
      ctx.fillStyle = subColor;
      const subTextWidth = ctx.measureText(sub).width;
      ctx.fillText(sub, x - subTextWidth / 2, y + 36);
    }
  } else {
    // run/event keep circular anchor
    drawRoundedRect(ctx, left, top, width, runCardHeight, 8);
    ctx.fillStyle = innerFill;
    ctx.fill();
    ctx.strokeStyle = stroke;
    ctx.lineWidth = selected ? 1.35 : 1.1;
    ctx.globalAlpha = selected ? 0.95 : 0.78;
    ctx.stroke();
    ctx.globalAlpha = 1;

    ctx.strokeStyle = 'rgba(148,163,184,0.62)';
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(x, y - pointRadius);
    ctx.lineTo(x, top + runCardHeight);
    ctx.stroke();

    ctx.save();
    ctx.shadowColor = nodeColor;
    ctx.shadowBlur = selected ? 20 : 10;
    ctx.fillStyle = nodeColor;
    ctx.beginPath();
    ctx.arc(x, y, pointRadius, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();

    ctx.fillStyle = '#f8fafc';
    ctx.beginPath();
    ctx.arc(x, y, Math.max(1.6, pointRadius * 0.36), 0, Math.PI * 2);
    ctx.fill();
    ctx.font = '600 11px Space Grotesk, sans-serif';
    ctx.fillStyle = titleColor;
    ctx.fillText(title, left + 10, top + (sub ? 12 : 14));

    if (sub) {
      ctx.font = '700 10px Space Grotesk, sans-serif';
      ctx.fillStyle = subColor;
      ctx.fillText(sub, left + 10, top + 23);
    }
  }
  ctx.restore();
}

function blendHex(base: string, boost: number): string {
  const b = base.replace('#', '');
  if (b.length !== 6) return base;
  const r = parseInt(b.slice(0, 2), 16);
  const g = parseInt(b.slice(2, 4), 16);
  const bl = parseInt(b.slice(4, 6), 16);
  const k = Math.max(0, Math.min(0.45, boost));
  const nr = Math.min(255, Math.round(r + (255 - r) * k));
  const ng = Math.min(255, Math.round(g + (255 - g) * k));
  const nb = Math.min(255, Math.round(bl + (255 - bl) * k));
  return `#${nr.toString(16).padStart(2, '0')}${ng.toString(16).padStart(2, '0')}${nb.toString(16).padStart(2, '0')}`;
}

function buildGraph(runId: string, events: RunEvent[], stages: StageView[], selectedNodeId: string | null): Graph {
  const graph = new Graph();
  const parsed = parseNodeId(selectedNodeId);
  const selectedStageName = parsed?.type === 'stage' ? parsed.stageName : null;

  const dependencies = new Set<string>();
  if (selectedStageName) {
    const stage = stages.find((s) => s.stage_name === selectedStageName);
    if (stage) {
      dependencies.add(stage.stage_name);
      for (const dep of stage.depends_on) dependencies.add(dep);
      for (const candidate of stages) {
        if (candidate.depends_on.includes(stage.stage_name)) dependencies.add(candidate.stage_name);
      }
    }
  }

  const runIdNode = runNodeId(runId);
  graph.addNode(runIdNode, {
    label: `Run ${runId}`,
    x: 1.8,
    y: 0,
    size: 24,
    color: '#0ea5e9',
    kind: 'run'
  });

  if (stages.length > 0) {
    const sorted = [...stages].sort((a, b) => {
      const g = groupIndex(a.group_id) - groupIndex(b.group_id);
      return g !== 0 ? g : a.stage_name.localeCompare(b.stage_name);
    });

    const groups = new Map<string, StageView[]>();
    for (const s of sorted) {
      const arr = groups.get(s.group_id) ?? [];
      arr.push(s);
      groups.set(s.group_id, arr);
    }

    const nameToId = new Map<string, string>();
    const groupEntries = [...groups.entries()].sort((a, b) => groupIndex(a[0]) - groupIndex(b[0]));

    for (const [groupId, groupStages] of groupEntries) {
      const gi = groupIndex(groupId);
      const x = 4 + gi * 2.7;
      groupStages.forEach((stage, idx) => {
        const center = 0;
        const spread = 1.75;
        const y = center + (idx - (groupStages.length - 1) / 2) * spread;
        const nodeId = stageNodeId(runId, stage.stage_name, sorted.indexOf(stage));
        nameToId.set(stage.stage_name, nodeId);

        const isSelected = nodeId === selectedNodeId;
        const isRelated = selectedStageName ? dependencies.has(stage.stage_name) : true;

      graph.addNode(nodeId, {
        label: `${stage.stage_name}\n${stage.status}`,
        x,
        y,
        size: isSelected ? 16 : 11,
        color: stageColor(stage.status),
        kind: 'stage',
        stageName: stage.stage_name,
        status: stage.status,
        selected: isSelected,
        forceLabel: true,
        alpha: isRelated ? 1 : 0.28
      });
      });
    }

    for (const stage of sorted) {
      const target = nameToId.get(stage.stage_name);
      if (!target) continue;

      if (stage.depends_on.length === 0) {
        const highlighted = selectedStageName ? selectedStageName === stage.stage_name : false;
        graph.addEdge(runIdNode, target, {
          size: highlighted ? 3.1 : 1.35,
          color: highlighted ? blendHex('#60a5fa', 0.18) : '#64748b',
          type: highlighted ? 'arrow' : 'line',
          alpha: selectedStageName && !highlighted ? 0.22 : 0.85
        });
        continue;
      }

      for (const dep of stage.depends_on) {
        const source = nameToId.get(dep);
        if (!source) continue;
        const isUpstream = selectedStageName ? stage.stage_name === selectedStageName : false;
        const isDownstream = selectedStageName ? dep === selectedStageName : false;
        const highlighted = isUpstream || isDownstream;
        const baseColor = isUpstream ? '#fb923c' : isDownstream ? '#60a5fa' : '#64748b';
        graph.addEdge(source, target, {
          size: highlighted ? 3.1 : 1.35,
          color: highlighted ? blendHex(baseColor, 0.22) : baseColor,
          type: highlighted ? 'arrow' : 'line',
          alpha: selectedStageName && !highlighted ? 0.2 : 0.85
        });
      }
    }
  } else {
    const usedIds = new Set<string>([runIdNode]);
    const nodes: string[] = [];

    for (const [index, ev] of events.slice(0, 40).entries()) {
      const raw = typeof ev.event_id === 'string' ? ev.event_id.trim() : '';
      let id = eventNodeId(runId, raw.length > 0 ? raw : `${ev.seq ?? index}-${index}`);
      if (usedIds.has(id)) {
        let suffix = 1;
        while (usedIds.has(`${id}-${suffix}`)) suffix += 1;
        id = `${id}-${suffix}`;
      }
      usedIds.add(id);
      nodes.push(id);

      const x = 3 + index * 1.45;
      const y = Math.sin(index * 0.7) * 0.32;

      graph.addNode(id, {
        label: `${compactType(ev.event_type_v2 ?? ev.event_type)}\n#${ev.seq}`,
        x,
        y,
        size: id === selectedNodeId ? 11.5 : 8.5,
        color: ev.level === 'error' ? '#ef4444' : '#475569',
        kind: 'event',
        selected: id === selectedNodeId,
        forceLabel: true,
        alpha: id === selectedNodeId ? 1 : 0.9
      });
    }

    let prev = runIdNode;
    for (const id of nodes) {
      const highlighted = id === selectedNodeId;
      graph.addEdge(prev, id, {
        size: highlighted ? 2.4 : 1.2,
        color: highlighted ? '#93c5fd' : '#64748b',
        type: highlighted ? 'arrow' : 'line',
        alpha: highlighted ? 1 : 0.7
      });
      prev = id;
    }
  }

  return graph;
}

export function RunGraph({ runId, events, stages = [], selectedNodeId = null, onSelectNode }: RunGraphProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const particleCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<Sigma | null>(null);
  const lastCenteredNodeRef = useRef<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [hoverTooltip, setHoverTooltip] = useState<{ text: string; x: number; y: number } | null>(null);
  const selectedStageName = useMemo(() => {
    const parsed = parseNodeId(selectedNodeId);
    return parsed?.type === 'stage' ? parsed.stageName : null;
  }, [selectedNodeId]);
  const graph = useMemo(
    () => buildGraph(runId, events, stages, selectedNodeId),
    [runId, events, stages, selectedNodeId]
  );

  const kpis = useMemo(() => {
    const activeBoxes = stages.length > 0 ? stages.filter((s) => s.status === 'running').length : 1;
    const creating = stages.length > 0 ? stages.filter((s) => s.status === 'queued').length : 0;
    const failedCount = stages.filter((s) => s.status === 'failed').length;
    const healthy = failedCount === 0;
    return { activeBoxes, creating, healthy };
  }, [stages]);

  useEffect(() => {
    lastCenteredNodeRef.current = null;
  }, [runId]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const renderer = new Sigma(graph, container, {
      renderEdgeLabels: false,
      labelRenderedSizeThreshold: 6,
      labelDensity: 0.85,
      defaultNodeColor: '#94a3b8',
      defaultEdgeColor: '#64748b',
      labelColor: { color: '#dbeafe' },
      labelSize: 13,
      defaultDrawNodeLabel: drawNodeCardLabel,
      defaultEdgeType: 'line',
      nodeReducer: (node, attrs) => {
        const out = { ...attrs } as Record<string, unknown>;
        if (typeof attrs.alpha === 'number') {
          out.zIndex = attrs.alpha > 0.5 ? 1 : 0;
        }
        return out;
      },
      edgeReducer: (_edge, attrs) => {
        const out = { ...attrs } as Record<string, unknown>;
        if (typeof attrs.alpha === 'number') {
          out.hidden = attrs.alpha < 0.16;
          out.zIndex = attrs.alpha > 0.5 ? 2 : 0;
        }
        return out;
      }
    });

    renderer.getCamera().animatedReset();
    renderer.on('clickNode', ({ node }) => {
      const parsed = parseNodeId(node);
      if (!parsed || !onSelectNode) return;
      onSelectNode(node, parsed.type);
    });
    renderer.on('enterNode', ({ node, event }) => {
      const attrs = graph.getNodeAttributes(node) as Record<string, unknown>;
      const label = typeof attrs.label === 'string' ? attrs.label.replace('\n', ' - ') : node;
      setHoverTooltip({ text: label, x: event.x + 10, y: event.y + 12 });
    });
    renderer.on('leaveNode', () => setHoverTooltip(null));
    renderer.on('enterEdge', ({ edge, event }) => {
      const source = graph.source(edge);
      const target = graph.target(edge);
      const sourceLabel = String(graph.getNodeAttribute(source, 'label') ?? source).replace('\n', ' ');
      const targetLabel = String(graph.getNodeAttribute(target, 'label') ?? target).replace('\n', ' ');
      setHoverTooltip({ text: `${sourceLabel} -> ${targetLabel}`, x: event.x + 10, y: event.y + 12 });
    });
    renderer.on('leaveEdge', () => setHoverTooltip(null));
    renderer.on('leaveStage', () => setHoverTooltip(null));

    renderer.getCamera().on('updated', () => {
      setZoom(Number((1 / renderer.getCamera().ratio).toFixed(2)));
    });

    rendererRef.current = renderer;
    return () => {
      renderer.kill();
      rendererRef.current = null;
    };
  }, [graph, onSelectNode]);

  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer || !selectedNodeId) return;
    const parsed = parseNodeId(selectedNodeId);
    if (!parsed || parsed.type === 'run') return;
    if (!graph.hasNode(selectedNodeId)) return;
    if (lastCenteredNodeRef.current === selectedNodeId) return;

    const attrs = graph.getNodeAttributes(selectedNodeId) as Record<string, unknown>;
    const nx = Number(attrs.x);
    const ny = Number(attrs.y);
    if (!Number.isFinite(nx) || !Number.isFinite(ny)) return;

    const cam = renderer.getCamera();
    const viewport = (renderer as unknown as { graphToViewport: (p: { x: number; y: number }) => { x: number; y: number } })
      .graphToViewport({ x: nx, y: ny });
    const container = containerRef.current;
    const w = container?.clientWidth ?? 0;
    const h = container?.clientHeight ?? 0;
    const marginX = Math.max(80, w * 0.14);
    const marginY = Math.max(80, h * 0.14);
    const outside =
      viewport.x < marginX ||
      viewport.x > (w - marginX) ||
      viewport.y < marginY ||
      viewport.y > (h - marginY);
    if (!outside) {
      lastCenteredNodeRef.current = selectedNodeId;
      return;
    }

    window.requestAnimationFrame(() => {
      cam.animate({ x: nx, y: ny, ratio: cam.ratio }, { duration: 240 });
      lastCenteredNodeRef.current = selectedNodeId;
    });
  }, [selectedNodeId, graph]);

  useEffect(() => {
    const canvas = particleCanvasRef.current;
    const container = containerRef.current;
    const renderer = rendererRef.current;
    if (!canvas || !container || !renderer || stages.length === 0) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const graphObj = graph;
    const idByStage = new Map<string, string>();
    graphObj.forEachNode((id, attrs) => {
      const stageName = typeof attrs.stageName === 'string' ? attrs.stageName : null;
      if (stageName) idByStage.set(stageName, id);
    });

    const activeStageName = selectedStageName ?? stages[0]?.stage_name ?? null;
    if (!activeStageName) return;
    const selected = stages.find((s) => s.stage_name === activeStageName);
    if (!selected) return;

    const paths: Array<{ from: string; to: string; color: string }> = [];
    for (const dep of selected.depends_on) {
      const a = idByStage.get(dep);
      const b = idByStage.get(selected.stage_name);
      if (a && b) paths.push({ from: a, to: b, color: '#fb923c' });
    }
    for (const stage of stages) {
      if (stage.depends_on.includes(selected.stage_name)) {
        const a = idByStage.get(selected.stage_name);
        const b = idByStage.get(stage.stage_name);
        if (a && b) paths.push({ from: a, to: b, color: '#60a5fa' });
      }
    }
    if (paths.length === 0) return;

    let raf = 0;
    const start = performance.now();
    const rnd = Array.from({ length: 28 }, (_, i) => ({
      sx: (i * 37) % 100,
      sy: (i * 67) % 100,
      sp: 0.2 + ((i * 11) % 7) * 0.08
    }));

    const draw = (now: number) => {
      if (document.hidden) {
        raf = window.requestAnimationFrame(draw);
        return;
      }
      const w = container.clientWidth;
      const h = container.clientHeight;
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
      ctx.clearRect(0, 0, w, h);

      const t = (now - start) / 1000;
      ctx.globalCompositeOperation = 'lighter';

      rnd.forEach((p, idx) => {
        const x = ((p.sx + t * p.sp * 12 + idx * 3) % 100) / 100 * w;
        const y = ((p.sy + t * p.sp * 7 + idx * 5) % 100) / 100 * h;
        const r = 1.2 + (idx % 3) * 0.55;
        ctx.fillStyle = 'rgba(96,165,250,0.24)';
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
      });

      paths.forEach((path, pathIndex) => {
        const fromAttrs = graphObj.getNodeAttributes(path.from);
        const toAttrs = graphObj.getNodeAttributes(path.to);
        const from = (renderer as unknown as { graphToViewport: (p: { x: number; y: number }) => { x: number; y: number } })
          .graphToViewport({ x: Number(fromAttrs.x), y: Number(fromAttrs.y) });
        const to = (renderer as unknown as { graphToViewport: (p: { x: number; y: number }) => { x: number; y: number } })
          .graphToViewport({ x: Number(toAttrs.x), y: Number(toAttrs.y) });

        const dx = to.x - from.x;
        const dy = to.y - from.y;
        const colorRgb = path.color === '#fb923c' ? '251,146,60' : '96,165,250';

        const altRgb = path.color === '#fb923c' ? '96,165,250' : '251,146,60';
        const haloGrad = ctx.createLinearGradient(from.x, from.y, to.x, to.y);
        haloGrad.addColorStop(0, `rgba(${colorRgb},0.10)`);
        haloGrad.addColorStop(0.45, `rgba(${altRgb},0.20)`);
        haloGrad.addColorStop(1, `rgba(${colorRgb},0.10)`);

        // broad dual-color halo
        ctx.save();
        ctx.strokeStyle = haloGrad;
        ctx.lineWidth = 10;
        ctx.shadowColor = `rgba(${colorRgb},0.45)`;
        ctx.shadowBlur = 22;
        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        ctx.lineTo(to.x, to.y);
        ctx.stroke();
        ctx.restore();

        // crisp inner line
        ctx.save();
        ctx.strokeStyle = `rgba(${colorRgb},0.62)`;
        ctx.lineWidth = 2.6;
        ctx.shadowColor = `rgba(${colorRgb},0.82)`;
        ctx.shadowBlur = 12;
        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        ctx.lineTo(to.x, to.y);
        ctx.stroke();
        ctx.restore();

        // moving streaks (flow vectors)
        for (let i = 0; i < 4; i += 1) {
          const phase = (t * 0.52 + pathIndex * 0.23 + i * 0.28) % 1;
          const tail = Math.max(0, phase - 0.085);
          const x1 = from.x + dx * tail;
          const y1 = from.y + dy * tail;
          const x2 = from.x + dx * phase;
          const y2 = from.y + dy * phase;
          const streakGrad = ctx.createLinearGradient(x1, y1, x2, y2);
          streakGrad.addColorStop(0, `rgba(${colorRgb},0.0)`);
          streakGrad.addColorStop(1, `rgba(${colorRgb},0.78)`);

          ctx.save();
          ctx.strokeStyle = streakGrad;
          ctx.lineWidth = 2.3;
          ctx.lineCap = 'round';
          ctx.beginPath();
          ctx.moveTo(x1, y1);
          ctx.lineTo(x2, y2);
          ctx.stroke();
          ctx.restore();
        }

        for (let i = 0; i < 5; i += 1) {
          const phase = (t * 0.9 + pathIndex * 0.19 + i * 0.24) % 1;
          const x = from.x + dx * phase;
          const y = from.y + dy * phase;
          const radius = 3.2 - i * 0.32;
          const alpha = 0.95 - i * 0.14;

          ctx.fillStyle = `rgba(${colorRgb},${alpha})`;
          ctx.shadowColor = `rgba(${colorRgb},0.8)`;
          ctx.shadowBlur = 12;
          ctx.beginPath();
          ctx.arc(x, y, radius, 0, Math.PI * 2);
          ctx.fill();
        }
      });

      ctx.globalCompositeOperation = 'source-over';
      raf = window.requestAnimationFrame(draw);
    };

    raf = window.requestAnimationFrame(draw);
    return () => {
      window.cancelAnimationFrame(raf);
      ctx.clearRect(0, 0, canvas.width, canvas.height);
    };
  }, [graph, selectedStageName, stages]);

  const zoomIn = () => {
    const renderer = rendererRef.current;
    if (!renderer) return;
    const cam = renderer.getCamera();
    cam.animate({ ratio: cam.ratio / 1.2 }, { duration: 180 });
  };

  const zoomOut = () => {
    const renderer = rendererRef.current;
    if (!renderer) return;
    const cam = renderer.getCamera();
    cam.animate({ ratio: cam.ratio * 1.2 }, { duration: 180 });
  };

  const reset = () => {
    rendererRef.current?.getCamera().animatedReset({ duration: 220 });
  };

  const onGraphKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      zoomIn();
      return;
    }
    if (event.key === '-') {
      event.preventDefault();
      zoomOut();
      return;
    }
    if (event.key === '0') {
      event.preventDefault();
      reset();
    }
  };

  return (
    <div className="graph-box sigma-graph-box">
      <div className="panel-title-row">
        <div className="panel-title">Execution Graph</div>
        <div className="graph-kpi-strip">
          <span>Active Boxes: <strong>{kpis.activeBoxes}</strong></span>
          <span>Creating: <strong>{kpis.creating}</strong></span>
          <span className={kpis.healthy ? 'kpi-succeeded' : 'kpi-failed'}><strong>{kpis.healthy ? 'HEALTHY' : 'DEGRADED'}</strong></span>
        </div>
        <div className="graph-actions">
          <button onClick={zoomIn}>+</button>
          <button onClick={zoomOut}>-</button>
          <button onClick={reset}>Reset</button>
          <span>{zoom.toFixed(2)}x</span>
        </div>
      </div>

      <div className="sigma-wrap" tabIndex={0} onKeyDown={onGraphKeyDown} title="Shortcuts: +, -, 0">
        <div ref={containerRef} className="sigma-canvas" />
        <canvas ref={particleCanvasRef} className="particle-layer" />
        {hoverTooltip && (
          <div className="graph-tooltip" style={{ left: hoverTooltip.x, top: hoverTooltip.y }}>
            {hoverTooltip.text}
          </div>
        )}
      </div>
    </div>
  );
}
