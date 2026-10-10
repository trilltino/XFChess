/** Adapt Phantom and Solflare providers to WalletSource. */
import bs58 from 'bs58';
import type { WalletSource, WalletKind } from './types';
import { detectSolanaTxCapabilities, WALLET_LABEL } from './types';

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
 * Connect with a real approval prompt rather than onlyIfTrusted so shared
 * browser trust cannot silently select a previous wallet.
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
    txCapabilities: detectSolanaTxCapabilities(kind, provider),
    // Sign raw bytes without utf8 mode: Phantom’s message prefix would invalidate backend verification.
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
      // Extension signing uses typed transactions in TransactionSigner to refresh
      // blockhashes; do not add a redundant bytes conversion here.
      throw new Error(
        'signTransaction is not used for extension wallets — see TransactionSigner.signWithExtension'
      );
    },
  };
}
