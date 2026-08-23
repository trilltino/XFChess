/**
 * Browser-extension providers (Phantom, Solflare) → `WalletSource`.
 *
 * This is a straight extraction of what `WalletStep.handleConnect` used to do
 * inline. Behaviour is deliberately unchanged; the notes below record why each
 * odd-looking bit is the way it is, because every one of them was a fix.
 */
import bs58 from 'bs58';
import type { WalletSource, WalletKind } from './types';
import { WALLET_LABEL } from './types';

export type ExtensionKind = Extract<WalletKind, 'phantom' | 'solflare'>;

export const EXTENSION_META: Record<
  ExtensionKind,
  { label: string; installUrl: string; provider: () => any }
> = {
  phantom: {
    label: WALLET_LABEL.phantom,
    installUrl: 'https://phantom.app/',
    provider: () => (window as any).phantom?.solana,
  },
  solflare: {
    label: WALLET_LABEL.solflare,
    installUrl: 'https://solflare.com/',
    provider: () => (window as any).solflare,
  },
};

export function withTimeout<T>(p: Promise<T>, ms: number, label: string): Promise<T> {
  return Promise.race([
    p,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`${label} timed out after ${ms / 1000}s`)), ms)
    ),
  ]);
}

/**
 * Connects an extension and returns a `WalletSource`.
 *
 * Always issues a real approval prompt — no `onlyIfTrusted` fast path. That
 * fast path resolved with no popup at all once an extension had trusted this
 * origin once, which is what made "Connect Wallet" silently reuse whichever
 * wallet was approved first instead of asking every time. Especially confusing
 * across multiple windows sharing one real browser profile: each window is its
 * own bridge origin by port, but the extension's own trust memory is not
 * necessarily scoped that finely.
 */
export async function connectExtension(kind: ExtensionKind): Promise<WalletSource> {
  const meta = EXTENSION_META[kind];
  const provider = meta.provider();
  if (!provider) throw new Error(`${meta.label} extension not detected.`);

  const resp: any = await withTimeout(provider.connect(), 30000, `${meta.label} connection`);

  // Phantom returns publicKey on the response; Solflare puts it on the provider
  // after connect and returns nothing useful. Check both, in that order.
  const pubkey: string =
    resp?.publicKey?.toBase58?.() ??
    resp?.publicKey?.toString?.() ??
    provider.publicKey?.toBase58?.() ??
    provider.publicKey?.toString?.();

  if (!pubkey) throw new Error('No public key returned from wallet');

  return {
    kind,
    pubkey,
    provider,
    // Signs raw bytes with no "utf8" argument, to avoid the off-chain message
    // prefix Phantom >= 0.16 applies in that mode. The backend verifies a bare
    // ed25519 signature over the message, so a prefixed one fails.
    signRaw: async (msg: string) => {
      const bytes = new TextEncoder().encode(msg);
      const { signature } = await withTimeout<{ signature: Uint8Array }>(
        provider.signMessage(bytes),
        60000,
        `${meta.label} signature`
      );
      return bs58.encode(signature);
    },
    signTransaction: async () => {
      // Extensions sign typed transaction objects, not bytes, and the signing
      // path in TransactionSigner already handles that shape directly (it needs
      // the deserialized tx anyway, to refresh the blockhash). Routing it
      // through here would mean deserializing, re-serializing and deserializing
      // again for no gain.
      throw new Error(
        'signTransaction is not used for extension wallets — see TransactionSigner.signWithExtension'
      );
    },
  };
}
