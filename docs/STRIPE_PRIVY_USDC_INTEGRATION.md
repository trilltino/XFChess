# Stripe + Privy + USDC Integration

> Deep integration guide for adding **Privy embedded wallets**, **Stripe fiat on-ramp**,
> and a **USDC-only** money model on top of the existing Phantom/Solflare wallet-adapter
> setup in `web-solana/`.
>
> Goal: a non-crypto person signs in with email, funds with a card, and plays — the crypto
> is invisible — **without breaking the non-custodial architecture** the jurisdictional
> analysis depends on.

---

## 0. TL;DR — what this changes

| Layer | Today | After |
|---|---|---|
| Wallet | `@solana/wallet-adapter-react` (Phantom, Solflare, WalletConnect) | **Privy** unified provider: email/Google → embedded wallet, *plus* Phantom/Solflare as external connectors |
| Funding | User must already hold SOL/USDC | **Stripe on-ramp** (card → USDC) via Privy `useFundWallet` |
| Money unit | PvP in **SOL**, tournaments in USDC | **USDC only**, everywhere |
| Cash-out | Manual / none | **Bridge** off-ramp (USDC → bank) or stablecoin card |
| Network | **Devnet** | **Mainnet** (real money requires it — see §1) |
| Move signing | Session keypair (raw) | **Unchanged** — stays a raw keypair, never Privy (see §7) |

**Three workstreams, in order:** (1) Privy provider + `useWallet()` shim, (2) USDC-only program + frontend, (3) Stripe funding + Bridge off-ramp.

---

## 1. Hard prerequisites — read before writing any code

These are non-negotiable facts that constrain everything:

1. **Real money ⇒ mainnet.** Stripe's on-ramp delivers **mainnet** USDC. Bridge off-ramps **mainnet** USDC. The USDC mint already in [`countryStablecoins.ts`](../web-solana/src/lib/countryStablecoins.ts) (`EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`) is **mainnet** USDC. Your app is currently `WalletAdapterNetwork.Devnet` ([`App.tsx:56`](../web-solana/src/App.tsx#L56)). **You cannot test the real money path on devnet.** Plan a mainnet config + a devnet config gated by env var, and accept that on-ramp/off-ramp can only be exercised on mainnet (use tiny amounts).

2. **USDC-only is a *program* change, not just frontend.** PvP `create_game` / `join_game` / `finalize` currently move **SOL** (lamports via `system_program::transfer`, `WAGER_ESCROW_SEED`). Only the **tournament** path has a USDC escrow (`TOURNAMENT_USDC_PRIZE_SEED`, `claim_prize.rs`). Making PvP USDC means refactoring the SOL escrow into an **SPL-token escrow** — use the tournament USDC code as the reference pattern. This is the largest single piece of work. See §5.

3. **Non-custodial invariant must survive.** XFChess never holds keys to user funds. Privy embedded wallets must be set to the **non-custodial configuration** (user-controlled). *Confirmed* by Privy's published architecture ([how-privy-embedded-wallets-work](https://privy.io/blog/how-privy-embedded-wallets-work)): *"Neither Privy nor your application ever sees the user's keys"* — keys are split via **Shamir Secret Sharing**, stored in isolated hardware, reconstructed only inside a **TEE** at signing under the user's control; *"Users have full custody of their wallets."* **Caveat:** Privy wallets *can* be configured custodial — you MUST use the non-custodial config and document this architecture in your compliance memo (the jurisdictional FACT 1 depends on it).

