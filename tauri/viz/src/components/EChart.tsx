import { useEffect, useRef } from 'react';
import type { EChartsCoreOption } from 'echarts';
import { init, use as registerCharts } from 'echarts/core';
import { BarChart, GraphChart, LineChart } from 'echarts/charts';
import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';

registerCharts([BarChart, GraphChart, LineChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

/** Thin React wrapper that drives an ECharts instance via a ref. */
export function EChart({
  option,
  height = 320,
}: {
  option: EChartsCoreOption;
  height?: number | string;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const chart = useRef<ReturnType<typeof init> | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    chart.current = init(ref.current, 'dark', { renderer: 'canvas' });
    const onResize = () => chart.current?.resize();
    window.addEventListener('resize', onResize);
    return () => {
      window.removeEventListener('resize', onResize);
      chart.current?.dispose();
      chart.current = null;
    };
  }, []);

  useEffect(() => {
    chart.current?.setOption(option, true);
  }, [option]);

  return (
    <div
      ref={ref}
      style={{
        width: '100%',
        height: typeof height === 'number' ? `${height}px` : height,
        background: 'transparent',
      }}
    />
  );
}
