import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '@solana/wallet-adapter-react';

interface ScheduleStatus {
    phase: string;
    seconds_until_start: number | null;
    current_players: number;
    min_players: number;
    max_players: number;
}

/**
 * Tournament detail page — live countdown, register button, standings link.
 * Gated: requires connected wallet for registration.
 */
export default function TournamentDetail() {
    const { id } = useParams<{ id: string }>();
    const { publicKey, connected } = useWallet();
    const [status, setStatus] = useState<ScheduleStatus | null>(null);
    const [countdown, setCountdown] = useState<string>('');

    useEffect(() => {
        if (!id) return;
        const fetchStatus = async () => {
            try {
                const resp = await fetch(`/tournament/${id}/schedule-status`);
                if (resp.ok) {
                    const data = await resp.json();
                    setStatus(data);
                }
            } catch {
                // backend not reachable
            }
        };
        fetchStatus();
        const interval = setInterval(fetchStatus, 30_000);
        return () => clearInterval(interval);
    }, [id]);

    useEffect(() => {
        if (!status?.seconds_until_start || status.seconds_until_start <= 0) {
            setCountdown(status?.phase === 'active' ? 'Tournament in progress' : '');
            return;
        }
        const tick = () => {
            const s = status.seconds_until_start!;
            const d = Math.floor(s / 86400);
            const h = Math.floor((s % 86400) / 3600);
            const m = Math.floor((s % 3600) / 60);
            setCountdown(
                d > 0 ? `${d}d ${h}h ${m}m` : h > 0 ? `${h}h ${m}m` : `${m}m`
            );
        };
        tick();
        const iv = setInterval(() => {
            setStatus((prev) =>
                prev && prev.seconds_until_start
                    ? { ...prev, seconds_until_start: prev.seconds_until_start - 1 }
                    : prev
            );
        }, 1000);
        return () => clearInterval(iv);
    }, [status]);

    const handleRegister = async () => {
        if (!publicKey || !id) return;
        // In production: build register + authorize_tournament_session TX
        // and send via wallet adapter. For now, log the intent.
        console.log('[tournament] Register intent:', { tournamentId: id, wallet: publicKey.toBase58() });
    };

    return (
        <div style={{ maxWidth: 720, margin: '2rem auto', padding: '0 1rem', color: '#eee' }}>
            <h2>Tournament #{id}</h2>

            {status && (
                <div style={{ background: '#1a1a2e', borderRadius: 8, padding: '1.5rem', marginBottom: '1rem' }}>
                    <div style={{ fontSize: '1.1rem', marginBottom: '0.5rem' }}>
                        Phase: <strong>{status.phase}</strong>
                    </div>
                    {countdown && (
                        <div style={{ fontSize: '2rem', fontWeight: 700, margin: '1rem 0' }}>
                            {countdown}
                        </div>
                    )}
                    <div style={{ display: 'flex', gap: '2rem', fontSize: '0.9rem', color: '#aaa' }}>
                        <span>Players: {status.current_players}/{status.max_players}</span>
                        <span>Min: {status.min_players}</span>
                    </div>
                </div>
            )}

            {connected ? (
                <button
                    onClick={handleRegister}
                    disabled={status?.phase !== 'countdown' && status?.phase !== 'grace_period'}
                    style={{
                        padding: '0.75rem 2rem',
                        background: '#6c5ce7',
                        color: '#fff',
                        border: 'none',
                        borderRadius: 6,
                        cursor: 'pointer',
                        fontSize: '1rem',
                    }}
                >
                    Register (1 wallet popup)
                </button>
            ) : (
                <p style={{ color: '#888' }}>Connect wallet to register</p>
            )}

            <div style={{ marginTop: '1.5rem', display: 'flex', gap: '1rem' }}>
                <Link to={`/tournament/${id}/standings`} style={{ color: '#6c5ce7' }}>
                    Standings
                </Link>
                <Link to={`/tournament/${id}/play`} style={{ color: '#6c5ce7' }}>
                    Play
                </Link>
                <Link to={`/spectate/${id}`} style={{ color: '#6c5ce7' }}>
                    Spectate
                </Link>
            </div>
        </div>
    );
}
