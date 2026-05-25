import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '@solana/wallet-adapter-react';
import WagerPriceWidget from '../components/WagerPriceWidget';
import { getSwissCurrentRound, getSwissPairings, getTournamentMatch } from '../lib/api';

interface ScheduleStatus {
    phase: string;
    seconds_until_start: number | null;
    current_players: number;
    min_players: number;
    max_players: number;
}

interface MatchInfo {
    round: number;
    board: number;
    opponent: string;
}

interface TournamentInfo {
    prize_pool: number;
    entry_fee_lamports: number;
    platform_fee_lamports: number;
    max_players: number;
    players: string[];
    prize_shares: number[];
}

function formatLamports(lamports: number): string {
    return (lamports / 1_000_000_000).toFixed(4) + ' SOL';
}

function describePrizeType(shares: number[]): string {
    if (shares[0] === 10000) return 'Winner Takes All';
    const positions = shares.filter(s => s > 0).length;
    return `Top ${positions}`;
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
    const [currentRound, setCurrentRound] = useState<number>(0);
    const [totalRounds, setTotalRounds] = useState<number>(0);
    const [myMatch, setMyMatch] = useState<MatchInfo | null>(null);
    const [pairingPreview, setPairingPreview] = useState<string>('');
    const [tournamentInfo, setTournamentInfo] = useState<TournamentInfo | null>(null);

    useEffect(() => {
        if (!id) return;
        let mounted = true;

        const fetchTournamentInfo = async () => {
            try {
                const resp = await fetch(`/tournament/${id}`);
                if (resp.ok && mounted) {
                    const data = await resp.json();
                    setTournamentInfo({
                        prize_pool: data.prize_pool ?? 0,
                        entry_fee_lamports: data.entry_fee_lamports ?? 0,
                        platform_fee_lamports: data.platform_fee_lamports ?? 0,
                        max_players: data.max_players ?? 0,
                        players: data.players ?? [],
                        prize_shares: data.prize_shares ?? [],
                    });
                }
            } catch {
                // backend not reachable
            }
        };

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

        const fetchSwissState = async () => {
            try {
                const round = await getSwissCurrentRound(id);
                if (!mounted) return;
                setCurrentRound(round.round);
                setTotalRounds(round.total_rounds);

                if (publicKey) {
                    const match = await getTournamentMatch(id, publicKey.toBase58());
                    if (!mounted) return;
                    setMyMatch(match);
                    if (match.found && !match.is_bye && match.round) {
                        const pairings = await getSwissPairings(id, match.round);
                        if (!mounted) return;
                        const preview = pairings.pairings.find((p) => p.board === match.board)?.white && pairings.pairings.find((p) => p.board === match.board)?.black
                            ? `Board ${match.board}: ${pairings.pairings.find((p) => p.board === match.board)?.white} vs ${pairings.pairings.find((p) => p.board === match.board)?.black}`
                            : '';
                        setPairingPreview(preview);
                    } else {
                        setPairingPreview('');
                    }
                }
            } catch {
                // ignore if Swiss endpoints are unavailable yet
            }
        };
        fetchTournamentInfo();
        fetchStatus();
        fetchSwissState();
        const tournamentInfoInterval = setInterval(fetchTournamentInfo, 30_000);
        const interval = setInterval(fetchStatus, 30_000);
        const swissInterval = setInterval(fetchSwissState, 10_000);
        return () => {
            mounted = false;
            clearInterval(tournamentInfoInterval);
            clearInterval(interval);
            clearInterval(swissInterval);
        };
    }, [id, publicKey]);

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

            {tournamentInfo && (
                <div style={{
                    background: 'linear-gradient(135deg, #1e1b4b, #312e81)',
                    borderRadius: 8,
                    padding: '1.25rem',
                    marginBottom: '1rem',
                    border: '1px solid #4338ca',
                }}>
                    <div style={{ fontWeight: 700, fontSize: '1rem', marginBottom: '0.75rem', color: '#a5b4fc' }}>
                        Prize Pot
                    </div>
                    <div style={{ display: 'flex', gap: '2rem', flexWrap: 'wrap' }}>
                        <div>
                            <div style={{ fontSize: '1.75rem', fontWeight: 800, color: '#fbbf24' }}>
                                {formatLamports(tournamentInfo.prize_pool)}
                            </div>
                            <div style={{ fontSize: '0.75rem', color: '#9ca3af', marginTop: 2 }}>Total Pot</div>
                        </div>
                        <div>
                            <div style={{ fontSize: '1.5rem', fontWeight: 700, color: '#eee' }}>
                                {tournamentInfo.players.length}<span style={{ color: '#9ca3af', fontSize: '1rem' }}>/{tournamentInfo.max_players}</span>
                            </div>
                            <div style={{ fontSize: '0.75rem', color: '#9ca3af', marginTop: 2 }}>Players</div>
                        </div>
                        <div>
                            <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#34d399' }}>
                                {describePrizeType(tournamentInfo.prize_shares)}
                            </div>
                            <div style={{ fontSize: '0.75rem', color: '#9ca3af', marginTop: 2 }}>Prize Type</div>
                        </div>
                        <div>
                            <div style={{ fontSize: '1rem', fontWeight: 600, color: '#eee' }}>
                                {formatLamports(tournamentInfo.entry_fee_lamports)}
                            </div>
                            <div style={{ fontSize: '0.75rem', color: '#9ca3af', marginTop: 2 }}>Entry Fee</div>
                        </div>
                    </div>
                    {tournamentInfo.prize_shares[0] < 10000 && (
                        <div style={{ marginTop: '0.75rem', display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                            {tournamentInfo.prize_shares.filter(s => s > 0).map((share, i) => (
                                <span key={i} style={{
                                    background: '#1e1b4b',
                                    border: '1px solid #4338ca',
                                    borderRadius: 4,
                                    padding: '2px 8px',
                                    fontSize: '0.75rem',
                                    color: '#a5b4fc',
                                }}>
                                    {i + 1}{['st','nd','rd'][i] ?? 'th'}: {(share / 100).toFixed(0)}%
                                </span>
                            ))}
                        </div>
                    )}
                </div>
            )}

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

            <div style={{ background: '#111827', borderRadius: 8, padding: '1rem', marginBottom: '1rem', color: '#eee' }}>
                <div style={{ fontWeight: 700, marginBottom: '0.5rem' }}>Swiss round status</div>
                <p style={{ margin: 0, color: '#9ca3af' }}>
                    Current round: {currentRound || 'waiting'}{totalRounds ? ` / ${totalRounds}` : ''}
                </p>
                {myMatch?.found && !myMatch.is_bye && (
                    <p style={{ margin: '0.5rem 0 0', color: '#d1d5db' }}>
                        You are {myMatch.your_color} on board {myMatch.board}. {pairingPreview}
                    </p>
                )}
                {myMatch?.found && myMatch.is_bye && (
                    <p style={{ margin: '0.5rem 0 0', color: '#d1d5db' }}>You have a bye this round. Wait for the round to finish and the next round to appear.</p>
                )}
                {myMatch && !myMatch.found && currentRound > 0 && (
                    <p style={{ margin: '0.5rem 0 0', color: '#d1d5db' }}>Your next pairing is not ready yet. This usually means the current round is still running.</p>
                )}
            </div>

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

            <div style={{ marginTop: '2rem' }}>
                <WagerPriceWidget />
            </div>
        </div>
    );
}
