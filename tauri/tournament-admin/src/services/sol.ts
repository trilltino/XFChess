export const LAMPORTS_PER_SOL = 1_000_000_000;

/** Parses a SOL-denominated input string into whole lamports. NaN/negative → 0. */
export function solInputToLamports(raw: string): number {
  const sol = parseFloat(raw);
  if (!Number.isFinite(sol) || sol < 0) return 0;
  return Math.round(sol * LAMPORTS_PER_SOL);
}

export function lamportsToSolInput(lamports: number): string {
  return String(lamports / LAMPORTS_PER_SOL);
}

/** Best-effort USD equivalent for a lamport amount — null when no rate is loaded yet. */
export function lamportsToUsd(lamports: number, solUsdRate: number | null): number | null {
  if (solUsdRate == null) return null;
  return (lamports / LAMPORTS_PER_SOL) * solUsdRate;
}

/** Parses a USD-denominated input string into whole lamports via the live rate. NaN/negative/no-rate → 0. */
export function usdInputToLamports(raw: string, solUsdRate: number | null): number {
  const usd = parseFloat(raw);
  if (!Number.isFinite(usd) || usd < 0 || !solUsdRate) return 0;
  return Math.round((usd / solUsdRate) * LAMPORTS_PER_SOL);
}

export function lamportsToUsdInput(lamports: number, solUsdRate: number | null): string {
  const usd = lamportsToUsd(lamports, solUsdRate);
  return usd == null ? "" : usd.toFixed(2);
}
