import { useEffect, useMemo, useRef, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '@solana/wallet-adapter-react';
import { getTournamentMatch, getSwissCurrentRound, recordSwissResult } from '../lib/api';

interface MatchState {
    found: boolean;
    round?: number | null;
    board?: number | null;
    opponent?: string;
    is_bye?: boolean;
}

/**
 * In-browser play page — loads Bevy WASM canvas for the player's current match.
 * Gated: requires connected wallet.
 */
export default function TournamentPlay() {
    const { id } = useParams<{ id: string }>();
    const { connected, publicKey } = useWallet();
    const canvasRef = useRef<HTMLDivElement>(null);
    const wasmLoaded = useRef(false);
    const [matchState, setMatchState] = useState<MatchState | null>(null);
    const [roundState, setRoundState] = useState<{ round: number; total_rounds: number; is_active: boolean } | null>(null);
    const [statusMessage, setStatusMessage] = useState('Loading tournament match...');
    const player = publicKey?.toBase58();

    const currentRoundLabel = useMemo(() => {
        if (!roundState) return '';
        return roundState.total_rounds > 0 ? `Round ${roundState.round} / ${roundState.total_rounds}` : `Round ${roundState.round}`;
    }, [roundState]);

    useEffect(() => {
        if (wasmLoaded.current || !canvasRef.current) return;
        wasmLoaded.current = true;

        // In-browser WASM board is not wired up; play happens in the desktop client.
    }, [id, publicKey]);

    useEffect(() => {
        if (!id) return;
        let mounted = true;
        const load = async () => {
            try {
                const round = await getSwissCurrentRound(id);
                if (!mounted) return;
                setRoundState(round);

                if (!player) {
                    setStatusMessage('Connect wallet to see your Swiss pairing.');
                    return;
                }

                const match = await getTournamentMatch(id, player);
                if (!mounted) return;
                setMatchState(match);

                if (!match.found) {
                    setStatusMessage('Waiting for your next pairing...');
                } else if (match.is_bye) {
                    setStatusMessage(`You received a bye in ${currentRoundLabel || 'this round'}.`);
                } else {
                    setStatusMessage(`Playing ${currentRoundLabel || `Round ${match.round}`}`);
                }
            } catch (e) {
                console.error('[play] Failed to load Swiss state:', e);
                if (mounted) setStatusMessage('Failed to load Swiss tournament state.');
            }
        };

        load();
        const iv = setInterval(load, 10_000);
        return () => {
            mounted = false;
            clearInterval(iv);
        };
    }, [id, player, currentRoundLabel]);

    const submitDemoResult = async (result: '1-0' | '0-1' | '0.5-0.5') => {
        if (!id || !matchState?.found || !matchState.round || !matchState.board) return;
        try {
            await recordSwissResult(id, {
                round: matchState.round,
                board: matchState.board,
                result,
            });
            setStatusMessage('Result submitted. Waiting for the other boards in this round...');
        } catch (e) {
            console.error('[play] Failed to submit Swiss result:', e);
            setStatusMessage('Result submission failed. Please retry.');
        }
    };

    if (!connected) {
        return (
            <div style={{ maxWidth: 720, margin: '2rem auto', padding: '0 1rem', color: '#eee' }}>
                <h2>Tournament Play</h2>
                <p style={{ color: '#888' }}>Connect your wallet to play in this tournament.</p>
                <Link to={`/tournament/${id}`} style={{ color: '#6c5ce7' }}>Back to tournament</Link>
            </div>
        );
    }

    return (
        <div style={{ width: '100%', height: '100vh', display: 'flex', flexDirection: 'column' }}>
            <div style={{ padding: '0.5rem 1rem', background: '#1a1a2e', color: '#eee', fontSize: '0.85rem', display: 'flex', justifyContent: 'space-between' }}>
                <span>Tournament #{id} — {currentRoundLabel || 'Swiss'}</span>
                <Link to={`/tournament/${id}/standings`} style={{ color: '#6c5ce7' }}>Standings</Link>
            </div>
            <div style={{ padding: '1rem', background: '#111827', color: '#eee', borderBottom: '1px solid #222' }}>
                <div style={{ marginBottom: '0.5rem', fontWeight: 600 }}>{statusMessage}</div>
                {matchState?.found && !matchState.is_bye && (
                    <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
                        <button onClick={() => submitDemoResult('1-0')} style={{ padding: '0.5rem 0.9rem' }}>Submit 1-0</button>
                        <button onClick={() => submitDemoResult('0-1')} style={{ padding: '0.5rem 0.9rem' }}>Submit 0-1</button>
                        <button onClick={() => submitDemoResult('0.5-0.5')} style={{ padding: '0.5rem 0.9rem' }}>Submit Draw</button>
                    </div>
                )}
                {matchState?.found && matchState.is_bye && (
                    <p style={{ margin: 0, color: '#9ca3af' }}>Your round is complete for this bye pairing. Wait for the remaining boards.</p>
                )}
            </div>
            <div ref={canvasRef} style={{ flex: 1, position: 'relative' }}>
                <canvas id="xfchess" style={{ width: '100%', height: '100%' }} />
            </div>
        </div>
    );
}
