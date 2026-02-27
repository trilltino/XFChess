import { motion } from 'framer-motion';
import { ArrowLeft, Play } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';
import './Demo.css';

const Demo = () => {
    const [isPlaying, setIsPlaying] = useState(false);
    const [isPlayingGameDemo, setIsPlayingGameDemo] = useState(false);
    const videoId = 'e_qn9iDrgDM';
    const gameDemoVideoId = ''; // Placeholder - add your game demo video ID here
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

            {/* TRY YOURSELF SECTION */}
            <div className="demo-section" style={{ marginTop: '3rem', padding: '2rem', background: 'rgba(20, 20, 20, 0.5)', borderRadius: '12px', border: '1px solid rgba(255, 255, 255, 0.1)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '1rem', marginBottom: '1.5rem' }}>
                    <div style={{ width: '12px', height: '12px', background: '#22c55e', borderRadius: '50%' }}></div>
                    <h2 style={{ margin: 0, fontSize: '1.5rem' }}>Try Yourself</h2>
                </div>
                <p style={{ color: '#a0a0a0', lineHeight: '1.6' }}>
                    Run <code style={{ background: '#111', padding: '0.2rem 0.4rem', borderRadius: '3px' }}>magicblock_e2e_test.bat</code> to start both player UIs and test the full wager flow on Solana devnet.
                </p>
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