4. **Stripe acceptable-use — TWO products, TWO rulebooks (verified).**
   - ❌ **Stripe payment processing is PROHIBITED for you.** The [Restricted Businesses list](https://stripe.com/legal/restricted-businesses) explicitly bans *"Games of skill including ... tournaments or competitions ... with a monetary or material prize"* and *"Payments of an entry or player fee that promise the entrant or player will win a prize of value."* **Never collect entry fees/stakes via Stripe Checkout/Payments.**
   - ⚠️ **Stripe Crypto On-ramp — terms are silent, but DON'T rely on it.** The [On-ramp Terms](https://stripe.com/legal/crypto-onramp) themselves contain no gambling clause and no self-custody restriction, and there's a real theory that the on-ramp merely *sells USDC to the player* (consumer, personal use) rather than processing a gambling payment. **BUT** the Restricted Businesses list is incorporated by reference into Stripe's **master Services Agreement**, which bars using *any* Stripe service *"in connection with"* a restricted business. The list explicitly names *"board games with a monetary or material prize"* and *"entry or player fee that promise ... a prize of value"* — i.e., XFChess. **Expect Stripe's underwriting to decline at integration review.** Treat Stripe as *unlikely*, not as the plan.
   - **Personal-use clause** (respect it regardless of provider): *"The Crypto Assets you are purchasing are for your own personal use. You may not use the Onramp Services to buy cryptocurrencies ... on behalf of any other person or entity."* → the **player** buys their own USDC; XFChess never buys on their behalf.
   - **Design provider-agnostic.** Route funding through **Privy `useFundWallet`** so the on-ramp provider is swappable. NOT a category-wide ban — Stripe is the strict outlier. Provider comparison for a *lawful skill* chess app (verified June 2026):

     | Provider | Gambling wording | Skill carve-out | Verdict |
     |---|---|---|---|
     | Stripe | bans "games of skill ... with a monetary prize" outright | none | ❌ No |
     | Transak | bans "gambling" outright (no "unlawful" qualifier) | none | ❌ Likely no |
     | MoonPay | bans ***unlawful*** gambling | implicit | ⚠️ Maybe — confirm directly |
     | **Coinbase Onramp** | bans ***unlawful*** gambling | ✅ **"Games of Skill" (entry fee + prize) = conditional/approvable** | ✅ **Best path** |

   - **Primary path = Coinbase Onramp.** It explicitly defines "Games of Skill" (*"not defined as gambling ... but which require an entry fee and award a prize"*) as **conditional, not prohibited**. Apply for conditional approval; lead with the jurisdictional memo (chess = lawful skill, not gambling). Eligibility hinges on "lawful," which is per-jurisdiction. (Coinbase's raw policy page blocked automated fetch — verify the clause verbatim before relying on it.)
   - **The legal memo does double duty:** the same "chess is a lawful skill contest, not gambling" analysis that is your jurisdictional shield is *also* the evidence package Coinbase's conditional approval requires.
   - **Unblockable fallback = self-funding.** If every on-ramp approval fails, users buy USDC on any exchange and withdraw to their Privy wallet ("deposit USDC to play"). XFChess has zero involvement in the fiat purchase → unambiguously the user's own personal crypto, outside your app. Clunkier (loses the pure-normie "feels like PayPal" magic) but cannot be banned.
   - **Note:** Privy (wallet) and Bridge (off-ramp) are unaffected by on-ramp choice — only the card→USDC provider swaps.

---

## 2. Target architecture

```
┌─────────────────────────── web-solana (React) ───────────────────────────┐
│                                                                            │
│  PrivyProvider                                                             │
│   ├─ email / Google login ──► embedded Solana wallet (self-custodial)      │
│   ├─ external connectors  ──► Phantom / Solflare (unchanged power users)   │
│   └─ useFundWallet()      ──► Stripe on-ramp (card → USDC, mainnet)        │
│                                                                            │
│  useWallet() COMPAT SHIM  ──► maps Privy wallet → wallet-adapter shape     │
│   (so the ~20 pages calling useWallet() keep working)                      │
│                                                                            │
│  AnchorProvider(connection, shimWallet)  ──► builds USDC txns              │
│  Session keypair (raw, in IndexedDB)      ──► signs every move (NOT Privy) │
└────────────────────────────────────────────────────────────────────────┘
        │ USDC (mainnet)                              │ raw keypair
        ▼                                             ▼
   xfchess-game program  ◄── SPL-token escrow ──  record_move (ER)
        │
        ▼ winnings (USDC)
   Bridge off-ramp ──► user's bank   /   stablecoin card (spend directly)
```

Two signers, two roles (this is the cost-critical split, see §7):
- **Privy embedded wallet** — signs only the *rare* things: `authorize_global_session`, funding approvals, withdrawals.
- **Session keypair** — signs *every* move and create/join. A plain Ed25519 keypair, **never** a Privy wallet.

---

## 3. Dependencies

```bash
cd web-solana
npm i @privy-io/react-auth
# Privy ships Solana support + external-wallet connectors + funding in the same package.
# Verify exact submodule paths against current Privy docs — they version the Solana API.
```

Keep `@solana/web3.js`, `@coral-xyz/anchor`, `@solana/spl-token` (add if missing — needed for USDC ATAs), and the MagicBlock ER SDK. You will **remove** the `@solana/wallet-adapter-*` provider wiring once the shim is proven (§4), but you can keep the packages installed during migration.

```bash
npm i @solana/spl-token   # ATA + token transfer instructions for USDC
```

---

## 4. Workstream 1 — Privy provider + `useWallet()` compatibility shim

### 4.1 Why a shim

~20 files call `useWallet()` from `@solana/wallet-adapter-react` (Players, Play, Tournaments, anchor_client, magicblock, …). Rewriting all of them at once is risky. Instead, replace the **provider** and expose a hook with the **same shape** (`publicKey`, `connected`, `signTransaction`, `signAllTransactions`, `sendTransaction`, `disconnect`). Pages don't change; only the import source does (or re-export under the same name).

### 4.2 Replace the provider in `App.tsx`

Replace the `ConnectionProvider` + `WalletProvider` block ([`App.tsx:96-104`](../web-solana/src/App.tsx#L96)) with `PrivyProvider`:

```tsx
// App.tsx (provider section)
import { PrivyProvider } from '@privy-io/react-auth';
import { toSolanaWalletConnectors } from '@privy-io/react-auth/solana'; // verify path

const NETWORK = import.meta.env.VITE_SOLANA_NETWORK ?? 'mainnet-beta';
const RPC = import.meta.env.VITE_SOLANA_RPC!; // Helius mainnet RPC

export default function App() {
  return (
    <PrivyProvider
      appId={import.meta.env.VITE_PRIVY_APP_ID!}
      config={{
        // Login options shown in the Privy modal:
        loginMethods: ['email', 'google', 'wallet'], // 'wallet' = Phantom/Solflare
        appearance: { walletChainType: 'solana-only' },
        externalWallets: {
          solana: { connectors: toSolanaWalletConnectors() }, // Phantom, Solflare
        },
        embeddedWallets: {
          solana: { createOnLogin: 'users-without-wallets' }, // email users get one silently
          // showWalletUIs: true  → confirmation modal on embedded-wallet signs (keep ON; see §6)
        },
        solana: {
          rpcs: { 'mainnet-beta': { rpc: RPC } },
        },
      }}
    >
      <Router>
        <AppContent />
      </Router>
    </PrivyProvider>
  );
}
```

> Connection: keep a single `Connection` from `@solana/web3.js` in a small context/provider (Privy doesn't supply one). `useConnection()` callers can be pointed at it via the shim or a thin replacement context.

### 4.3 The shim hook

Create `web-solana/src/lib/useWallet.ts` exporting a wallet-adapter-shaped object backed by Privy:

```ts
// useWallet.ts — drop-in replacement for @solana/wallet-adapter-react's useWallet
import { usePrivy, useSolanaWallets } from '@privy-io/react-auth'; // verify exports
import { PublicKey, Transaction, VersionedTransaction } from '@solana/web3.js';

export function useWallet() {
  const { ready, authenticated, login, logout } = usePrivy();
  const { wallets } = useSolanaWallets();       // embedded + external, unified
  const active = wallets[0];                    // or your selection logic

  const publicKey = active ? new PublicKey(active.address) : null;

  return {
    publicKey,
    connected: ready && authenticated && !!active,
    connecting: !ready,
    connect: login,
    disconnect: logout,
    // Privy exposes signing per wallet; adapt to wallet-adapter signatures:
    signTransaction: <T extends Transaction | VersionedTransaction>(tx: T) =>
      active!.signTransaction(tx) as Promise<T>,
    signAllTransactions: <T extends Transaction | VersionedTransaction>(txs: T[]) =>
      active!.signAllTransactions(txs) as Promise<T[]>,
    sendTransaction: (tx: Transaction | VersionedTransaction) =>
      active!.sendTransaction(tx),            // confirm Privy's send API/signature
  };
}
```

Then either (a) change imports in the ~20 files from `@solana/wallet-adapter-react` → `../lib/useWallet`, or (b) keep a barrel that re-exports. The `AnchorProvider(connection, wallet as any, …)` pattern in [`anchor_client.ts:8`](../web-solana/src/lib/anchor_client.ts#L8) and [`magicblock.ts`](../web-solana/src/lib/magicblock.ts) already takes a `wallet`-shaped object, so it keeps working as long as the shim exposes `publicKey` + `signTransaction` + `signAllTransactions`.

### 4.4 Replace the connect UI

`WalletSelectionModal` ([`WalletSelectionModal.tsx`](../web-solana/src/components/WalletSelectionModal.tsx)) and the "Connect Wallet" button ([`App.tsx:338`](../web-solana/src/App.tsx#L338)) get replaced by a single **`login()`** call — Privy renders its own modal (email + Google + Phantom/Solflare in one). You can delete the custom modal, or keep a thin "Sign in / Connect" button that calls `login()`.

### 4.5 Tauri note

The existing Tauri branch disables extension wallets and pushes WalletConnect ([`App.tsx:53`](../web-solana/src/App.tsx#L53), `WalletSelectionModal.tsx`). With Privy, **email/Google login works in Tauri without an extension** — a real upgrade. Verify Privy's OAuth redirect works inside the Tauri webview; you may need a custom redirect/deep-link handler.

---

## 5. Workstream 2 — USDC-only (program + frontend)

This is the biggest change. Today PvP money is SOL; the program must move to an SPL-token escrow.

### 5.1 Program changes (Rust / Anchor)

Reference the **existing tournament USDC path** — it already does SPL escrow with a PDA authority:
[`claim_prize.rs`](../programs/xfchess-game/src/tournament_ix/prizes/claim_prize.rs) (`TOURNAMENT_USDC_PRIZE_SEED`, `token::transfer` with `CpiContext::new_with_signer`).

For PvP, mirror that pattern:

1. **Escrow becomes a token account.** Replace the system-owned `WAGER_ESCROW_SEED` lamport PDA with a **USDC ATA owned by a PDA** (escrow authority = `[ESCROW_SEED, game_id]`).
2. **`create_game` / `join_game`:** instead of `system_program::transfer` of lamports, do `token::transfer` of `stake_amount` (USDC, 6 decimals) from the player's USDC ATA → escrow token account. Add `token_program`, `usdc_mint`, player ATA, escrow ATA to the accounts struct.
3. **`finalize` ([`finalize.rs`](../programs/xfchess-game/src/game_ix/finalize.rs)):** replace the SOL payout (`finalize.rs:118-185`) and the `country_fee`/treasury sweep (`finalize.rs:205-224`) with `token::transfer` from escrow ATA → winner ATA (pot) and → treasury ATA (the 10p platform fee, expressed in USDC). Draw/refund path → each player's ATA.
4. **Fee denomination.** "10p" is GBP; on-chain you store a USDC amount. Keep the existing pattern (`constants.rs:116` — *"backend calculates live lamport amounts from the SOL/GBP rate"*) but compute **USDC** units from GBP/USD instead. The platform fee stays a flat, disclosed amount (not a % of pot) — preserves FACT 2 of the jurisdictional analysis.
5. **Rent for ATAs.** Each escrow ATA needs rent (~0.002 SOL). Decide who advances it (relayer/`fee_payer`, already a concept in `finalize.rs:30`) and reclaim it on close.
6. **Session-key caps.** `GlobalSessionDelegation` caps are in **lamports/SOL** ([`global_session.rs:57-60`](../programs/xfchess-game/src/state/global_session.rs#L57) — `spending_limit` 5 SOL, `max_wager` 1 SOL). Re-denominate these to **USDC** or they'll gate the wrong asset.

> Keep `record_move` ([`record.rs`](../programs/xfchess-game/src/moves_ix/record.rs)) **unchanged** — it moves no funds, only validates/records. USDC has zero impact on the move path.

### 5.2 Frontend changes

1. **ATA management:** before a USDC stake, ensure the player has a USDC ATA (`getAssociatedTokenAddress` + `createAssociatedTokenAccountInstruction` from `@solana/spl-token`). Privy embedded wallets won't have one until first funded.
2. **Amounts:** display GBP, transact USDC (6 decimals). Add a GBP↔USD rate source (you already fetch prices via Helius in [`useWalletUsdBalance.ts:42`](../web-solana/src/hooks/useWalletUsdBalance.ts#L42)).
3. **Balance UI:** [`useWalletUsdBalance.ts`](../web-solana/src/hooks/useWalletUsdBalance.ts) already enumerates SPL tokens — filter to the USDC mint and show that as "your balance" (stable £/$), instead of SOL value which wobbles.
4. **IDL:** regenerate the Anchor IDL after the program changes and copy `xfchess_game.json` into `web-solana/src/lib/` (per `web-solana/CLAUDE.md`).

---

## 6. Workstream 3 — Stripe funding + Bridge off-ramp

### 6.1 Funding (card → USDC) via Privy

Privy wraps on-ramp providers (Stripe being native post-acquisition) behind `useFundWallet`:

```ts
import { useFundWallet } from '@privy-io/react-auth/solana'; // verify path

const { fundWallet } = useFundWallet();

await fundWallet(walletAddress, {
  cluster: { name: 'mainnet-beta' },
  amount: '20',          // USD; encourage LARGER top-ups (see fee note)
  asset: 'USDC',
  // provider/onramp config: select Stripe per Privy's funding config
});
```

This opens the **Stripe on-ramp in a modal**. User pays by card; mainnet USDC lands in their Privy wallet in ~seconds.

**Fee design (critical):** Stripe on-ramp ≈ **1.5% + $0.30 fixed**, borne by the user. The $0.30 fixed fee makes per-game ramping absurd (£2 top-up ≈ 18% fee). So **the UX must be balance-first**: user tops up £20 once, then your 10p/50p fees draw down the existing USDC balance with no further ramp. Never trigger `fundWallet` per game. Surface "Add funds" only when balance is low.

### 6.2 Off-ramp (USDC → bank) via Bridge

- **Withdraw flow:** a "Withdraw" button → Bridge off-ramp (link bank once, KYC once) → USDC converted to fiat → bank.
- **Stablecoin card:** alternatively, a Bridge/Stripe Issuing card lets the user *spend* USDC directly — no explicit withdrawal.
- **You never touch the money** — Bridge is the regulated party. Non-custodial invariant intact.
- **KYC coordination:** the first withdrawal triggers KYC at the Bridge layer. You already plan Didit KYC — make these one step, not two, so users aren't double-verified. Gate KYC by a cumulative-stake threshold (e.g. £100) as the jurisdictional doc suggests.

---

## 7. How Privy popups work (and why moves have none)

This is the question that decides both UX and cost. Privy has **four** distinct popup/modal moments:

| Moment | Popup? | Frequency | Notes |
|---|---|---|---|
| **Login** | Yes — Privy modal | Once per session | Email OTP / Google / "connect Phantom". For external wallets, Privy hands off to the wallet's own popup. |
| **Embedded-wallet signing** | Configurable | Only on funds-moving txns *you route through Privy* | Controlled by `embeddedWallets.showWalletUIs`. Default ON shows a confirmation modal; can be turned OFF for silent signing. |
| **External wallet (Phantom/Solflare) signing** | Yes — the wallet's native popup | Per tx routed to it | Same as today; Privy doesn't change extension behaviour. |
| **Funding** | Yes — Stripe modal | Per top-up (rare) | `useFundWallet` opens the on-ramp. |

**The key point for XFChess:** moves do **not** go through Privy at all. `record_move` is signed by the **session keypair** ([`record.rs:17`](../programs/xfchess-game/src/moves_ix/record.rs#L17) — `player: Signer`, constrained to `session_delegation.session_key`), which is a **raw Ed25519 keypair** held client-side ([`global_session.rs:32`](../programs/xfchess-game/src/state/global_session.rs#L32) — *"Hot key held by the VPS / client"*). So:

- **Per-move signing = session keypair = zero Privy popups, zero Privy signature billing.**
- **Privy signs only the rare events:** `authorize_global_session` (once per 30 days / 200 games — [`global_session.rs:53-56`](../programs/xfchess-game/src/state/global_session.rs#L53)), funding, and withdrawals.

For those rare events, **keep `showWalletUIs` ON** — you *want* a confirmation when real money is authorized or moved. The "no popup per move" smoothness comes from your **session-key + ER design**, not from disabling Privy's UI.

> ⚠️ **Hard rule for whoever implements this:** the session key MUST stay a raw keypair (generate with `Keypair.generate()`, persist in IndexedDB/secure storage). **Do NOT implement the session key as a second Privy embedded wallet** — that would route all 80 signatures/game through Privy, blow the 50k/month free signature tier (~625 games), and add a popup per move. Privy = main wallet only.

---

## 8. Costs (recap)

| Item | Who pays | Amount |
|---|---|---|
| Stripe on-ramp | End user | ~1.5% + $0.30 per top-up (so: big, infrequent top-ups) |
| Bridge off-ramp | End user | conversion fee on withdrawal |
| Privy | **You** | Free <500 MAU; ~$299/mo to 2,500 MAU; usage-based beyond. Trivial vs revenue at that scale **iff** §7 boundary is respected |
| Solana per move | Relayer/escrow | ~$0.0001 (ER moves cheaper still) |
| Escrow ATA rent | Relayer (reclaim on close) | ~0.002 SOL per game, recoverable |

---

## 9. Risks & gotchas

1. **Mainnet cutover** — on-ramp/off-ramp/USDC only work on mainnet. Maintain env-gated config; test money paths with tiny real amounts.
2. **Devnet USDC ≠ mainnet USDC** — different mints. Don't hardcode one mint for both networks.
3. **Stripe gambling policy (verified, §1.4)** — payment processing is *prohibited* for skill-tournaments-with-prizes; the **crypto on-ramp is the only viable Stripe path** (user buys own USDC, personal use). Residual risk is Stripe's underwriting discretion → get written sign-off before depending on it.
4. **Privy self-custody (verified, §1.3)** — non-custodial via TEE + Shamir sharding, but only if you select the **non-custodial config**. Document it for the jurisdictional FACT 1.
5. **Privy API drift** — hook/config names (`useSolanaWallets`, `useFundWallet`, connector paths) change between Privy versions. Treat the snippets here as shape, not gospel; verify against current Privy docs.
6. **Double KYC** — unify Didit + Bridge KYC.
7. **Session-key boundary** — see §7 warning. This is the single most important implementation rule.
8. **Two-context bleak** — during migration, don't leave both `WalletProvider` and `PrivyProvider` mounted; pick one source of truth for `useWallet()`.

---

## 10. Ordered checklist

**Phase 0 — decisions**
- [ ] Confirm Stripe acceptable-use for skill-chess
- [ ] Confirm Privy self-custody model in writing
- [ ] Decide mainnet RPC (Helius) + env-gated network config

**Phase 1 — Privy provider + shim** (no money yet, devnet OK)
- [ ] Add `@privy-io/react-auth`, set `VITE_PRIVY_APP_ID`
- [ ] Replace provider in `App.tsx` (§4.2) + Connection context
- [ ] Write `lib/useWallet.ts` shim (§4.3); repoint the ~20 imports
- [ ] Replace `WalletSelectionModal` / Connect button with `login()` (§4.4)
- [ ] Verify Anchor `AnchorProvider` + ER still sign via shim
- [ ] Verify Tauri email login (§4.5)

**Phase 2 — USDC-only**
- [ ] Program: SPL-token escrow for PvP `create`/`join`/`finalize` (§5.1)
- [ ] Program: re-denominate session caps + platform fee to USDC
- [ ] Regenerate + copy IDL
- [ ] Frontend: ATA creation, GBP↔USDC display, USDC balance UI (§5.2)

**Phase 3 — fiat rails**
- [ ] Funding via `useFundWallet` → Stripe, balance-first UX (§6.1)
- [ ] Withdraw via Bridge off-ramp + KYC unify (§6.2)
- [ ] Optional: stablecoin card

**Phase 4 — mainnet**
- [ ] Flip env to mainnet, smoke-test money paths with tiny amounts
- [ ] Verify non-custodial invariant end-to-end

---

## 11. Env vars (add to `web-solana/.env`)

```
VITE_PRIVY_APP_ID=
VITE_SOLANA_NETWORK=mainnet-beta        # or devnet for non-money testing
VITE_SOLANA_RPC=                        # Helius mainnet RPC
VITE_USDC_MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v   # mainnet USDC
VITE_HELIUS_API_KEY=                    # already used by useWalletUsdBalance
# Stripe on-ramp / Bridge keys are configured in Privy's dashboard, not the client.
```

---

## 12. What does NOT change

- `record_move` and the on-chain chess validation — money-agnostic.
- The session-key + Ephemeral Rollup move flow — keeps moves popup-free and off Privy.
- The escrow→winner *direct payout* posture (FACT 2) — still direct, just in USDC.
- The flat-fee model (10p PvP / 50p tournament entry) — same economics, USDC-denominated.

The net effect: **same trustless engine, a front door a non-crypto person can walk through.**
