# web-solana/src/privy

Privy embedded-wallet integration: gives users without a browser extension a
self-custodial wallet created from email/social login, which then works with the same
signing flows as extension wallets.

| File | Contents |
|------|----------|
| [PrivyProviderWrapper.tsx](PrivyProviderWrapper.tsx) | Wraps the app with the Privy provider (configured in [config.ts](config.ts)) |
| [PrivyAuthButton.tsx](PrivyAuthButton.tsx) | Login/logout button using Privy auth state |
| [config.ts](config.ts) | Privy app ID + embedded-wallet settings |

Funding for these wallets is the USDC/Coinbase-Onramp flow (see the fiat on-ramp plan
in the project docs).
