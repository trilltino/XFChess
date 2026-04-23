import { useEffect, useRef } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useWallet } from '@solana/wallet-adapter-react';

/**
 * In-browser play page — loads Bevy WASM canvas for the player's current match.
 * Gated: requires connected wallet.
 */
export default function TournamentPlay() {
    const { id } = useParams<{ id: string }>();
    const { publicKey, connected, signTransaction } = useWallet();
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
        //
        //         // Register wallet signing bridge
        //         if (signTransaction) {
        //             wasm.sign_callback(async (txBytes: Uint8Array) => {
        //                 console.log('[play] Sign callback invoked with', txBytes.length, 'bytes');
        //                 return txBytes;
        //             });
        //         }
        //
        //         wasm.load_tournament(Number(id));
        //     } catch (err) {
        //         console.error('[play] WASM load failed:', err);
        //         if (canvasRef.current) {
        //             canvasRef.current.innerHTML =
        //                 '<p style="color:#888;text-align:center;padding:2rem">WASM not available</p>';
        //         }
        //     }
        // };
        // loadWasm();
    }, [id, signTransaction]);

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
                <span>Tournament #{id} — Playing</span>
                <Link to={`/tournament/${id}/standings`} style={{ color: '#6c5ce7' }}>Standings</Link>
            </div>
            <div ref={canvasRef} style={{ flex: 1, position: 'relative' }}>
                <canvas id="xfchess" style={{ width: '100%', height: '100%' }} />
            </div>
        </div>
    );
}
