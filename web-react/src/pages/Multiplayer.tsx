import { motion } from 'framer-motion';
import { ArrowLeft, Network, Cpu, Zap, Globe, Shield, GitBranch, Radio } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './Multiplayer.css';

const Multiplayer = () => {
    return (
        <motion.div
            className="multiplayer-container"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.5 }}
        >
            <Link to="/" className="back-btn" style={{ position: 'absolute', top: '2rem', left: '2rem', display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#e63946', textDecoration: 'none', fontWeight: 'bold' }}>
                <ArrowLeft size={18} /> Back
            </Link>

            <div className="multi-header">
                <div className="section-label" style={{ color: '#e63946', fontSize: '0.75rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', marginBottom: '0.75rem' }}>Networking</div>
                <h1>P2P Online Gaming</h1>
                <p>How Braid, Iroh, and Stockfish power decentralized, real-time chess.</p>
            </div>

            {/* ARCHITECTURE OVERVIEW */}
            <section className="mp-section">
                <h2 className="mp-section-title">Architecture Overview</h2>
                <p className="mp-section-desc">
                    XFChess uses a three-layer networking stack to deliver real-time, peer-to-peer gameplay without a central game server.
                    <strong> Braid-HTTP</strong> handles live state synchronisation between clients,
                    <strong> Iroh</strong> provides the underlying hole-punched transport layer for direct peer connections,
                    and <strong> Stockfish</strong> runs locally as a sidecar process to power AI analysis and offline play.
                    Together, these three components eliminate the need for a traditional matchmaking server while preserving sub-second move latency.
                </p>

                <div className="mp-arch-diagram">
                    <div className="mp-arch-node">
                        <div className="mp-arch-icon" style={{ background: 'rgba(168, 85, 247, 0.15)', border: '1px solid rgba(168, 85, 247, 0.3)' }}>
                            <Radio size={24} color="#a855f7" />
                        </div>
                        <span>Braid-HTTP</span>
                        <small>State Sync</small>
                    </div>
                    <div className="mp-arch-arrow">→</div>
                    <div className="mp-arch-node">
                        <div className="mp-arch-icon" style={{ background: 'rgba(230, 57, 70, 0.15)', border: '1px solid rgba(230, 57, 70, 0.3)' }}>
                            <Network size={24} color="#e63946" />
                        </div>
                        <span>Iroh</span>
                        <small>P2P Transport</small>
                    </div>
                    <div className="mp-arch-arrow">→</div>
                    <div className="mp-arch-node">
                        <div className="mp-arch-icon" style={{ background: 'rgba(34, 197, 94, 0.15)', border: '1px solid rgba(34, 197, 94, 0.3)' }}>
                            <Cpu size={24} color="#22c55e" />
                        </div>
                        <span>Stockfish</span>
                        <small>AI Engine</small>
                    </div>
                </div>
            </section>

            <div className="mp-divider" />

            {/* BRAID */}
            <section className="mp-section">
                <div className="mp-tech-header">
                    <div className="mp-tech-icon" style={{ background: 'rgba(168, 85, 247, 0.15)', border: '1px solid rgba(168, 85, 247, 0.3)' }}>
                        <Radio size={28} color="#a855f7" />
                    </div>
                    <div>
                        <div className="mp-tech-label" style={{ color: '#a855f7' }}>State Synchronisation</div>
                        <h2 className="mp-tech-title">Braid-HTTP</h2>
                    </div>
                </div>

                <p className="mp-section-desc">
                    <strong>Braid</strong> is an extension to HTTP that adds live, version-controlled state synchronisation. Instead of polling an endpoint or maintaining a WebSocket, Braid clients subscribe to a resource URL and receive incremental patches whenever the server-side state changes — using standard HTTP semantics. In XFChess, each active game is a Braid resource. When a player makes a move, the Bevy client serialises the updated FEN string and game metadata into a Braid patch and pushes it to the local Braid node. The opponent's client, subscribed to the same resource, receives the patch in real-time and applies it to their local game state.
                </p>

                <div className="mp-feature-grid">
                    <div className="mp-feature-card">
                        <Zap size={20} color="#a855f7" />
                        <h4>HTTP Subscriptions</h4>
                        <p>Braid extends standard HTTP with a <code>Subscribe</code> header. Clients open a long-lived HTTP request and receive incremental JSON patches as the resource changes — no WebSocket or SSE required.</p>
                    </div>
                    <div className="mp-feature-card">
                        <GitBranch size={20} color="#a855f7" />
                        <h4>Version Control</h4>
                        <p>Every state update carries a version vector. If two moves arrive out of order (e.g. due to network jitter), Braid's merge logic resolves conflicts deterministically — the same way Git resolves divergent branches.</p>
                    </div>
                    <div className="mp-feature-card">
                        <Shield size={20} color="#a855f7" />
                        <h4>No Central Server</h4>
                        <p>Each XFChess client runs a lightweight Braid HTTP node locally. Peers connect directly to each other's nodes — there is no matchmaking server that can go offline or censor game state.</p>
                    </div>
                </div>

                <CodeViewer
                    title="src/multiplayer/mod.rs"
                    language="Rust (Braid)"
                    code={`// Broadcast a network message via the local Braid node
tokio::spawn(async move {
    while let Some(msg) = msg_rx.recv().await {
        let json = serde_json::to_vec(&msg)?;
        let version = Version::new(Uuid::new_v4().to_string());
        let update = Update::snapshot(version, json);
        
        // Broadcast move/event to all peers on the game topic
        node.put(GAME_TOPIC, update).await?;
    }
});`}
                />
            </section>

            <div className="mp-divider" />

            {/* IROH */}
            <section className="mp-section">
                <div className="mp-tech-header">
                    <div className="mp-tech-icon" style={{ background: 'rgba(230, 57, 70, 0.15)', border: '1px solid rgba(230, 57, 70, 0.3)' }}>
                        <Network size={28} color="#e63946" />
                    </div>
                    <div>
                        <div className="mp-tech-label" style={{ color: '#e63946' }}>P2P Transport</div>
                        <h2 className="mp-tech-title">Iroh</h2>
                    </div>
                </div>

                <p className="mp-section-desc">
                    <strong>Iroh</strong> is a Rust library from n0 that provides direct, hole-punched peer-to-peer connections using the QUIC transport protocol. It solves the fundamental problem of P2P gaming: most players are behind NAT routers that block inbound connections. Iroh uses a lightweight relay network to coordinate the initial handshake between peers, then establishes a direct QUIC connection that bypasses the relay entirely. In XFChess, Iroh acts as the transport layer beneath Braid — when two players want to connect, their clients exchange Iroh node IDs (derived from their public keys), and Iroh handles the NAT traversal automatically.
                </p>

                <div className="mp-feature-grid">
                    <div className="mp-feature-card">
                        <Globe size={20} color="#e63946" />
                        <h4>NAT Hole Punching</h4>
                        <p>Iroh coordinates simultaneous UDP packets from both peers to punch through NAT firewalls, establishing a direct path without port forwarding or UPnP configuration.</p>
                    </div>
                    <div className="mp-feature-card">
                        <Zap size={20} color="#e63946" />
                        <h4>QUIC Transport</h4>
                        <p>Built on QUIC (the protocol underlying HTTP/3), Iroh provides multiplexed streams, built-in encryption via TLS 1.3, and connection migration — so a game survives a network switch mid-match.</p>
                    </div>
                    <div className="mp-feature-card">
                        <Shield size={20} color="#e63946" />
                        <h4>Cryptographic Identity</h4>
                        <p>Every Iroh node has a public/private keypair. Node IDs are derived from public keys, meaning connections are authenticated by default — you can't impersonate another player's node.</p>
                    </div>
                </div>

                <CodeViewer
                    title="src/multiplayer/mod.rs"
                    language="Rust (Iroh)"
                    code={`// Spawn an Iroh node with Gossip discovery enabled
let config = BraidIrohConfig {
    secret_key: Some(secret_key),
    discovery: DiscoveryConfig::Real,
    proxy_config: None,
};
let node = BraidIrohNode::spawn(config).await?;

// Subscribe to the game topic for neighbor discovery
let mut rx = node.subscribe(GAME_TOPIC, vec![]).await?;

// Handle new peers appearing on the network
while let Some(result) = rx.next().await {
    if let Ok(Event::NeighborUp(peer_id)) = result {
        info!("Peer Discovered: {}", peer_id);
    }
}`}
                />
            </section>

            <div className="mp-divider" />

            {/* STOCKFISH */}
            <section className="mp-section">
                <div className="mp-tech-header">
                    <div className="mp-tech-icon" style={{ background: 'rgba(34, 197, 94, 0.15)', border: '1px solid rgba(34, 197, 94, 0.3)' }}>
                        <Cpu size={28} color="#22c55e" />
                    </div>
                    <div>
                        <div className="mp-tech-label" style={{ color: '#22c55e' }}>AI Engine</div>
                        <h2 className="mp-tech-title">Stockfish</h2>
                    </div>
                </div>

                <p className="mp-section-desc">
                    <strong>Stockfish</strong> is the world's strongest open-source chess engine, running entirely on the local machine as a sidecar process managed by Bevy's async task pool. XFChess communicates with Stockfish over standard I/O using the Universal Chess Interface (UCI) protocol — sending FEN positions and receiving best-move responses. This architecture means the AI never requires a network connection: analysis is instant, private, and available offline. In online matches, Stockfish runs in the background as a post-game analysis tool; in single-player mode, it acts as the opponent at configurable strength levels from beginner to Grandmaster.
                </p>

                <div className="mp-feature-grid">
                    <div className="mp-feature-card">
                        <Cpu size={20} color="#22c55e" />
                        <h4>UCI Protocol</h4>
                        <p>XFChess spawns Stockfish as a child process and communicates via stdin/stdout using UCI commands. The engine receives a FEN string and a depth limit, then returns the best move in algebraic notation.</p>
                    </div>
                    <div className="mp-feature-card">
                        <Zap size={20} color="#22c55e" />
                        <h4>Async Sidecar</h4>
                        <p>Stockfish runs on a dedicated OS thread managed by Bevy's <code>IoTaskPool</code>. Move requests are sent via an async channel, keeping the game loop at 60fps while the engine searches in parallel.</p>
                    </div>
                    <div className="mp-feature-card">
                        <Shield size={20} color="#22c55e" />
                        <h4>Configurable Strength</h4>
                        <p>Search depth, hash table size, and thread count are all configurable at runtime. On mobile, depth is automatically reduced based on battery level to balance strength against power consumption.</p>
                    </div>
                </div>

                <CodeViewer
                    title="src/game/ai/systems.rs"
                    language="Rust (Bevy + UCI)"
                    code={`// Spawn AI task and send FEN to Stockfish sidecar
fn spawn_ai_task_system(
    ai_config: Res<ChessAIResource>,
    current_turn: Res<CurrentTurn>,
    engine: ResMut<ChessEngine>,
    braid_manager: Option<Res<BraidNodeManager>>,
) {
    let fen = engine.current_fen().to_string();
    
    // Trigger Stockfish sidecar via async channel
    if let Some(mgr) = braid_manager {
        if let Some(tx) = &mgr.sidecar_fen_tx {
            tx.send(fen).ok();
        }
    }
}

// execute_move is called when sidecar returns UCI move string`}
                />
            </section>

            <div className="mp-divider" />

            {/* HOW THEY WORK TOGETHER */}
            <section className="mp-section">
                <h2 className="mp-section-title">How They Work Together</h2>
                <p className="mp-section-desc">
                    When two players start an online match, Iroh establishes a direct QUIC tunnel between their devices. Braid runs over this tunnel, treating the Iroh connection as its transport — each player's Braid node publishes move patches to the shared game resource, and the opponent's node receives them in real-time. Stockfish runs independently on each machine, available for post-game analysis or as a hint engine. On-chain settlement via Solana happens asynchronously: once the game concludes, the final move log and result are committed to the XFChess program on Solana L1, creating a permanent, tamper-proof record of the match — independent of whether the Iroh connection is still alive.
                </p>

                <div className="mp-flow">
                    <div className="mp-flow-step">
                        <div className="mp-flow-num">1</div>
                        <div className="mp-flow-content">
                            <h4>Iroh Neighbor Discovery</h4>
                            <p>Clients join the broadcast topic. Iroh Gossip handles the heavy lifting of peer discovery and NAT hole-punching for direct connections.</p>
                        </div>
                    </div>
                    <div className="mp-flow-step">
                        <div className="mp-flow-num">2</div>
                        <div className="mp-flow-content">
                            <h4>Matchmaking Handshake</h4>
                            <p>Peers exchange <code>GameInvite</code> messages. Once accepted, a <code>GameStart</code> packet syncs the initial FEN and player assignments.</p>
                        </div>
                    </div>
                    <div className="mp-flow-step">
                        <div className="mp-flow-num">3</div>
                        <div className="mp-flow-content">
                            <h4>Real-Time Sync</h4>
                            <p>Moves are broadcast as <code>NetworkMessage::Move</code> variants. The Braid node ensures all peers receive the same versioned state update.</p>
                        </div>
                    </div>
                    <div className="mp-flow-step">
                        <div className="mp-flow-num">4</div>
                        <div className="mp-flow-content">
                            <h4>Background Analysis</h4>
                            <p>Stockfish sidecar runs locally to provide instant move verification and hints, completely decoupled from the network transport.</p>
                        </div>
                    </div>
                </div>
            </section>
        </motion.div>
    );
};

export default Multiplayer;
