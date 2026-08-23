/**
 * Privy's embedded Solana wallet → `WalletSource`.
 *
 * Privy's signing API is bytes-in / bytes-out:
 *
 * ```ts
 * signMessage({ message: Uint8Array, wallet }) -> { signature: Uint8Array }
 * signTransaction({ transaction: Uint8Array, wallet }) -> { signedTransaction: Uint8Array }
 * ```
 *
 * which is exactly the shape the bridge speaks (`tauri/src/main.rs` sends raw
 * serialized transaction bytes over TCP). So nothing here has to know or care
 * that Privy is built on `@solana/kit` while the rest of this app is on
 * `@solana/web3.js` — no typed transaction object ever crosses the boundary.
 *
 * Building the source is a plain function rather than a hook so it can be called
 * from event handlers; the hooks themselves are called by the component and
 * their results passed in.
 */
import bs58 from 'bs58';
import type { WalletSource } from './types';

/**
 * Generic over the wallet type rather than naming Privy's
 * `ConnectedStandardSolanaWallet` directly. Two reasons: this module does not
 * import the SDK's types (keeping it trivially unit-testable with a fake), and
 * the caller's concrete type flows straight through to the `signMessage` /
 * `signTransaction` it passes in — a structural stand-in was rejected by
 * TypeScript because those hooks demand the full class, private fields included.
 */
type SignMessageFn<W> = (input: {
  message: Uint8Array;
  wallet: W;
}) => Promise<{ signature: Uint8Array }>;
type SignTransactionFn<W> = (input: {
  transaction: Uint8Array;
  wallet: W;
}) => Promise<{ signedTransaction: Uint8Array }>;

export function privyWalletSource<W extends { address: string }>(
  wallet: W,
  signMessage: SignMessageFn<W>,
  signTransaction: SignTransactionFn<W>
): WalletSource {
  return {
    kind: 'privy',
    pubkey: wallet.address,
    // No provider object: an embedded wallet has no user-selected cluster, so
    // there is nothing for `ensureDevnet` to nudge. Call sites must skip that
    // step for this kind rather than reaching in here for something to poke.
    provider: null,
    signRaw: async (msg: string) => {
      const { signature } = await signMessage({
        message: new TextEncoder().encode(msg),
        wallet,
      });
      return bs58.encode(signature);
    },
    signTransaction: async (tx: Uint8Array) => {
      const { signedTransaction } = await signTransaction({ transaction: tx, wallet });
      return signedTransaction;
    },
  };
}
