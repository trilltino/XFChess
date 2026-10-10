import { useEffect, useState } from "react";
import { apiClient } from "../services/api";

/** Match the backend’s 60-second rate-cache TTL. */
const POLL_INTERVAL_MS = 60_000;
/** Retry while no rate is loaded so startup or cold-cache failures do not leave the UI waiting. */
const RETRY_INTERVAL_MS = 5_000;

/**
 * Display-only SOL/USD conversion; return null until a rate loads and
 * keep SOL input available when the feed fails.
 */
export function useSolUsdRate(): number | null {
  const [rate, setRate] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;

    const poll = async () => {
      try {
        const r = await apiClient.getExchangeRates();
        if (!cancelled && r.ok) {
          const usd = r.data?.rates?.usd;
          if (typeof usd === "number") setRate(usd);
        }
      } catch { /* network error — keep polling */ }
      if (!cancelled) {
        timer = setTimeout(poll, rate == null ? RETRY_INTERVAL_MS : POLL_INTERVAL_MS);
      }
    };
    poll();

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rate == null]);

  return rate;
}
