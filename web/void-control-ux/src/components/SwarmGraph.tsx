import { useEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent } from 'react';
import Graph from 'graphology';
import Sigma from 'sigma';
import type { SwarmCandidateCard, SwarmExecutionSummary, SwarmIterationSummary } from '../lib/types';

interface SwarmGraphProps {
  summary: SwarmExecutionSummary;
  iterations: SwarmIterationSummary[];
  candidates: SwarmCandidateCard[];
  selectedCandidateId: string | null;
  onSelectCandidate: (candidateId: string) => void;
}

function stateColor(state: SwarmCandidateCard['state']): string {
  switch (state) {
    case 'best':
      return '#22c55e';
    case 'failed':
    case 'canceled':
      return '#ef4444';
    case 'running':
      return '#38bdf8';
    case 'queued':
      return '#94a3b8';
    default:
      return '#60a5fa';
  }
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

function drawRunCoreAnchor(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  radius: number,
  status: string,
  selected: boolean
) {
  const stateColor =
    status === 'failed'
      ? '#ef4444'
      : status === 'succeeded' || status === 'completed'
        ? '#22c55e'
        : status === 'queued'
          ? '#94a3b8'
          : '#38bdf8';
  const outer = Math.max(12.5, radius * 1.2);
  const inner = outer * 0.62;

  ctx.save();
  ctx.translate(x, y);

  ctx.save();
  ctx.shadowColor = stateColor;
  ctx.shadowBlur = selected ? 28 : 18;
  ctx.fillStyle = blendHex(stateColor, 0.1);
  ctx.beginPath();
  ctx.arc(0, 0, outer + 2, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();

  const shellGrad = ctx.createRadialGradient(-outer * 0.2, -outer * 0.4, outer * 0.2, 0, 0, outer);
  shellGrad.addColorStop(0, '#dff7ff');
  shellGrad.addColorStop(0.48, '#66cdfb');
  shellGrad.addColorStop(1, '#0f2f56');
  ctx.fillStyle = shellGrad;
  ctx.beginPath();
  ctx.arc(0, 0, outer, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = selected ? '#d7fbff' : '#90e0ff';
  ctx.lineWidth = selected ? 1.9 : 1.2;
  ctx.stroke();

  const crystalGrad = ctx.createLinearGradient(-inner, -inner, inner, inner);
  crystalGrad.addColorStop(0, '#eefcff');
  crystalGrad.addColorStop(0.55, status === 'failed' ? '#fca5a5' : status === 'succeeded' ? '#86efac' : '#93c5fd');
  crystalGrad.addColorStop(1, '#163f68');
  ctx.fillStyle = crystalGrad;
  ctx.beginPath();
  ctx.moveTo(-inner * 0.3, -inner);
  ctx.lineTo(inner * 0.78, -inner * 0.22);
  ctx.lineTo(inner * 0.38, inner);
  ctx.lineTo(-inner * 0.84, inner * 0.54);
  ctx.lineTo(-inner, -inner * 0.24);
  ctx.closePath();
  ctx.fill();
  ctx.strokeStyle = 'rgba(236,254,255,0.65)';
  ctx.lineWidth = 0.8;
  ctx.stroke();
  ctx.restore();
}

function drawNodeCardLabel(ctx: CanvasRenderingContext2D, data: Record<string, unknown>) {
  const label = typeof data.label === 'string' ? data.label : '';
  if (!label) return;
  const x = Number(data.x ?? 0);
  const y = Number(data.y ?? 0);
  const nodeColor = typeof data.color === 'string' ? data.color : '#64748b';
  const status = typeof data.status === 'string' ? data.status : '';
  const kind = typeof data.kind === 'string' ? data.kind : 'candidate';
  const selected = Boolean(data.selected);
  const lines = label.split('\n');
  const title = lines[0] ?? '';
  const sub = lines[1] ?? '';
  const titleColor = '#e6eefc';
  const subColor = status === 'best'
    ? '#86efac'
    : status === 'failed'
      ? '#fca5a5'
      : status === 'running'
        ? '#93c5fd'
        : '#94a3b8';

  const isExecution = kind === 'execution' || kind === 'iteration';
  const pointRadius = isExecution ? 11.5 : 10.2;
  const runCardHeight = sub ? 28 : 22;

  if (kind === 'execution') {
    drawRunCoreAnchor(ctx, x, y, pointRadius, status, selected);
    return;
  }

  ctx.save();
  ctx.font = '600 11px Space Grotesk, sans-serif';
  const titleWidth = ctx.measureText(title).width;
  ctx.font = '700 10px Space Grotesk, sans-serif';
  const subWidth = sub ? ctx.measureText(sub).width : 0;
  const width = Math.max(titleWidth, subWidth) + 30;
  const left = kind === 'iteration'
      ? x - width - 18
      : x + 12;
  const top = kind === 'iteration'
      ? y - runCardHeight - 24
      : isExecution
      ? y - pointRadius - runCardHeight - 12
      : y - 16;
  const innerFill = selected ? 'rgba(9, 20, 38, 0.97)' : 'rgba(8, 18, 34, 0.93)';
  const stroke = selected ? blendHex(nodeColor, 0.3) : nodeColor;

  if (isExecution) {
    ctx.save();
    ctx.shadowColor = nodeColor;
    ctx.shadowBlur = 34;
    drawRoundedRect(ctx, left - 2.5, top - 2.5, width + 5, runCardHeight + 5, 10);
    ctx.strokeStyle = stroke;
    ctx.lineWidth = 1.5;
    ctx.globalAlpha = 0.85;
    ctx.stroke();
    ctx.restore();

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
    drawRunCoreAnchor(ctx, x, y, pointRadius, status, selected);

    ctx.font = '600 11px Space Grotesk, sans-serif';
    ctx.fillStyle = titleColor;
    ctx.fillText(title, left + 10, top + (sub ? 12 : 14));
    if (sub) {
      ctx.font = '700 10px Space Grotesk, sans-serif';
      ctx.fillStyle = subColor;
      ctx.fillText(sub, left + 10, top + 23);
    }
  } else {
    drawAgentBoxAnchor(ctx, x, y, pointRadius, nodeColor, selected);
    ctx.font = '600 11px Space Grotesk, sans-serif';
    ctx.fillStyle = titleColor;
    const titleTextWidth = ctx.measureText(title).width;
    ctx.font = '700 10px Space Grotesk, sans-serif';
    const subTextWidth = sub ? ctx.measureText(sub).width : 0;
    const textInset = 28;
    const cardWidth = Math.max(titleTextWidth, subTextWidth) + textInset + 10;
    const cardHeight = sub ? 34 : 22;
    const titleLeft = x + textInset;
    const cardTop = y - 22;
    drawRoundedRect(ctx, titleLeft - 8, cardTop, cardWidth, cardHeight, 8);
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
    ctx.font = '600 11px Space Grotesk, sans-serif';
    ctx.fillStyle = titleColor;
    ctx.fillText(title, titleLeft, y - 8);
    ctx.restore();
    if (sub) {
      ctx.font = '700 10px Space Grotesk, sans-serif';
      ctx.fillStyle = subColor;
      ctx.fillText(sub, titleLeft, y + 10);
    }
  }
  ctx.restore();
}

function drawNoopNodeHover() {}

function buildSwarmGraph(
  summary: SwarmExecutionSummary,
  iterations: SwarmIterationSummary[],
  candidates: SwarmCandidateCard[],
  selectedCandidateId: string | null
): Graph {
  const graph = new Graph();
  const executionNodeId = `execution:${summary.executionId}`;
  graph.addNode(executionNodeId, {
    label: `Execution ${summary.executionId}\n${summary.status}`,
    x: -2.6,
    y: 0,
    size: 22,
    color: '#0ea5e9',
    kind: 'execution',
    status: summary.status.toLowerCase(),
    selected: false,
    forceLabel: false
  });

  const sortedIterations = [...iterations].sort((a, b) => a.iterationIndex - b.iterationIndex);
  for (const [iterationOffset, iteration] of sortedIterations.entries()) {
    const iterationNodeId = `iteration:${iteration.iterationIndex}`;
    const iterationX = 2.3 + iterationOffset * 12.4;
    const candidateColumnX = iterationX + 10.9;
    graph.addNode(iterationNodeId, {
      label: `Iteration ${iteration.iterationLabel}\n${iteration.candidateCount} candidates`,
      x: iterationX,
      y: 0,
      size: 15,
      color: '#60a5fa',
      kind: 'iteration',
      status: 'running',
      selected: false,
      forceLabel: true
    });
    graph.addEdge(executionNodeId, iterationNodeId, {
      size: 2.4,
      color: '#7dd3fc',
      type: 'line',
      alpha: 0.92
    });

    const iterationCandidates = candidates.filter((candidate) => candidate.iterationIndex === iteration.iterationIndex);
    iterationCandidates.forEach((candidate, index) => {
      const centeredIndex = index - (iterationCandidates.length - 1) / 2;
      const y = centeredIndex * 3.05;
      const nodeId = `candidate:${candidate.candidateId}`;
      const selected = selectedCandidateId === candidate.candidateId;
      graph.addNode(nodeId, {
        label: `${candidate.candidateId}\n${candidate.state.replace(/_/g, ' ')}`,
        x: candidateColumnX,
        y,
        size: selected ? 16 : 12,
        color: stateColor(candidate.state),
        kind: 'candidate',
        status: candidate.state,
        selected,
        candidateId: candidate.candidateId,
        forceLabel: true
      });
      graph.addEdge(iterationNodeId, nodeId, {
        size: candidate.state === 'best' ? 3 : 1.55,
        color: candidate.state === 'best' ? '#22c55e' : '#64748b',
        type: candidate.state === 'best' ? 'arrow' : 'line',
        alpha: candidate.state === 'best' ? 1 : 0.78
      });
    });
  }
  return graph;
}

export function SwarmGraph({
  summary,
  iterations,
  candidates,
  selectedCandidateId,
  onSelectCandidate
}: SwarmGraphProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const particleCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const rendererRef = useRef<Sigma | null>(null);
  const [zoom, setZoom] = useState(1);
  const [hoverTooltip, setHoverTooltip] = useState<{ text: string; x: number; y: number } | null>(null);

  const graph = useMemo(
    () => buildSwarmGraph(summary, iterations, candidates, selectedCandidateId),
    [summary, iterations, candidates, selectedCandidateId]
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const renderer = new Sigma(graph, container, {
      renderEdgeLabels: false,
      labelRenderedSizeThreshold: 0,
      labelDensity: 2,
      labelGridCellSize: 1,
      defaultNodeColor: '#94a3b8',
      defaultEdgeColor: '#64748b',
      labelColor: { color: '#dbeafe' },
      labelSize: 13,
      defaultDrawNodeLabel: drawNodeCardLabel,
      defaultDrawNodeHover: drawNoopNodeHover
    });
    renderer.getCamera().animatedReset();
    renderer.on('clickNode', ({ node }) => {
      if (node.startsWith('candidate:')) onSelectCandidate(node.slice('candidate:'.length));
    });
    renderer.on('enterNode', ({ node, event }) => {
      const attrs = graph.getNodeAttributes(node) as Record<string, unknown>;
      const label = typeof attrs.label === 'string' ? attrs.label.replace('\n', ' - ') : node;
      setHoverTooltip({ text: label, x: event.x + 10, y: event.y + 12 });
    });
    renderer.on('leaveNode', () => setHoverTooltip(null));
    renderer.getCamera().on('updated', () => {
      setZoom(Number((1 / renderer.getCamera().ratio).toFixed(2)));
    });
    rendererRef.current = renderer;
    return () => {
      renderer.kill();
      rendererRef.current = null;
    };
  }, [graph, onSelectCandidate]);

  useEffect(() => {
    const canvas = particleCanvasRef.current;
    const container = containerRef.current;
    const renderer = rendererRef.current;
    if (!canvas || !container || !renderer || candidates.length === 0) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const graphObj = graph;
    const executionNodeId = `execution:${summary.executionId}`;
    let raf = 0;
    const start = performance.now();

    const draw = (now: number) => {
      const w = container.clientWidth;
      const h = container.clientHeight;
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
      ctx.clearRect(0, 0, w, h);
      const t = (now - start) / 1000;
      ctx.globalCompositeOperation = 'lighter';

      graphObj.forEachEdge((edge, attrs, source, target) => {
        const fromAttrs = graphObj.getNodeAttributes(source);
        const toAttrs = graphObj.getNodeAttributes(target);
        const from = (renderer as unknown as { graphToViewport: (p: { x: number; y: number }) => { x: number; y: number } })
          .graphToViewport({ x: Number(fromAttrs.x), y: Number(fromAttrs.y) });
        const to = (renderer as unknown as { graphToViewport: (p: { x: number; y: number }) => { x: number; y: number } })
          .graphToViewport({ x: Number(toAttrs.x), y: Number(toAttrs.y) });
        const dx = to.x - from.x;
        const dy = to.y - from.y;
        const colorRgb = target.startsWith('candidate:') && String(graphObj.getNodeAttribute(target, 'status')) === 'best'
          ? '34,197,94'
          : '96,165,250';

        ctx.save();
        ctx.strokeStyle = `rgba(${colorRgb},0.28)`;
        ctx.lineWidth = 6;
        ctx.shadowColor = `rgba(${colorRgb},0.45)`;
        ctx.shadowBlur = 18;
        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        ctx.lineTo(to.x, to.y);
        ctx.stroke();
        ctx.restore();

        ctx.save();
        ctx.strokeStyle = `rgba(${colorRgb},0.62)`;
        ctx.lineWidth = Number(attrs.size ?? 2);
        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        ctx.lineTo(to.x, to.y);
        ctx.stroke();
        ctx.restore();

        const phase = (t * 0.55) % 1;
        const x = from.x + dx * phase;
        const y = from.y + dy * phase;
        ctx.fillStyle = `rgba(${colorRgb},0.92)`;
        ctx.shadowColor = `rgba(${colorRgb},0.8)`;
        ctx.shadowBlur = 10;
        ctx.beginPath();
        ctx.arc(x, y, 2.2, 0, Math.PI * 2);
        ctx.fill();
      });

      if (graphObj.hasNode(executionNodeId)) {
        const runAttrs = graphObj.getNodeAttributes(executionNodeId) as Record<string, unknown>;
        const runPoint = (renderer as unknown as { graphToViewport: (p: { x: number; y: number }) => { x: number; y: number } })
          .graphToViewport({ x: Number(runAttrs.x), y: Number(runAttrs.y) });
        const pulse = 0.7 + Math.sin(t * 2.1) * 0.16;
        const ringRadius = 18 + pulse * 4;
        ctx.save();
        ctx.strokeStyle = 'rgba(56,189,248,0.42)';
        ctx.lineWidth = 1.8;
        ctx.shadowColor = 'rgba(56,189,248,0.55)';
        ctx.shadowBlur = 18;
        ctx.beginPath();
        ctx.arc(runPoint.x, runPoint.y, ringRadius, t * 0.5, t * 0.5 + Math.PI * 1.45);
        ctx.stroke();
        ctx.restore();
      }

      ctx.globalCompositeOperation = 'source-over';
      raf = window.requestAnimationFrame(draw);
    };

    raf = window.requestAnimationFrame(draw);
    return () => {
      window.cancelAnimationFrame(raf);
      ctx.clearRect(0, 0, canvas.width, canvas.height);
    };
  }, [candidates, graph, summary.executionId]);

  const zoomIn = () => rendererRef.current?.getCamera().animate({ ratio: rendererRef.current.getCamera().ratio / 1.2 }, { duration: 180 });
  const zoomOut = () => rendererRef.current?.getCamera().animate({ ratio: rendererRef.current.getCamera().ratio * 1.2 }, { duration: 180 });
  const reset = () => rendererRef.current?.getCamera().animatedReset({ duration: 220 });

  const onGraphKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      zoomIn();
    } else if (event.key === '-') {
      event.preventDefault();
      zoomOut();
    } else if (event.key === '0') {
      event.preventDefault();
      reset();
    }
  };

  const healthy = summary.counts.failed === 0;
  return (
    <div className="graph-box sigma-graph-box sigma-graph-surface">
      <div className="panel-title-row">
        <div className="panel-title">Execution Graph</div>
        <div className="graph-kpi-strip">
          <span>Iterations: <strong>{iterations.length}</strong></span>
          <span>Candidates: <strong>{candidates.length}</strong></span>
          <span className={healthy ? 'kpi-succeeded' : 'kpi-failed'}><strong>{healthy ? 'HEALTHY' : 'DEGRADED'}</strong></span>
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
