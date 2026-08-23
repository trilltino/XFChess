/**
 * Mounts Privy when `VITE_PRIVY_APP_ID` is set AND the SDK chunk has loaded;
 * renders children untouched otherwise.
 *
 * Placement matters: this sits OUTSIDE the wallet-adapter providers (see
 * App.tsx). Privy is an auth layer that *produces* an embedded wallet;
 * wallet-adapter is the layer that *consumes* it. The handoff between them is
 * the Wallet Standard registration in PrivyStandardBridge, which must therefore
 * mount inside WalletProvider.
 *
 * The SDK is loaded lazily — see privyRuntime.ts for why, and for why the
 * resulting one-time remount is safe.
 *
 * Login methods are deliberately `['google', 'email']` and NOT `'wallet'`:
 * external wallets (Phantom, Solflare, Mobile Wallet Adapter) stay entirely on
 * wallet-adapter. Letting Privy also broker external wallets would give the app
 * two independent, competing notions of "the connected wallet" — exactly the
 * split-brain that AppContent's stale-JWT guard already exists to clean up after.
 */
import { useEffect, useMemo, type ReactNode } from 'react';
import { createSolanaRpc, createSolanaRpcSubscriptions } from '@solana/kit';
import { PRIVY_APP_ID, PRIVY_ENABLED } from './config';
import { schedulePrivyLoad, usePrivyRuntime, type PrivyRuntime } from './privyRuntime';

/**
 * Privy's `solana.rpcs` wants live `@solana/kit` clients, not URLs, and requires
 * a subscriptions client alongside the request client. Derive the websocket
 * endpoint from the HTTP one so both halves point at the same node — important
 * with Helius, where falling back to the public URL would silently give Privy a
 * different (shared, rate-limited, load-balanced) view of the cluster than the
 * one ConnectionProvider reads from.
 */
function toWebsocketUrl(httpUrl: string): string {
  return httpUrl.replace(/^https:/, 'wss:').replace(/^http:/, 'ws:');
}

function LoadedProvider({
  runtime,
  rpcUrl,
  children,
}: {
  runtime: PrivyRuntime;
  rpcUrl: string;
  children: ReactNode;
}) {
  const { PrivyProvider } = runtime.core;

  const solanaRpcs = useMemo(
    () => ({
      'solana:devnet': {
        rpc: createSolanaRpc(rpcUrl),
        rpcSubscriptions: createSolanaRpcSubscriptions(toWebsocketUrl(rpcUrl)),
      },
    }),
    [rpcUrl]
  );

  return (
    <PrivyProvider
      appId={PRIVY_APP_ID}
      config={{
        appearance: {
          theme: 'dark',
          accentColor: '#14f195',
          walletChainType: 'solana-only',
        },
        // Google only. Privy's `'email'` method is email-OTP that also mints an
        // embedded wallet — not the backend's argon2 email/password login — but
        // it is deliberately not offered: the social path exists to give a
        // non-crypto user a wallet in one click, and a second option earns
        // nothing here. External wallets stay entirely on wallet-adapter.
        loginMethods: ['google'],
        embeddedWallets: {
          solana: { createOnLogin: 'users-without-wallets' },
        },
        solana: { rpcs: solanaRpcs },
      }}
    >
      {children}
    </PrivyProvider>
  );
}

export function PrivyProviderWrapper({
  children,
  rpcUrl,
}: {
  children: ReactNode;
  /** Same endpoint ConnectionProvider uses, so both agree on the cluster. */
  rpcUrl: string;
}) {
  const runtime = usePrivyRuntime();

  useEffect(() => {
    schedulePrivyLoad();
  }, []);

  if (!PRIVY_ENABLED || !runtime) return <>{children}</>;

  return (
    <LoadedProvider runtime={runtime} rpcUrl={rpcUrl}>
      {children}
    </LoadedProvider>
  );
}

export default PrivyProviderWrapper;
