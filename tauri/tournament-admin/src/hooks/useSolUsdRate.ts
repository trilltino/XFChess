import { useEffect, useState } from "react";
import { apiClient } from "../services/api";

/**
 * Best-effort SOL→USD rate for display-only USD equivalents next to
 * SOL-denominated inputs. Deliberately not used to gate input — the rate
 * feed (CoinGecko-backed, GET /api/rates/all) is known to go down
 * independently of the admin panel, and SOL amounts must stay enterable
 * even when it does. Returns null until a rate has loaded successfully.
 */
export function useSolUsdRate(): number | null {
  const [rate, setRate] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    apiClient
      .getExchangeRates()
      .then(r => {
        if (cancelled || !r.ok) return;
        const usd = r.data?.rates?.usd;
        if (typeof usd === "number") setRate(usd);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  return rate;
}
