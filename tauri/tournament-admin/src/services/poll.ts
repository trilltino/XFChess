// Schedule after completion so slow requests cannot accumulate concurrent polling rounds.
export function startPolling(work: (active: () => boolean) => Promise<void>, interval: number, immediate = true) {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = async () => {
    try { await work(() => !stopped); }
    finally { if (!stopped) timer = setTimeout(() => { void tick().catch(() => {}); }, interval); }
  };
  if (immediate) void tick().catch(() => {});
  else timer = setTimeout(() => { void tick().catch(() => {}); }, interval);
  return () => { stopped = true; clearTimeout(timer); };
}
