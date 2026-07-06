import { useEffect, useMemo, useRef, useState } from 'react';
import type { EChartsCoreOption } from 'echarts';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { EChart } from './components/EChart';
import { buildTopologyOption } from './lib/topology';

const PUBLIC_DEVNET = 'https://api.devnet.solana.com';
const TRITON_COLOR = '#f59e0b';
const DEVNET_COLOR = '#64748b';

/** Mirrors the Rust LevelResult emitted on the `bench-level` event. */
interface LevelResult {
  endpoint: 'triton' | 'baseline';
  concurrency: number;
  ok: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
  throttled: number;
  errors: number;
  rps: number;
}

function pick(results: LevelResult[], levels: number[], f: (r: LevelResult) => number): (number | null)[] {
  return levels.map((c) => {
    const r = results.find((x) => x.concurrency === c);
    return r ? f(r) : null;
  });
}

function redact(url: string): string {
  const i = url.lastIndexOf('/');
  return i >= 0 && url.length - i - 1 >= 16 ? `${url.slice(0, i + 1)}***` : url;
}

export default function App() {
  const [tritonUrl, setTritonUrl] = useState('');
  const [baselineUrl, setBaselineUrl] = useState(PUBLIC_DEVNET);
  const [requests, setRequests] = useState(120);
  const [levelsStr, setLevelsStr] = useState('1,8,32,64');
  const [running, setRunning] = useState(false);
  const [tritonRes, setTritonRes] = useState<LevelResult[]>([]);
  const [baseRes, setBaseRes] = useState<LevelResult[]>([]);
  const [pulse, setPulse] = useState(0);
  const [status, setStatus] = useState('Idle — paste your Triton URL and run. Requests run natively in Rust (no CORS).');
  const runningRef = useRef(false);

  const levels = useMemo(
    () => levelsStr.split(',').map((s) => parseInt(s.trim(), 10)).filter((n) => n > 0),
    [levelsStr]
  );

  // animate topology
  useEffect(() => {
    const id = setInterval(() => setPulse((p) => p + 0.25), 120);
    return () => clearInterval(id);
  }, []);

  // subscribe to live results from Rust
  useEffect(() => {
    const unlistenLevel = listen<LevelResult>('bench-level', (e) => {
      const r = e.payload;
      if (r.endpoint === 'triton') setTritonRes((p) => [...p, r]);
      else setBaseRes((p) => [...p, r]);
    });
    const unlistenDone = listen('bench-done', () => {
      runningRef.current = false;
      setRunning(false);
      setStatus('Done. Triton 429≈0 while public devnet throttles = the win, visualised.');
    });
    return () => {
      unlistenLevel.then((f) => f());
      unlistenDone.then((f) => f());
    };
  }, []);

  async function run() {
    if (!tritonUrl) {
      setStatus('Enter your Triton RPC URL first.');
      return;
    }
    setRunning(true);
    runningRef.current = true;
    setTritonRes([]);
    setBaseRes([]);
    setStatus('Running natively — Triton vs public devnet…');
    try {
      await invoke('run_read_load', {
        tritonUrl,
        baselineUrl,
        levels,
        requests,
      });
    } catch (e) {
      setStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
      setRunning(false);
      runningRef.current = false;
    }
  }

  const x = levels.map((c) => `c=${c}`);
  const throttlePct = (r: LevelResult) =>
    Math.round((r.throttled / Math.max(r.ok + r.throttled + r.errors, 1)) * 100);

  const throughputOpt: EChartsCoreOption = {
    backgroundColor: 'transparent',
    title: { text: 'Throughput (req/s) — higher is better', textStyle: { color: '#e2e8f0', fontSize: 13 } },
    tooltip: { trigger: 'axis' },
    legend: { textStyle: { color: '#94a3b8' }, top: 26 },
    grid: { left: 50, right: 16, top: 64, bottom: 30 },
    xAxis: { type: 'category', data: x, axisLabel: { color: '#94a3b8' } },
    yAxis: { type: 'value', axisLabel: { color: '#94a3b8' } },
    series: [
      { name: 'Triton', type: 'bar', data: pick(tritonRes, levels, (r) => Math.round(r.rps)), itemStyle: { color: TRITON_COLOR } },
      { name: 'public devnet', type: 'bar', data: pick(baseRes, levels, (r) => Math.round(r.rps)), itemStyle: { color: DEVNET_COLOR } },
    ],
  };

  const throttleOpt: EChartsCoreOption = {
    backgroundColor: 'transparent',
    title: { text: 'Throttled (% HTTP 429) — lower is better', textStyle: { color: '#e2e8f0', fontSize: 13 } },
    tooltip: { trigger: 'axis' },
    legend: { textStyle: { color: '#94a3b8' }, top: 26 },
    grid: { left: 50, right: 16, top: 64, bottom: 30 },
    xAxis: { type: 'category', data: x, axisLabel: { color: '#94a3b8' } },
    yAxis: { type: 'value', max: 100, axisLabel: { color: '#94a3b8', formatter: '{value}%' } },
    series: [
      { name: 'Triton', type: 'bar', data: pick(tritonRes, levels, throttlePct), itemStyle: { color: TRITON_COLOR } },
      { name: 'public devnet', type: 'bar', data: pick(baseRes, levels, throttlePct), itemStyle: { color: DEVNET_COLOR } },
    ],
  };

  const latencyOpt: EChartsCoreOption = {
    backgroundColor: 'transparent',
    title: { text: 'Latency p50 / p95 (ms)', textStyle: { color: '#e2e8f0', fontSize: 13 } },
    tooltip: { trigger: 'axis' },
    legend: { textStyle: { color: '#94a3b8' }, top: 26 },
    grid: { left: 50, right: 16, top: 64, bottom: 30 },
    xAxis: { type: 'category', data: x, axisLabel: { color: '#94a3b8' } },
    yAxis: { type: 'value', axisLabel: { color: '#94a3b8' } },
    series: [
      { name: 'Triton p50', type: 'line', smooth: true, data: pick(tritonRes, levels, (r) => Math.round(r.p50)), itemStyle: { color: TRITON_COLOR } },
      { name: 'Triton p95', type: 'line', smooth: true, lineStyle: { type: 'dashed' }, data: pick(tritonRes, levels, (r) => Math.round(r.p95)), itemStyle: { color: TRITON_COLOR } },
      { name: 'devnet p50', type: 'line', smooth: true, data: pick(baseRes, levels, (r) => Math.round(r.p50)), itemStyle: { color: DEVNET_COLOR } },
      { name: 'devnet p95', type: 'line', smooth: true, lineStyle: { type: 'dashed' }, data: pick(baseRes, levels, (r) => Math.round(r.p95)), itemStyle: { color: DEVNET_COLOR } },
    ],
  };

  const tritonLatency = tritonRes.length ? tritonRes[tritonRes.length - 1].p50 : undefined;
  const topoOpt = useMemo(() => buildTopologyOption({ tritonLatency, pulse }), [tritonLatency, pulse]);

  const card: React.CSSProperties = {
    background: 'rgba(15,23,42,0.55)',
    border: '1px solid rgba(148,163,184,0.18)',
    borderRadius: 12,
    padding: 16,
  };
  const input: React.CSSProperties = {
    background: '#0f172a',
    color: '#e2e8f0',
    border: '1px solid rgba(148,163,184,0.3)',
    borderRadius: 8,
    padding: '9px 11px',
    fontSize: 13,
  };

  return (
    <div style={{ maxWidth: 1240, margin: '0 auto', padding: '28px 20px' }}>
      <h1 style={{ fontSize: 24, fontWeight: 800, margin: 0 }}>
        XFChess Network Visualiser <span style={{ color: TRITON_COLOR }}>·</span> Triton
      </h1>
      <p style={{ color: '#94a3b8', margin: '6px 0 20px' }}>
        Native RPC benchmark (Triton vs public devnet) + live network topology. Token stays in this window — never committed, never in a browser.
      </p>

      <div style={{ ...card, marginBottom: 18, display: 'grid', gap: 12, gridTemplateColumns: '1fr 1fr auto auto auto' }}>
        <input style={input} placeholder="Triton RPC URL (https://…rpcpool.com/<token>)" value={tritonUrl} onChange={(e) => setTritonUrl(e.target.value)} />
        <input style={input} placeholder="Baseline URL" value={baselineUrl} onChange={(e) => setBaselineUrl(e.target.value)} />
        <input style={{ ...input, width: 90 }} type="number" value={requests} onChange={(e) => setRequests(parseInt(e.target.value, 10) || 0)} title="requests / level" />
        <input style={{ ...input, width: 130 }} value={levelsStr} onChange={(e) => setLevelsStr(e.target.value)} title="concurrency levels (comma-separated)" />
        <button onClick={run} disabled={running} style={{ ...input, cursor: running ? 'wait' : 'pointer', background: TRITON_COLOR, color: '#0f172a', fontWeight: 700, border: 'none' }}>
          {running ? 'Running…' : 'Run benchmark'}
        </button>
      </div>

      <p style={{ color: '#94a3b8', fontSize: 13, marginBottom: 16 }}>
        {status}
        {tritonUrl && <span style={{ marginLeft: 8, opacity: 0.7 }}>· {redact(tritonUrl)}</span>}
      </p>

      <div style={{ display: 'grid', gap: 16, gridTemplateColumns: '1fr 1fr', marginBottom: 16 }}>
        <div style={card}><EChart option={throughputOpt} /></div>
        <div style={card}><EChart option={throttleOpt} /></div>
      </div>
      <div style={{ ...card, marginBottom: 16 }}><EChart option={latencyOpt} height={280} /></div>

      <div style={card}>
        <h2 style={{ fontSize: 15, fontWeight: 700, margin: '0 0 4px' }}>Network topology</h2>
        <p style={{ color: '#94a3b8', fontSize: 12, margin: '0 0 8px' }}>
          Drag nodes · the backend↔Triton edge shows live p50 latency from the benchmark.
        </p>
        <EChart option={topoOpt} height={460} />
      </div>
    </div>
  );
}
