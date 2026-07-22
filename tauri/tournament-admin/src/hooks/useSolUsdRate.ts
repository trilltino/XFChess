import { useEffect, useState } from "react";
import { apiClient } from "../services/api";

/** How often to re-poll the rate feed once a value has loaded — matches the
 * backend's own RateCache TTL (60s), so we never poll faster than the value
 * can actually change. */
const POLL_INTERVAL_MS = 60_000;
/** Retry cadence while no rate has loaded yet — the backend may still be
 * starting up, or the one-shot CoinGecko/Helius fetch behind it may have
 * raced a cold cache; a single failed attempt shouldn't leave the UI stuck
 * on "rate loading" forever. */
const RETRY_INTERVAL_MS = 5_000;

/**
 * Best-effort SOL→USD rate for display-only USD equivalents next to
 * SOL-denominated inputs. Deliberately not used to gate input — the rate
 * feed (CoinGecko-backed, GET /api/rates/all) is known to go down
 * independently of the admin panel, and SOL amounts must stay enterable
 * even when it does. Polls until a rate has loaded, then keeps refreshing
 * it in the background; returns null until the first successful fetch.
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
