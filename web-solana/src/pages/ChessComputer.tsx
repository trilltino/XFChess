import chessComputerIcon from '../assets/chess-computer-icon.webp';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';

export function ChessComputer() {
    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '100px' }}>
            <SeoHead meta={PAGE_METADATA.computer} />
            <div className="section-label">GAME MODES</div>
            <h2 style={{ fontSize: '2.5rem', marginBottom: '8px' }}>Chess Computer<span className="accent">.</span></h2>

            {/* Content Section */}
            <div style={{ maxWidth: '1000px', margin: '0 auto', marginTop: '40px' }}>
                {/* Hero with Image */}
                <div style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '40px',
                    marginBottom: '60px',
                    background: 'rgba(255, 255, 255, 0.03)',
                    border: '1px solid rgba(255, 255, 255, 0.08)',
                    borderRadius: '16px',
                    padding: '40px'
                }}>
                    <div style={{ flex: '1' }}>
                        <h3 style={{ fontSize: '1.8rem', fontWeight: 800, marginBottom: '16px', color: '#fff' }}>
                            Powered by Stockfish
                        </h3>
                        <p style={{ color: 'var(--text-dim)', lineHeight: 1.7, fontSize: '1.05rem' }}>
                            Challenge our advanced chess AI powered by Stockfish, one of the strongest chess engines in the world. Test your skills against adjustable difficulty levels from beginner to grandmaster.
                        </p>
                    </div>
                    <div style={{ flex: '0 0 350px' }}>
                        <img
                            src={chessComputerIcon}
                            alt="Chess Computer Icon"
                            style={{ width: '100%', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }}
                        />
                    </div>
                </div>

                {/* How It Works */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        How It Works
                    </h3>
                    <div style={{ background: 'rgba(255, 255, 255, 0.03)', borderRadius: '12px', padding: '32px', border: '1px solid rgba(255, 255, 255, 0.08)' }}>
                        <div style={{ display: 'grid', gap: '24px' }}>
                            <div style={{ display: 'flex', gap: '16px' }}>
                                <div style={{
                                    width: '40px',
                                    height: '40px',
                                    borderRadius: '50%',
                                    background: 'var(--primary)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'center',
                                    fontWeight: 800,
                                    fontSize: '1.1rem',
                                    flexShrink: 0
                                }}>1</div>
                                <div>
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Stockfish Integration</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        XFChess uses the Stockfish chess engine, running as a separate process that analyzes positions and calculates the best moves.
                                    </p>
                                </div>
                            </div>
                            <div style={{ display: 'flex', gap: '16px' }}>
                                <div style={{
                                    width: '40px',
                                    height: '40px',
                                    borderRadius: '50%',
                                    background: 'var(--primary)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'center',
                                    fontWeight: 800,
                                    fontSize: '1.1rem',
                                    flexShrink: 0
                                }}>2</div>
                                <div>
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>UCI Protocol</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        The game communicates with Stockfish using the Universal Chess Interface (UCI) protocol, sending FEN positions and receiving move recommendations.
                                    </p>
                                </div>
                            </div>
                            <div style={{ display: 'flex', gap: '16px' }}>
                                <div style={{
                                    width: '40px',
                                    height: '40px',
                                    borderRadius: '50%',
                                    background: 'var(--primary)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'center',
                                    fontWeight: 800,
                                    fontSize: '1.1rem',
                                    flexShrink: 0
                                }}>3</div>
                                <div>
                                    <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Adjustable Difficulty</h4>
                                    <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                        Choose from multiple difficulty levels by adjusting search depth and thinking time, making the AI suitable for players of all skill levels.
                                    </p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Code Snippet */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        Stockfish Process Spawning (Rust)
                    </h3>
                    <div style={{
                        background: '#1a1a2e',
                        borderRadius: '12px',
                        padding: '24px',
                        border: '1px solid rgba(255, 255, 255, 0.1)',
                        fontFamily: 'monospace',
                        fontSize: '0.9rem',
                        overflow: 'auto'
                    }}>
                        <pre style={{ margin: 0, color: '#e0e0e0' }}>
<span style={{ color: '#89ddff' }}>let mut child</span> <span style={{ color: '#ff79c6' }}>=</span> <span style={{ color: '#82aaff' }}>Command::new</span>(stockfish_path)<br />
    .<span style={{ color: '#82aaff' }}>stdin</span>(<span style={{ color: '#82aaff' }}>Stdio::piped</span>())<br />
    .<span style={{ color: '#82aaff' }}>stdout</span>(<span style={{ color: '#82aaff' }}>Stdio::piped</span>())<br />
    .<span style={{ color: '#82aaff' }}>stderr</span>(<span style={{ color: '#82aaff' }}>Stdio::null</span>())<br />
    .<span style={{ color: '#82aaff' }}>spawn</span>()<br />
    .<span style={{ color: '#82aaff' }}>map_err</span>(<span style={{ color: '#f07178' }}>|e|</span> <span style={{ color: '#82aaff' }}>format!</span>(<span style={{ color: '#c3e88d' }}>""Failed to spawn Stockfish: {}"</span>, e))<span style={{ color: '#ff79c6' }}>?</span>;
                        </pre>
                    </div>
                </div>

                {/* Features */}
                <div style={{ marginBottom: '48px' }}>
                    <h3 style={{ fontSize: '1.5rem', fontWeight: 800, marginBottom: '24px', color: '#fff' }}>
                        Features
                    </h3>
                    <div style={{ display: 'grid', gap: '16px' }}>
                        <div style={{
                            background: 'rgba(255, 255, 255, 0.03)',
                            borderRadius: '12px',
                            padding: '24px',
                            border: '1px solid rgba(255, 255, 255, 0.08)'
                        }}>
                            <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Multiple Difficulty Levels</h4>
                            <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                Choose from beginner to grandmaster-level AI opponents with adjustable search depth.
                            </p>
                        </div>
                        <div style={{
                            background: 'rgba(255, 255, 255, 0.03)',
                            borderRadius: '12px',
                            padding: '24px',
                            border: '1px solid rgba(255, 255, 255, 0.08)'
                        }}>
                            <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Real-Time Analysis</h4>
                            <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                Get real-time move suggestions and game analysis to improve your chess understanding.
                            </p>
                        </div>
                        <div style={{
                            background: 'rgba(255, 255, 255, 0.03)',
                            borderRadius: '12px',
                            padding: '24px',
                            border: '1px solid rgba(255, 255, 255, 0.08)'
                        }}>
                            <h4 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: '8px', color: '#fff' }}>Puzzle Mode</h4>
                            <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, margin: 0 }}>
                                Practice tactical positions and improve your pattern recognition with puzzle challenges.
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    );
}
