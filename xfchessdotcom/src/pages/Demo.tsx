import { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { Clapperboard } from 'lucide-react';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';

// Drop the video file at xfchessdotcom/public/videos/demo.mp4 — this page
// detects it and swaps the placeholder for a real player automatically, no
// code changes needed.
const DEMO_VIDEO_SRC = '/videos/demo.mp4';
const DEMO_POSTER_SRC = '/videos/demo-poster.jpg';

export function Demo() {
    const [videoReady, setVideoReady] = useState(false);

    useEffect(() => {
        let cancelled = false;
        fetch(DEMO_VIDEO_SRC, { method: 'HEAD' })
            .then((res) => { if (!cancelled) setVideoReady(res.ok); })
            .catch(() => { if (!cancelled) setVideoReady(false); });
        return () => { cancelled = true; };
    }, []);

    return (
        <motion.main
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="section"
            style={{ minHeight: '100vh', paddingTop: '140px' }}
        >
            <SeoHead meta={PAGE_METADATA.demo} />
            <div className="section-label">WATCH</div>
            <h2 style={{ fontSize: '3rem' }}>Demo<span className="accent">.</span></h2>
            <p style={{ maxWidth: '700px', fontSize: '1.2rem', marginBottom: '48px' }}>
                See XFChess in action — ranked matches, wagered PvP, and tournament play, all on-chain.
            </p>

            <div
                style={{
                    position: 'relative',
                    width: '100%',
                    aspectRatio: '16 / 9',
                    background: 'var(--glass)',
                    border: '1px solid var(--border)',
                    borderRadius: '16px',
                    overflow: 'hidden',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                }}
            >
                {videoReady ? (
                    <video
                        src={DEMO_VIDEO_SRC}
                        poster={DEMO_POSTER_SRC}
                        controls
                        playsInline
                        preload="metadata"
                        style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                    />
                ) : (
                    <div style={{ textAlign: 'center', color: 'var(--text-dim)' }}>
                        <Clapperboard size={48} color="var(--primary)" style={{ marginBottom: '16px' }} />
                        <p style={{ fontSize: '1rem', fontWeight: 600 }}>Demo video coming soon</p>
                    </div>
                )}
            </div>
        </motion.main>
    );
}

export default Demo;
