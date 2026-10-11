# Wallet UI

React UI for Phantom, Solflare, and Privy authentication and signing. It runs in
a Chromium app window so installed wallet extensions remain available.

`src/App.tsx` handles onboarding and profile setup. `src/wallet/` contains the
bridge client, authentication, provider adapters, transaction validation,
signing component, popup tracking, and shared UI elements.

Game transactions retain their original message, blockhash, and local
signatures. The browser and game both validate returned transactions before
broadcast. Profile creation simulates before approval and rebuilds unsigned
transactions for expired-blockhash retries. Message signing uses a separate
request type and verifies the connected wallet's signature.

XFChess JWTs stay in memory and are handed to the native bridge before the
wallet is marked connected. Closing a popup cancels pending signing; successful
signing hides the window for reuse. OAuth windows opened by the page close when
their parent unloads. Extension-owned approvals and `noopener` windows remain
under provider control.

Run `npm ci`, `npm test`, and `npm run build`. For development, `npm run dev`
uses port 5174; `VITE_BRIDGE_PORT` selects the owning bridge (default 7454).
