/**
 * One uniform handle on "a wallet the user just proved control of", regardless
 * of whether it came from a browser extension or from Privy's embedded wallet.
 *
 * ## Why this exists
 *
 * `WalletStep.handleConnect` used to interleave two concerns in one function:
 * provider mechanics (call `.connect()`, dig the pubkey out of whichever shape
 * that particular extension returns, wire up `signMessage`) and the XFChess
 * auth flow (`check-wallet` → `login`/`register` → `POST /wallet` → hand off to
 * the game). Adding a third provider to that shape would have meant a third
 * copy of the auth flow, and the auth flow is the part carrying hard-won
 * invariants — see `authenticateWithBackend` in App.tsx for the three separate
 * bugs encoded in its ordering.
 *
 * So: providers produce a `WalletSource`; the auth flow consumes one.
 *
 * ## Contract
 *
 * - `pubkey` is base58 and already known — building a WalletSource implies the
 *   provider connection has completed.
 * - `signRaw` signs the **raw UTF-8 bytes** of `msg` and returns a base58
 *   signature. It must NOT apply the off-chain message prefix some wallets add
 *   for `signMessage(msg, "utf8")`; the backend verifies a bare ed25519
 *   signature over `xfchess:<action>:<ts>`.
 * - `signTransaction` takes and returns **serialized transaction bytes**. Bytes
 *   in, bytes out is what lets one code path serve both an extension (which
 *   speaks `@solana/web3.js`) and Privy (which speaks `@solana/kit`) — no typed
 *   transaction object crosses this boundary.
 */
export type WalletKind = 'phantom' | 'solflare' | 'privy';

export type WalletSource = {
  kind: WalletKind;
  /** Base58 public key. */
  pubkey: string;
  /** Signs raw UTF-8 bytes of `msg`; returns a base58 signature. */
  signRaw: (msg: string) => Promise<string>;
  /** Signs serialized transaction bytes; returns serialized signed bytes. */
  signTransaction: (tx: Uint8Array) => Promise<Uint8Array>;
  /**
   * The underlying provider object, for the few call sites that still need it
   * (e.g. `ensureDevnet`). `null` for Privy: an embedded wallet has no
   * user-selected cluster to nudge, so there is nothing to reach into.
   */
  provider: unknown | null;
};

/** Human-facing label per provider. */
export const WALLET_LABEL: Record<WalletKind, string> = {
  phantom: 'Phantom',
  solflare: 'Solflare',
  privy: 'Privy',
};
