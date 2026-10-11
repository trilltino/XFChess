// Schedule after completion so slow requests cannot accumulate concurrent polling rounds.
export function startPolling(work: (active: () => boolean) => Promise<void>, interval: number) {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = async () => {
    try { await work(() => !stopped); }
    finally { if (!stopped) timer = setTimeout(() => { void tick().catch(() => {}); }, interval); }
  };
  void tick().catch(() => {});
  return () => { stopped = true; clearTimeout(timer); };
}
