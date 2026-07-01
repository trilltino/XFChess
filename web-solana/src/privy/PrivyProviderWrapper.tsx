import type { ReactNode } from 'react';
import { PrivyProvider } from '@privy-io/react-auth';
import { PRIVY_APP_ID, PRIVY_ENABLED } from './config';

/**
 * Wraps the app in Privy auth when VITE_PRIVY_APP_ID is set. When it isn't,
 * children render unchanged (no provider), so existing wallet-adapter auth keeps
 * working and nothing breaks without a configured Privy app.
 *
 * Privy sits OUTSIDE the wallet-adapter providers (see App.tsx) — it adds
 * email/social + embedded-wallet login alongside the existing external-wallet flow.
 */
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
        loginMethods: ['email', 'google', 'wallet'],
        embeddedWallets: {
          solana: { createOnLogin: 'users-without-wallets' },
        },
      }}
    >
      {children}
    </PrivyProvider>
  );
}

export default PrivyProviderWrapper;
