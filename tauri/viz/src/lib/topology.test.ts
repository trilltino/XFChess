import { describe, it, expect } from 'vitest';
import { CATEGORIES, NODES, LINKS, buildTopologyOption } from './topology';

describe('topology data integrity', () => {
  it('every link references a real node id', () => {
    const ids = new Set(NODES.map((n) => n.id));
    for (const link of LINKS) {
      expect(ids.has(link.source), `dangling link source: ${link.source}`).toBe(true);
      expect(ids.has(link.target), `dangling link target: ${link.target}`).toBe(true);
    }
  });

  it('every node references a real category index', () => {
    for (const node of NODES) {
      expect(node.category).toBeGreaterThanOrEqual(0);
      expect(node.category).toBeLessThan(CATEGORIES.length);
    }
  });
});

describe('buildTopologyOption', () => {
  it('includes every node and link', () => {
    const option = buildTopologyOption();
    const series = option.series as Array<{ data: unknown[]; links: unknown[] }>;
    expect(series[0].data).toHaveLength(NODES.length);
    expect(series[0].links).toHaveLength(LINKS.length);
  });

  it('only the triton node gets the highlighted border style', () => {
    const option = buildTopologyOption();
    const series = option.series as Array<{
      data: Array<{ id: string; itemStyle?: unknown }>;
    }>;
    for (const node of series[0].data) {
      if (node.id === 'triton') {
        expect(node.itemStyle).toBeDefined();
      } else {
        expect(node.itemStyle).toBeUndefined();
      }
    }
  });

  it('symbolSize is larger for the triton node than any other', () => {
    const option = buildTopologyOption();
    const series = option.series as Array<{
      symbolSize: (v: unknown, p: { data?: { id?: string } }) => number;
    }>;
    expect(series[0].symbolSize(null, { data: { id: 'triton' } })).toBe(58);
    expect(series[0].symbolSize(null, { data: { id: 'backend' } })).toBe(40);
  });

  it('the backend->triton link label includes latency when provided, omits it otherwise', () => {
    const withLatency = buildTopologyOption({ tritonLatency: 42.4 });
    const withoutLatency = buildTopologyOption();

    const findTritonLink = (opt: ReturnType<typeof buildTopologyOption>) => {
      const series = opt.series as Array<{
        links: Array<{ source: string; target: string; label: { formatter: string } }>;
      }>;
      return series[0].links.find((l) => l.source === 'backend' && l.target === 'triton')!;
    };

    expect(findTritonLink(withLatency).label.formatter).toBe('settle · tx build\n42ms');
    expect(findTritonLink(withoutLatency).label.formatter).toBe('settle · tx build');
  });

  it('the backend->triton link is drawn wider and in the highlight color than other links', () => {
    const option = buildTopologyOption();
    const series = option.series as Array<{
      links: Array<{
        source: string;
        target: string;
        lineStyle: { width: number; color: string };
      }>;
    }>;
    const triton = series[0].links.find((l) => l.source === 'backend' && l.target === 'triton')!;
    const other = series[0].links.find((l) => !(l.source === 'backend' && l.target === 'triton'))!;

    expect(triton.lineStyle.color).toBe('#f59e0b');
    expect(triton.lineStyle.width).toBeGreaterThan(other.lineStyle.width);
  });
});
