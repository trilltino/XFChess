import { useEffect, useRef } from 'react';
import * as echarts from 'echarts';

/** Thin React wrapper that drives an ECharts instance via a ref. */
export function EChart({
  option,
  height = 320,
}: {
  option: echarts.EChartsCoreOption;
  height?: number | string;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const chart = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    chart.current = echarts.init(ref.current, 'dark', { renderer: 'canvas' });
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
