import { motion } from 'framer-motion';
import { ArrowLeft, Play } from 'lucide-react';
import { Link } from 'react-router-dom';
import './Demo.css';

const Demo = () => {
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
            <div className="demo-video-wrap">
                <div className="demo-video-inner">
                    <div className="demo-play-overlay">
                        <div className="demo-play-btn">
                            <Play size={32} fill="#fff" color="#fff" />
                        </div>
                        <p className="demo-coming-soon">Demo video coming soon</p>
                    </div>
                    <div className="demo-board-preview">
                        <div className="demo-board-grid">
                            {Array.from({ length: 64 }).map((_, i) => {
                                const row = Math.floor(i / 8);
                                const col = i % 8;
                                const isLight = (row + col) % 2 === 0;
                                return (
                                    <div
                                        key={i}
                                        className={`demo-square ${isLight ? 'light' : 'dark'}`}
                                    />
                                );
                            })}
                        </div>
                        <div className="demo-board-overlay">
                            <span className="demo-xf-badge"><span style={{ color: '#e63946' }}>XF</span>Chess</span>
                        </div>
                    </div>
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

        </motion.div>
    );
};

export default Demo;
