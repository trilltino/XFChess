import { motion } from 'framer-motion';
import { ArrowLeft, Play } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';
import './Demo.css';

const Demo = () => {
    const [isPlaying, setIsPlaying] = useState(false);
    const [isPlayingGameDemo, setIsPlayingGameDemo] = useState(false);
    const videoId = 'e_qn9iDrgDM';
    const gameDemoVideoId = 'dQw4w9WgXcQ'; // Placeholder - replace with actual video ID
    // Use hqdefault.jpg for better reliability - maxresdefault doesn't exist for all videos
    const thumbnailUrl = `https://img.youtube.com/vi/${videoId}/hqdefault.jpg`;
    const gameDemoThumbnailUrl = `https://img.youtube.com/vi/${gameDemoVideoId}/hqdefault.jpg`;

    return (
        <motion.div
            className="demo-container"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.5 }}
        >
            <Link to="/" className="back-btn demo-back">
                <ArrowLeft size={18} /> Back
            </Link>

            {/* VIDEO / DEMO EMBED */}
            {/* Introduction - outside video container */}
            <div style={{ textAlign: 'center', marginBottom: '1rem' }}>
                <h3 style={{ margin: '0 0 0.5rem 0', fontSize: '1.25rem', color: '#fff' }}>Introduction</h3>
                <p style={{ margin: 0, color: '#a0a0a0', fontStyle: 'italic' }}>
                    tino says hi and introduces XFChess
                </p>
            </div>
            <div className="demo-video-wrap">
                <div className="demo-video-embed" style={{ position: 'absolute', inset: 0, overflow: 'hidden', borderRadius: '12px', background: '#000' }}>
                    {!isPlaying ? (
                        <>
                            <img
                                src={thumbnailUrl}
                                alt="XFChess Introduction Thumbnail"
                                style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', objectFit: 'cover', cursor: 'pointer', backgroundColor: '#1a1a1a' }}
                                onClick={() => setIsPlaying(true)}
                                onError={(e) => {
                                    const target = e.target as HTMLImageElement;
                                    // Prevent infinite loop if even the fallback fails
                                    if (!target.src.includes('/0.jpg')) {
                                        target.src = `https://img.youtube.com/vi/${videoId}/0.jpg`;
                                    }
                                }}
                            />
                            <div
                                onClick={() => setIsPlaying(true)}
                                style={{
                                    position: 'absolute',
                                    top: '50%',
                                    left: '50%',
                                    transform: 'translate(-50%, -50%)',
                                    width: '80px',
                                    height: '80px',
                                    background: 'rgba(255, 0, 0, 0.9)',
                                    borderRadius: '50%',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'center',
                                    cursor: 'pointer',
                                    transition: 'transform 0.2s, background 0.2s',
                                    boxShadow: '0 4px 20px rgba(0, 0, 0, 0.5)'
                                }}
                                onMouseEnter={(e) => {
                                    e.currentTarget.style.transform = 'translate(-50%, -50%) scale(1.1)';
                                    e.currentTarget.style.background = 'rgba(255, 0, 0, 1)';
                                }}
                                onMouseLeave={(e) => {
                                    e.currentTarget.style.transform = 'translate(-50%, -50%) scale(1)';
                                    e.currentTarget.style.background = 'rgba(255, 0, 0, 0.9)';
                                }}
                            >
                                <Play size={32} color="white" fill="white" />
                            </div>
                        </>
                    ) : (
                        <iframe
                            src={`https://www.youtube.com/embed/${videoId}?autoplay=1`}
                            title="XFChess Introduction"
                            style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', border: 'none' }}
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                            allowFullScreen
                        />
                    )}
                </div>
            </div>

            {/* LIVE GAME DEMO - SECOND VIDEO */}
            <div style={{ textAlign: 'center', marginBottom: '1rem', marginTop: '3rem' }}>
                <h3 style={{ margin: '0 0 0.5rem 0', fontSize: '1.25rem', color: '#fff' }}>Live Game Demo</h3>
                <p style={{ margin: '0 0 0.5rem 0', color: '#a0a0a0', fontStyle: 'italic' }}>
                    Watch "fool's mate" in action with live blockchain interactions
                </p>
                <p style={{ margin: '0 0 0.75rem 0', color: '#888', fontSize: '0.9rem', maxWidth: '600px', marginLeft: 'auto', marginRight: 'auto' }}>
                    Fool's mate is the fastest possible checkmate in chess — just two moves by White and a catastrophic response by Black.
                    This demo shows how the game detects checkmate and records the result on-chain in real-time.
                </p>
                <p style={{ margin: 0, color: '#e63946', fontSize: '0.85rem', background: 'rgba(230, 57, 70, 0.1)', padding: '0.5rem 1rem', borderRadius: '4px', display: 'inline-block' }}>
                    ⚠️ The game is fully implemented but is a prototype — full games are still being worked on
                </p>
            </div>
            <div className="demo-video-wrap">
                <div className="demo-video-embed" style={{ position: 'absolute', inset: 0, overflow: 'hidden', borderRadius: '12px', background: '#000' }}>
                    {!isPlayingGameDemo ? (
                        <>
                            <img
                                src={gameDemoThumbnailUrl}
                                alt="Live Game Demo Thumbnail"
                                style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', objectFit: 'cover', cursor: 'pointer', backgroundColor: '#1a1a1a' }}
                                onClick={() => setIsPlayingGameDemo(true)}
                                onError={(e) => {
                                    const target = e.target as HTMLImageElement;
                                    if (!target.src.includes('/0.jpg')) {
                                        target.src = `https://img.youtube.com/vi/${gameDemoVideoId}/0.jpg`;
                                    }
                                }}
                            />
                            <div
                                onClick={() => setIsPlayingGameDemo(true)}
                                style={{
                                    position: 'absolute',
                                    top: '50%',
                                    left: '50%',
                                    transform: 'translate(-50%, -50%)',
                                    width: '80px',
                                    height: '80px',
                                    background: 'rgba(255, 0, 0, 0.9)',
                                    borderRadius: '50%',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'center',
                                    cursor: 'pointer',
                                    transition: 'transform 0.2s, background 0.2s',
                                    boxShadow: '0 4px 20px rgba(0, 0, 0, 0.5)'
                                }}
                                onMouseEnter={(e) => {
                                    e.currentTarget.style.transform = 'translate(-50%, -50%) scale(1.1)';
                                    e.currentTarget.style.background = 'rgba(255, 0, 0, 1)';
                                }}
                                onMouseLeave={(e) => {
                                    e.currentTarget.style.transform = 'translate(-50%, -50%) scale(1)';
                                    e.currentTarget.style.background = 'rgba(255, 0, 0, 0.9)';
                                }}
                            >
                                <Play size={32} color="white" fill="white" />
                            </div>
                        </>
                    ) : (
                        <iframe
                            src={`https://www.youtube.com/embed/${gameDemoVideoId}?autoplay=1`}
                            title="Live Game Demo"
                            style={{ position: 'absolute', top: 0, left: 0, width: '100%', height: '100%', border: 'none' }}
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                            allowFullScreen
                        />
                    )}
                </div>
            </div>

            {/* MULTIPLAYER TESTING INSTRUCTIONIONS */}
            <div className="demo-section" style={{ marginTop: '3rem', padding: '2rem', background: 'rgba(20, 20, 20, 0.5)', borderRadius: '12px', border: '1px solid rgba(255, 255, 255, 0.1)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '1rem', marginBottom: '1.5rem' }}>
                    <div style={{ width: '12px', height: '12px', background: '#22c55e', borderRadius: '50%' }}></div>
                    <h2 style={{ margin: 0, fontSize: '1.5rem' }}>Multiplayer Testing Guide</h2>
                </div>
                <p style={{ color: '#a0a0a0', lineHeight: '1.6', marginBottom: '2rem' }}>
                    Test the full wager flow with two players on Solana devnet. Each player runs a separate browser instance and launches the native game client.
                </p>

                <div style={{ display: 'grid', gap: '1.5rem' }}>
                    {/* Step 1 */}
                    <div style={{ background: 'rgba(0, 0, 0, 0.4)', padding: '1.5rem', borderRadius: '8px', borderLeft: '4px solid #22c55e' }}>
                        <h3 style={{ margin: '0 0 0.75rem 0', fontSize: '1.1rem', color: '#fff' }}>Step 1: Start Both Player UIs</h3>
                        <p style={{ margin: '0 0 1rem 0', color: '#888', fontSize: '0.9rem' }}>Run the E2E test script to start both browser instances:</p>
                        <code style={{ display: 'block', background: '#111', padding: '0.75rem', borderRadius: '4px', fontFamily: 'monospace', fontSize: '0.85rem', color: '#22c55e' }}>
                            magicblock_e2e_test.bat
                        </code>
                        <p style={{ margin: '0.75rem 0 0 0', color: '#666', fontSize: '0.8rem' }}>This opens Player 1 on port 5173 and Player 2 on port 5174</p>
                    </div>

                    {/* Step 2 */}
                    <div style={{ background: 'rgba(0, 0, 0, 0.4)', padding: '1.5rem', borderRadius: '8px', borderLeft: '4px solid #3b82f6' }}>
                        <h3 style={{ margin: '0 0 0.75rem 0', fontSize: '1.1rem', color: '#fff' }}>Step 2: Player 1 Creates Game</h3>
                        <ol style={{ margin: 0, paddingLeft: '1.25rem', color: '#888', fontSize: '0.9rem', lineHeight: '1.6' }}>
                            <li>Open <strong>http://localhost:5173</strong> (Player 1)</li>
                            <li>Connect your Solana wallet (devnet)</li>
                            <li>Click <strong>"Create Wager Game"</strong></li>
                            <li>Set wager amount (e.g., 0.01 SOL)</li>
                            <li>Copy the <strong>Game ID</strong> shown</li>
                        </ol>
                    </div>

                    {/* Step 3 */}
                    <div style={{ background: 'rgba(0, 0, 0, 0.4)', padding: '1.5rem', borderRadius: '8px', borderLeft: '4px solid #8b5cf6' }}>
                        <h3 style={{ margin: '0 0 0.75rem 0', fontSize: '1.1rem', color: '#fff' }}>Step 3: Player 2 Joins Game</h3>
                        <ol style={{ margin: 0, paddingLeft: '1.25rem', color: '#888', fontSize: '0.9rem', lineHeight: '1.6' }}>
                            <li>Open <strong>http://localhost:5174</strong> (Player 2)</li>
                            <li>Connect a different Solana wallet (devnet)</li>
                            <li>Click <strong>"Join Game"</strong></li>
                            <li>Paste the <strong>Game ID</strong> from Player 1</li>
                            <li>Confirm the wager match</li>
                        </ol>
                    </div>

                    {/* Step 4 */}
                    <div style={{ background: 'rgba(0, 0, 0, 0.4)', padding: '1.5rem', borderRadius: '8px', borderLeft: '4px solid #f59e0b' }}>
                        <h3 style={{ margin: '0 0 0.75rem 0', fontSize: '1.1rem', color: '#fff' }}>Step 4: Launch Game Clients</h3>
                        <p style={{ margin: '0 0 1rem 0', color: '#888', fontSize: '0.9rem' }}>Both players click <strong>"Launch Game"</strong> in their browsers:</p>
                        <ul style={{ margin: '0 0 1rem 0', paddingLeft: '1.25rem', color: '#888', fontSize: '0.9rem', lineHeight: '1.6' }}>
                            <li>Downloads <code>xfchess_session_&lt;game_id&gt;.json</code> (unique filename per game)</li>
                            <li>Run the batch file to start the native game:</li>
                        </ul>
                        <code style={{ display: 'block', background: '#111', padding: '0.75rem', borderRadius: '4px', fontFamily: 'monospace', fontSize: '0.85rem', color: '#f59e0b' }}>
                            cd C:\Users\isich\XFChess
                            <br />
                            launch_game_with_session.bat %USERPROFILE%\Downloads\xfchess_session_&lt;game_id&gt;.json
                        </code>
                    </div>

                    {/* Step 5 */}
                    <div style={{ background: 'rgba(0, 0, 0, 0.4)', padding: '1.5rem', borderRadius: '8px', borderLeft: '4px solid #e63946' }}>
                        <h3 style={{ margin: '0 0 0.75rem 0', fontSize: '1.1rem', color: '#fff' }}>Step 5: Play & Verify</h3>
                        <ul style={{ margin: 0, paddingLeft: '1.25rem', color: '#888', fontSize: '0.9rem', lineHeight: '1.6' }}>
                            <li>Game launches with players assigned White/Black</li>
                            <li>Each move is recorded on Solana devnet</li>
                            <li>Game state synced via on-chain Game PDA</li>
                            <li>Winner receives payout on <strong>finalizeGame</strong></li>
                        </ul>
                    </div>
                </div>

                <div style={{ marginTop: '1.5rem', padding: '1rem', background: 'rgba(230, 57, 70, 0.1)', borderRadius: '8px', border: '1px solid rgba(230, 57, 70, 0.3)' }}>
                    <p style={{ margin: 0, color: '#e63946', fontSize: '0.85rem', fontWeight: 600 }}>⚠️ Requirements:</p>
                    <ul style={{ margin: '0.5rem 0 0 0', paddingLeft: '1.25rem', color: '#888', fontSize: '0.8rem' }}>
                        <li>Both players need devnet SOL (get from <a href="https://faucet.solana.com/" target="_blank" rel="noopener noreferrer" style={{ color: '#3b82f6' }}>faucet.solana.com</a>)</li>
                        <li>Game client must be built: <code style={{ background: '#111', padding: '0.2rem 0.4rem', borderRadius: '3px' }}>cargo build --release</code></li>
                        <li>Program ID: <code style={{ background: '#111', padding: '0.2rem 0.4rem', borderRadius: '3px' }}>3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP</code></li>
                    </ul>
                </div>
            </div>

            {/* TRANSACTION EVIDENCE SECTION */}
            <div className="demo-section" style={{ marginTop: '4rem', padding: '2rem', background: 'rgba(20, 20, 20, 0.5)', borderRadius: '12px', border: '1px solid rgba(255, 255, 255, 0.1)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '1rem', marginBottom: '1.5rem' }}>
                    <div style={{ width: '12px', height: '12px', background: '#e63946', borderRadius: '50%' }}></div>
                    <h2 style={{ margin: 0, fontSize: '1.5rem' }}>Transaction Evidence</h2>
                </div>
                <p style={{ color: '#a0a0a0', lineHeight: '1.6', marginBottom: '2rem' }}>
                    Every game on XFChess leaves an immutable trail on the Solana blockchain.
                    Explore real transaction data, verify game outcomes, and audit the fairness
                    of every match through our transparent on-chain records.
                </p>
                <div style={{
                    background: 'rgba(0, 0, 0, 0.5)',
                    borderRadius: '8px',
                    padding: '2rem',
                    border: '1px dashed rgba(230, 57, 70, 0.3)',
                    textAlign: 'center'
                }}>
                    <p style={{ color: '#666', fontStyle: 'italic', margin: 0 }}>
                        Transaction explorer coming soon...
                    </p>
                    <p style={{ color: '#444', fontSize: '0.85rem', marginTop: '0.5rem' }}>
                        Real-time game verification and on-chain proof of play
                    </p>
                </div>
            </div>

        </motion.div >
    );
};

export default Demo;
