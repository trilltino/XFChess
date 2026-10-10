/**
 * Mount Privy when VITE_PRIVY_APP_ID is set. Offer Google only; extension
 * wallets use their own provider path to keep one connected-wallet authority.
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
        // Configure the selected Solana chain RPC for Privy signing hooks.
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
