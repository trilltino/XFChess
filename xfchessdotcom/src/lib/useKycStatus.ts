import { useState, useEffect } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';

export interface KycStatus {
    verified: boolean;
    verified_at: number | null;
    country: string | null;
    requires_kyc: boolean;
}

const CACHE: Record<string, { status: KycStatus; ts: number }> = {};
const CACHE_TTL_MS = 60_000; // re-check at most once per minute

export function useKycStatus() {
    const { publicKey, connected } = useWallet();
    const [status, setStatus] = useState<KycStatus | null>(null);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        if (!connected || !publicKey) {
            setStatus(null);
            return;
        }

        const key = publicKey.toBase58();
        const cached = CACHE[key];
        if (cached && Date.now() - cached.ts < CACHE_TTL_MS) {
            setStatus(cached.status);
            return;
        }

        setLoading(true);
        const backendUrl = (import.meta as any).env?.VITE_BACKEND_URL || 'http://localhost:8090';

        fetch(`${backendUrl}/identity/status/${key}`)
            .then(r => r.ok ? r.json() : null)
            .then((data: KycStatus | null) => {
                if (data) {
                    CACHE[key] = { status: data, ts: Date.now() };
                    setStatus(data);
                } else {
                    setStatus({ verified: false, verified_at: null, country: null, requires_kyc: true });
                }
            })
            .catch(() => {
                setStatus({ verified: false, verified_at: null, country: null, requires_kyc: true });
            })
            .finally(() => setLoading(false));
    }, [connected, publicKey]);

    const invalidate = () => {
        if (publicKey) delete CACHE[publicKey.toBase58()];
    };

    return { kycStatus: status, kycLoading: loading, invalidateKyc: invalidate };
}
