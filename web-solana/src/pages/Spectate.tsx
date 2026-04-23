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

        // WASM temporarily disabled - see xfchess-wasm/pkg/ for stub
        // const loadWasm = async () => {
        //     try {
        //         const wasm = await import('/wasm/xfchess_wasm.js');
        //         await wasm.default();
        //         if (gameId) {
        //             wasm.load_game(Number(gameId), 'spectate');
        //         }
        //     } catch (err) {
        //         console.error('[spectate] WASM load failed:', err);
        //         if (canvasRef.current) {
        //             canvasRef.current.innerHTML =
        //                 '<p style="color:#888;text-align:center;padding:2rem">WASM not available — build with scripts/build_wasm.bat</p>';
        //         }
        //     }
        // };
        // loadWasm();
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
