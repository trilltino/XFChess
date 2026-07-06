/**
 * XFChess network topology for the visualiser graph.
 *
 * Nodes = real components (Bevy clients, Iroh relay, Axum backend, Triton RPC,
 * MagicBlock ER, Solana L1). The backend↔Triton edge is driven by live latency
 * from the benchmark.
 */

import type { EChartsCoreOption } from 'echarts';

export const CATEGORIES = [
  { name: 'Client (Bevy)' },
  { name: 'Relay' },
  { name: 'Backend' },
  { name: 'RPC' },
  { name: 'Chain' },
];

interface TopoNode {
  id: string;
  name: string;
  category: number;
  value: string;
}
interface TopoLink {
  source: string;
  target: string;
  proto: string;
}

export const NODES: TopoNode[] = [
  { id: 'player-a', name: 'Player A', category: 0, value: 'Bevy game client' },
  { id: 'player-b', name: 'Player B', category: 0, value: 'Bevy game client' },
  { id: 'relay', name: 'Iroh Relay', category: 1, value: 'QUIC P2P relay (braid-iroh)' },
  { id: 'backend', name: 'Backend API', category: 2, value: 'Axum signing + settlement' },
  { id: 'triton', name: 'Triton RPC', category: 3, value: 'Dedicated Solana RPC (base layer)' },
  { id: 'er', name: 'MagicBlock ER', category: 3, value: 'Ephemeral Rollup (devnet-eu)' },
  { id: 'solana', name: 'Solana L1', category: 4, value: 'Base-layer validators' },
];

export const LINKS: TopoLink[] = [
  { source: 'player-a', target: 'player-b', proto: 'Iroh QUIC · moves' },
  { source: 'player-a', target: 'relay', proto: 'Braid HTTP-209' },
  { source: 'player-b', target: 'relay', proto: 'Braid HTTP-209' },
  { source: 'player-a', target: 'backend', proto: 'WebSocket auth' },
  { source: 'relay', target: 'backend', proto: 'relay state' },
  { source: 'backend', target: 'triton', proto: 'settle · tx build' },
  { source: 'player-a', target: 'er', proto: 'record_move' },
  { source: 'backend', target: 'er', proto: 'delegate · finalize' },
  { source: 'backend', target: 'solana', proto: 'finalize · prize' },
  { source: 'triton', target: 'solana', proto: 'RPC → validators' },
  { source: 'er', target: 'solana', proto: 'commit · undelegate' },
];

const PALETTE = ['#7dd3fc', '#a78bfa', '#34d399', '#f59e0b', '#f472b6'];

export function buildTopologyOption(opts?: { tritonLatency?: number; pulse?: number }): EChartsCoreOption {
  const pulse = opts?.pulse ?? 0;

  const links = LINKS.map((l) => {
    const isTriton = l.source === 'backend' && l.target === 'triton';
    const latency = isTriton ? opts?.tritonLatency : undefined;
    const base = isTriton ? 3.5 : 1.6;
    const width = base + Math.sin(pulse + l.source.length) * 0.6 + 0.6;
    return {
      source: l.source,
      target: l.target,
      label: {
        show: true,
        formatter: latency ? `${l.proto}\n${latency.toFixed(0)}ms` : l.proto,
        fontSize: 9,
        color: '#94a3b8',
      },
      lineStyle: {
        width,
        color: isTriton ? '#f59e0b' : '#475569',
        opacity: isTriton ? 0.95 : 0.5,
        curveness: 0.12,
      },
    };
  });

  return {
    backgroundColor: 'transparent',
    tooltip: {
      formatter: (p: { dataType?: string; data?: { value?: string; name?: string } }) =>
        p.dataType === 'node' ? `<b>${p.data?.name}</b><br/>${p.data?.value ?? ''}` : '',
    },
    legend: [{ data: CATEGORIES.map((c) => c.name), textStyle: { color: '#94a3b8' }, top: 8 }],
    series: [
      {
        type: 'graph',
        layout: 'force',
        roam: true,
        draggable: true,
        force: { repulsion: 520, edgeLength: 150, gravity: 0.08, layoutAnimation: true },
        label: { show: true, position: 'right', color: '#e2e8f0', fontSize: 11 },
        edgeSymbol: ['none', 'arrow'],
        edgeSymbolSize: 7,
        categories: CATEGORIES,
        color: PALETTE,
        symbolSize: (_v: unknown, p: { data?: { id?: string } }) => (p.data?.id === 'triton' ? 58 : 40),
        data: NODES.map((n) => ({
          id: n.id,
          name: n.name,
          value: n.value,
          category: n.category,
          itemStyle:
            n.id === 'triton'
              ? { borderColor: '#f59e0b', borderWidth: 3, shadowColor: '#f59e0b', shadowBlur: 18 }
              : undefined,
        })),
        links,
      },
    ],
  };
}
