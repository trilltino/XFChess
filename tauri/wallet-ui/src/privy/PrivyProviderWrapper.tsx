/**
 * Mounts Privy around the popup when `VITE_PRIVY_APP_ID` is set.
 *
 * Unlike the website (`xfchessdotcom/src/privy/`), this imports the SDK
 * statically. The website defers it because a megabyte of JavaScript in the
 * critical path of a public marketing page is a real cost; here the bundle is
 * served from `http://localhost` by the Tauri bridge on the user's own machine,
 * so the download is essentially free and the lazy-load machinery would be
 * complexity with no benefit.
 *
 * Google only. Privy's `'email'` method is email-OTP that also mints an embedded
 * wallet — not the backend's argon2 email/password login — but it is
 * deliberately not offered: the social path exists to give a non-crypto user a
 * wallet in one click.
 *
 * Extension wallets stay on their own path (`wallet/extension.ts`) — letting
 * Privy also broker Phantom would give the popup two competing notions of "the
 * connected wallet", and this window's whole job is to report exactly one
 * pubkey to the bridge.
 */
import type { ReactNode } from 'react';
import { PrivyProvider } from '@privy-io/react-auth';
import { createSolanaRpc, createSolanaRpcSubscriptions } from '@solana/kit';
import {
  PRIVY_APP_ID,
  PRIVY_ENABLED,
  SOLANA_CHAIN,
  SOLANA_RPC_URL,
  SOLANA_RPC_WS_URL,
} from './config';

export function PrivyProviderWrapper({ children }: { children: ReactNode }) {
  if (!PRIVY_ENABLED) return <>{children}</>;

  return (
    <PrivyProvider
      appId={PRIVY_APP_ID}
      config={{
        appearance: {
          theme: 'dark',
          accentColor: '#14f195',
          walletChainType: 'solana-only',
        },
        loginMethods: ['google'],
        embeddedWallets: {
          solana: { createOnLogin: 'users-without-wallets' },
        },
        // Privy's standard-wallet signing hooks resolve an RPC for the chain
        // being signed on, and throw outright if there is no entry for it.
        // Without this the very first embedded-wallet transaction died with
        // "No RPC configuration found for chain solana:mainnet" — mainnet
        // because `chain` defaults there, hence SOLANA_CHAIN at every call site.
        solana: {
          rpcs: {
            [SOLANA_CHAIN]: {
              rpc: createSolanaRpc(SOLANA_RPC_URL),
              rpcSubscriptions: createSolanaRpcSubscriptions(SOLANA_RPC_WS_URL),
            },
          },
        },
      }}
    >
      {children}
    </PrivyProvider>
  );
}

export default PrivyProviderWrapper;
