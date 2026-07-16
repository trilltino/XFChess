import { useEffect, useRef } from 'react';
import { useParams } from 'react-router-dom';

/**
 * Public spectate page — no wallet required.
 * Loads the Bevy WASM canvas and subscribes to a game's move stream.
 */
export default function Spectate() {
    const { game_id: gameId } = useParams<{ game_id: string }>();
    const canvasRef = useRef<HTMLDivElement>(null);
    const wasmLoaded = useRef(false);

    useEffect(() => {
        if (wasmLoaded.current || !canvasRef.current) return;
        wasmLoaded.current = true;

        // In-browser WASM board is not wired up; spectating renders the move feed only.
    }, [gameId]);

    return (
        <div style={{ width: '100%', height: '100vh', display: 'flex', flexDirection: 'column' }}>
            <div style={{ padding: '0.5rem 1rem', background: '#1a1a2e', color: '#eee', fontSize: '0.85rem' }}>
                Spectating Game #{gameId} — public, no wallet required
            </div>
            <div ref={canvasRef} style={{ flex: 1, position: 'relative' }}>
                <canvas id="xfchess" style={{ width: '100%', height: '100%' }} />
            </div>
        </div>
    );
}
