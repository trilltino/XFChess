import { useState, useEffect, useCallback, useRef } from 'react';

export interface SolGbpPrice {
    solGbp: number | null;
    loading: boolean;
    updatedAt: Date | null;
    error: boolean;
}

const HELIUS_KEY = import.meta.env.VITE_HELIUS_API_KEY as string | undefined;
// Native SOL mint address used by Helius token-price endpoint
const SOL_MINT = 'So11111111111111111111111111111111111111112';

async function fetchSolGbp(): Promise<number> {
    // Step 1 — Helius token-price API gives SOL/USD (requires API key).
    // Step 2 — CoinGecko gives SOL/USD and SOL/GBP in one call (no key needed).
    // We combine both: Helius USD price × (CoinGecko GBP / CoinGecko USD) for accuracy.
    const cgPromise = fetch(
        'https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd,gbp',
        { cache: 'no-store' },
    ).then(r => { if (!r.ok) throw new Error('cg'); return r.json() as Promise<{ solana: { usd: number; gbp: number } }>; });

    if (HELIUS_KEY) {
        const heliusPromise = fetch(
            `https://api.helius.xyz/v0/token-price?id=${SOL_MINT}&api-key=${HELIUS_KEY}`,
            { cache: 'no-store' },
        ).then(r => { if (!r.ok) throw new Error('helius'); return r.json() as Promise<{ price: number }>; });

        const [helius, cg] = await Promise.all([heliusPromise, cgPromise]);
        // Use Helius for USD, derive GBP via CoinGecko's USD/GBP ratio
        const gbpPerUsd = cg.solana.gbp / cg.solana.usd;
        return helius.price * gbpPerUsd;
    }

    // No Helius key — fall back to CoinGecko SOL/GBP directly
    const cg = await cgPromise;
    return cg.solana.gbp;
}

export function useSolGbpPrice(): SolGbpPrice & { refresh: () => void } {
    const [state, setState] = useState<SolGbpPrice>({
        solGbp: null,
        loading: true,
        updatedAt: null,
        error: false,
    });
    const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

    const refresh = useCallback(async () => {
        setState(s => ({ ...s, loading: true, error: false }));
        try {
            const solGbp = await fetchSolGbp();
            setState({ solGbp, loading: false, updatedAt: new Date(), error: false });
        } catch {
            setState(s => ({ ...s, loading: false, error: true }));
        }
    }, []);

    useEffect(() => {
        refresh();
        timerRef.current = setInterval(refresh, 60_000);
        return () => { if (timerRef.current) clearInterval(timerRef.current); };
    }, [refresh]);

    return { ...state, refresh };
}
