import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';

interface StandingsEntry {
    player_id: string;
    score: number;
    buchholz: number;
    rank: number;
}

/**
 * Live tournament standings — subscribes to Braid-HTTP for real-time updates.
 * Gated: requires connected wallet.
 */
export default function TournamentStandings() {
    const { id } = useParams<{ id: string }>();
    const [standings, setStandings] = useState<StandingsEntry[]>([]);
    const [round, setRound] = useState<number>(0);

    useEffect(() => {
        if (!id) return;
        // Subscribe to Braid-HTTP for live standings patches
        const evtSource = new EventSource(`/braid/tournament/${id}/standings`);
        evtSource.onmessage = (e) => {
            try {
                const data = JSON.parse(e.data);
                if (data.standings) setStandings(data.standings);
                if (data.round) setRound(data.round);
            } catch {
                // ignore malformed
            }
        };
        evtSource.onerror = () => evtSource.close();
        return () => evtSource.close();
    }, [id]);

    return (
        <div style={{ maxWidth: 720, margin: '2rem auto', padding: '0 1rem', color: '#eee' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <h2>Standings — Tournament #{id}</h2>
                <Link to={`/tournament/${id}`} style={{ color: '#6c5ce7', fontSize: '0.9rem' }}>
                    Back
                </Link>
            </div>
            {round > 0 && <p style={{ color: '#aaa' }}>After round {round}</p>}
            <table style={{ width: '100%', borderCollapse: 'collapse', marginTop: '1rem' }}>
                <thead>
                    <tr style={{ borderBottom: '1px solid #333', textAlign: 'left' }}>
                        <th style={{ padding: '0.5rem' }}>#</th>
                        <th style={{ padding: '0.5rem' }}>Player</th>
                        <th style={{ padding: '0.5rem' }}>Score</th>
                        <th style={{ padding: '0.5rem' }}>Buchholz</th>
                    </tr>
                </thead>
                <tbody>
                    {standings.map((entry) => (
                        <tr key={entry.player_id} style={{ borderBottom: '1px solid #222' }}>
                            <td style={{ padding: '0.5rem' }}>{entry.rank}</td>
                            <td style={{ padding: '0.5rem', fontFamily: 'monospace', fontSize: '0.85rem' }}>
                                {entry.player_id.slice(0, 8)}…
                            </td>
                            <td style={{ padding: '0.5rem' }}>{entry.score}</td>
                            <td style={{ padding: '0.5rem' }}>{entry.buchholz}</td>
                        </tr>
                    ))}
                    {standings.length === 0 && (
                        <tr>
                            <td colSpan={4} style={{ padding: '2rem', textAlign: 'center', color: '#666' }}>
                                No standings yet — tournament not started
                            </td>
                        </tr>
                    )}
                </tbody>
            </table>
        </div>
    );
}
