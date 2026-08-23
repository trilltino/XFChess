/**
 * Lazy loader for the Privy SDK, shared by every Privy-aware component.
 *
 * ## Why this exists
 *
 * `@privy-io/react-auth` is ~3.4 MB raw / ~1 MB gzip — on its own, 4.5x the
 * weight of the entire rest of this app (~224 kB gzip). Because the provider has
 * to wrap the app for its hooks to work, a plain static import puts all of that
 * in the critical path of every page load, including the prerendered marketing
 * routes that have nothing to do with wallets.
 *
 * So the SDK is imported dynamically and the provider swaps in once it lands.
 *
 * ## Why the swap is safe
 *
 * Swapping `PrivyProviderWrapper` from passthrough to real provider remounts
 * everything below it — `ConnectionProvider`, `WalletProvider`, `Router`. That
 * would be destructive mid-session, so the load is kicked off immediately on
 * app mount and lands during the first idle moment: before any wallet has
 * connected, before any game is in progress, before the user has interacted.
 * `BrowserRouter` re-reads `window.location` on mount, so route position
 * survives. There is exactly one such swap per page load, at t≈0.
 *
 * `loadPrivy()` is also called on demand (see SocialLoginButtons) so a user who
 * somehow reaches the buttons before the idle load fires still gets a working
 * flow rather than a dead control.
 *
 * ## Store shape
 *
 * A minimal external store rather than context, because the consumers live on
 * opposite sides of the provider boundary: `PrivyProviderWrapper` renders the
 * provider, while `PrivyStandardBridge` and `SocialLoginButtons` render inside
 * it. A context published by the wrapper could not reach the wrapper itself.
 * All consumers re-render from the same `useSyncExternalStore` snapshot, so the
 * provider is guaranteed to be mounted in the same commit that lets the
 * bridge render.
 */
import { useSyncExternalStore } from 'react';
import { PRIVY_ENABLED } from './config';

export type PrivyModule = typeof import('@privy-io/react-auth');
export type PrivySolanaModule = typeof import('@privy-io/react-auth/solana');

export type PrivyRuntime = {
  core: PrivyModule;
  solana: PrivySolanaModule;
};

let runtime: PrivyRuntime | null = null;
let inFlight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function emit() {
  for (const l of listeners) l();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): PrivyRuntime | null {
  return runtime;
}

/**
 * Server snapshot for the prerender pass (`scripts/prerender.mjs` runs this app
 * in Node). Always null: prerendered HTML must not depend on the SDK, and the
 * client hydrates to the same null state before the idle swap.
 */
function getServerSnapshot(): PrivyRuntime | null {
  return null;
}

/** Idempotent. Safe to call from anywhere, any number of times. */
export function loadPrivy(): Promise<void> {
  if (!PRIVY_ENABLED) return Promise.resolve();
  if (runtime) return Promise.resolve();
  if (inFlight) return inFlight;

  inFlight = Promise.all([import('@privy-io/react-auth'), import('@privy-io/react-auth/solana')])
    .then(([core, solana]) => {
      runtime = { core, solana };
      emit();
    })
    .catch((err) => {
      // Leave `runtime` null so the app keeps working with extension wallets
      // only. Reset `inFlight` so a later on-demand call can retry — a chunk
      // fetch can fail on a flaky connection and should not permanently
      // disable social login for the session.
      inFlight = null;
      console.error('[privy] SDK failed to load; social login unavailable', err);
    });

  return inFlight;
}

/** Kicks off the background load once the browser is idle. */
export function schedulePrivyLoad(): void {
  if (!PRIVY_ENABLED || typeof window === 'undefined') return;
  const idle = (
    window as Window & { requestIdleCallback?: (cb: () => void, o?: { timeout: number }) => void }
  ).requestIdleCallback;
  if (idle) idle(() => void loadPrivy(), { timeout: 2000 });
  else window.setTimeout(() => void loadPrivy(), 0);
}

/** `null` until the SDK is loaded. */
export function usePrivyRuntime(): PrivyRuntime | null {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
