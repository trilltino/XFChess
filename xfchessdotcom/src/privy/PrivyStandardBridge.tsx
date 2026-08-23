/**
 * Registers Privy's embedded Solana wallet with the Wallet Standard registry so
 * `@solana/wallet-adapter-react` picks it up and it appears in
 * `useWallet().wallets` next to Phantom.
 *
 * Renders nothing. Must be mounted INSIDE `WalletProvider` (see App.tsx). It
 * only renders its inner component once the lazily-loaded SDK is available, at
 * which point `PrivyProviderWrapper` has mounted the provider in the same
 * commit (both read the same `usePrivyRuntime()` snapshot), so the hooks below
 * always find their context.
 *
 * Two subtleties, both from how wallet-adapter behaves:
 *
 *  1. Registration is idempotent per wallet name, guarded by a module-scoped
 *     Set (see the note on `registeredWallets` — a `useRef` is the obvious
 *     choice and the wrong one). The wallets array is a fresh reference on many
 *     renders, and re-dispatching `register-wallet` for a wallet the adapter
 *     already holds duplicates it.
 *
 *  2. This cannot make the wallet appear on FIRST paint. Privy hydrates its own
 *     state asynchronously, so `ready` is false initially and the array is
 *     empty. `autoConnect` will therefore already have run without seeing Privy.
 *     That is expected — the picker re-renders from `useWallet()` when the
 *     wallet lands. Never gate UI on "is Privy present" at mount time.
 */
import { useEffect } from 'react';
import { registerWallet } from './registerStandardWallet';
import { PRIVY_WALLET_NAME } from './config';
import { usePrivyRuntime, type PrivyRuntime } from './privyRuntime';

/**
 * Module-scoped, NOT a `useRef`.
 *
 * A ref is per-component-instance, and this component is remounted at least
 * twice in a normal page load: once when `PrivyProviderWrapper` swaps from
 * passthrough to the real provider after the lazy SDK load, and again under
 * React StrictMode's double-invoked effects. Each remount handed the effect a
 * fresh empty ref, so it re-dispatched `register-wallet` every time — observed
 * live as three `[privy] registered embedded wallet` log lines and two
 * "Continue with Google" buttons in the DOM for one modal.
 *
 * Registration is a global, idempotent-by-name operation against a `window`
 * event bus, so the bookkeeping has to live at the same scope the effect does.
 */
const registeredWallets = new Set<string>();

function PrivyStandardBridgeInner({ runtime }: { runtime: PrivyRuntime }) {
  const { ready, wallets } = runtime.solana.useStandardWallets();

  useEffect(() => {
    if (!ready) return;
    for (const wallet of wallets) {
      // Only Privy's own embedded wallet. `useStandardWallets` also surfaces
      // Privy's view of external wallets; registering those would duplicate
      // entries wallet-adapter already discovered for itself.
      const isPrivyEmbedded =
        wallet.name === PRIVY_WALLET_NAME && 'privy:' in (wallet.features ?? {});
      if (!isPrivyEmbedded) continue;
      if (registeredWallets.has(wallet.name)) continue;

      registerWallet(wallet as unknown as Parameters<typeof registerWallet>[0]);
      registeredWallets.add(wallet.name);
      console.info('[privy] registered embedded wallet with Wallet Standard');
    }
  }, [ready, wallets]);

  return null;
}

export function PrivyStandardBridge() {
  // Hooks cannot be called conditionally, so the guard lives out here: with no
  // runtime there is no provider above us and the inner hooks would throw.
  const runtime = usePrivyRuntime();
  if (!runtime) return null;
  return <PrivyStandardBridgeInner runtime={runtime} />;
}

export default PrivyStandardBridge;
