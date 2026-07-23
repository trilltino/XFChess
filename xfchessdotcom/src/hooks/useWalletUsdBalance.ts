import { useState, useEffect, useCallback, useRef } from 'react';
import { Connection, PublicKey } from '@solana/web3.js';

const SOL_MINT = 'So11111111111111111111111111111111111111112';
const TOKEN_PROGRAM_ID = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const HELIUS_KEY = import.meta.env.VITE_HELIUS_API_KEY as string | undefined;

export interface TokenHolding {
  mint: string;
  uiAmount: number;
  decimals: number;
  usdPrice: number | null;
  usdValue: number | null;
}

export interface WalletUsdBalance {
  solBalance: number;
  solUsdPrice: number | null;
  solUsdValue: number | null;
  tokens: TokenHolding[];
  totalUsdValue: number | null;
  loading: boolean;
  error: string | null;
  lastUpdated: Date | null;
}

function parseTokenAccountData(accountInfo: any): { mint: string; uiAmount: number; decimals: number } | null {
  try {
    const parsed = accountInfo?.data?.parsed?.info;
    if (!parsed) return null;
    const mint = parsed.mint as string;
    const tokenAmount = parsed.tokenAmount;
    const uiAmount = tokenAmount?.uiAmount as number;
    const decimals = tokenAmount?.decimals as number;
    if (typeof uiAmount !== 'number' || uiAmount <= 0) return null;
    return { mint, uiAmount, decimals };
  } catch {
    return null;
  }
}

async function fetchHeliusPrice(mint: string): Promise<number | null> {
  if (!HELIUS_KEY) return null;
  try {
    const res = await fetch(
      `https://api.helius.xyz/v0/token-price?id=${mint}&api-key=${HELIUS_KEY}`,
      { cache: 'no-store' }
    );
    if (!res.ok) return null;
    const data = (await res.json()) as any;
    return typeof data.price === 'number' ? data.price : null;
  } catch {
    return null;
  }
}

export async function getWalletUsdBalance(
  connection: Connection,
  publicKey: PublicKey
): Promise<Omit<WalletUsdBalance, 'loading' | 'error'>> {
  const lamports = await connection.getBalance(publicKey);
  const solBalance = lamports / 1e9;

  const tokenResp = await (connection as any).getParsedTokenAccountsByOwner(
    publicKey,
    { programId: TOKEN_PROGRAM_ID },
    'confirmed'
  );

  const tokenInfos = (tokenResp.value as any[])
    .map((v: any) => parseTokenAccountData(v.account))
    .filter((t: any): t is { mint: string; uiAmount: number; decimals: number } => t !== null);

  const solPrice = await fetchHeliusPrice(SOL_MINT);

  const tokens: TokenHolding[] = [];
  for (const t of tokenInfos) {
    const price = await fetchHeliusPrice(t.mint);
    tokens.push({
      mint: t.mint,
      uiAmount: t.uiAmount,
      decimals: t.decimals,
      usdPrice: price,
      usdValue: price !== null ? t.uiAmount * price : null,
    });
  }

  const solUsdValue = solPrice !== null ? solBalance * solPrice : null;
  const tokenUsdValues = tokens.map(t => t.usdValue).filter((v): v is number => v !== null);
  const totalTokenUsd = tokenUsdValues.length > 0 ? tokenUsdValues.reduce((a, b) => a + b, 0) : null;

  let totalUsdValue: number | null = null;
  if (solUsdValue !== null && totalTokenUsd !== null) {
    totalUsdValue = solUsdValue + totalTokenUsd;
  } else if (solUsdValue !== null) {
    totalUsdValue = solUsdValue;
  } else if (totalTokenUsd !== null) {
    totalUsdValue = totalTokenUsd;
  }

  return {
    solBalance,
    solUsdPrice: solPrice,
    solUsdValue,
    tokens,
    totalUsdValue,
    lastUpdated: new Date(),
  };
}

export function useWalletUsdBalance(
  connection: Connection | null,
  publicKey: PublicKey | null
): WalletUsdBalance & { refresh: () => void } {
  const [state, setState] = useState<WalletUsdBalance>({
    solBalance: 0,
    solUsdPrice: null,
    solUsdValue: null,
    tokens: [],
    totalUsdValue: null,
    loading: false,
    error: null,
    lastUpdated: null,
  });

  const refresh = useCallback(async () => {
    if (!connection || !publicKey) {
      setState(s => ({ ...s, loading: false }));
      return;
    }
    setState(s => ({ ...s, loading: true, error: null }));
    try {
      const data = await getWalletUsdBalance(connection, publicKey);
      setState({ ...data, loading: false, error: null });
    } catch (err: any) {
      setState(s => ({
        ...s,
        loading: false,
        error: err?.message || 'Failed to fetch wallet balance',
      }));
    }
  }, [connection, publicKey]);

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    refresh();
    timerRef.current = setInterval(refresh, 15_000);
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [refresh]);

  return { ...state, refresh };
}
