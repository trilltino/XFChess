/**
 * Adapt Privy signing to WalletSource using raw serialized bytes. Provider hooks
 * are passed in so the adapter can be constructed from event handlers.
 */
import bs58 from 'bs58';
import type { WalletSource } from './types';
import { detectSolanaTxCapabilities } from './types';

/**
 * Preserve the caller's concrete wallet type for SDK hooks while allowing
 * fake wallets in adapter tests.
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
    txCapabilities: detectSolanaTxCapabilities('privy', wallet),
    // Embedded wallets have no provider-selected cluster; callers must skip ensureDevnet.
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
