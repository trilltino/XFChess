/**
 * A connected wallet shared by extension and embedded providers.
 * `pubkey` is base58. `signRaw` signs UTF-8 bytes without a wallet-specific prefix
 * and returns a base58 signature. Transactions cross this boundary as serialized bytes.
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
   * Underlying extension provider; null for embedded wallets without a
   * user-selected cluster.
   */
  provider: unknown | null;
  /** Best-effort transaction-version capabilities for builder policy. */
  txCapabilities?: SolanaTxCapabilities;
};

export type SolanaTxCapabilities = {
  canSignLegacy: boolean;
  canSignV0: boolean;
  canSignV1: boolean;
  canRefreshBlockhash: boolean;
  canHandlePartialSignatures: boolean;
  source: WalletKind | 'unknown';
  reason?: string;
};

export function defaultSolanaTxCapabilities(
  source: WalletKind | 'unknown',
): SolanaTxCapabilities {
  return {
    canSignLegacy: true,
    canSignV0: true,
    canSignV1: false,
    canRefreshBlockhash: false,
    canHandlePartialSignatures: true,
    source,
    reason: 'v1 signing is disabled until the connected wallet declares support.',
  };
}

export function detectSolanaTxCapabilities(
  source: WalletKind | 'unknown',
  provider?: unknown,
): SolanaTxCapabilities {
  const base = defaultSolanaTxCapabilities(source);
  const features = (provider as any)?.features ?? {};
  const hasExplicitV1 =
    Boolean(features['solana:signTransaction:v1']) ||
    Boolean(features['solana:signAndSendTransaction:v1']);
  return {
    ...base,
    canSignV1: hasExplicitV1,
    reason: hasExplicitV1
      ? 'wallet-standard provider advertises transaction v1 signing'
      : base.reason,
  };
}

/** Human-facing label per provider. */
export const WALLET_LABEL: Record<WalletKind, string> = {
  phantom: 'Phantom',
  solflare: 'Solflare',
  privy: 'Privy',
};
