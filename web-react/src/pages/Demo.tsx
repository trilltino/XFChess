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

            <div className="demo-header">
                <div className="demo-label">Live Preview</div>
                <h1>See <span className="demo-accent">XFChess</span> in action.</h1>
            </div>

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

        </motion.div>
    );
};

export default Demo;
