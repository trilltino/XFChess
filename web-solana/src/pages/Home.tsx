import { Link, useNavigate } from 'react-router-dom';
import { Globe, GitBranch } from 'lucide-react';
import { useWallet } from '@solana/wallet-adapter-react';

export function Home() {
    const { connected } = useWallet();
    const navigate = useNavigate();

    const handleMatchClick = () => {
        if (connected) {
            navigate('/profile'); // Direct to profile which has "Launch Game" and stats
        } else {
            navigate('/auth/login');
        }
    };

    return (
        <main className="home-root">
            {/* HERO: STRATEGY / ACTION CHESS */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_medieval_battle_hero_1775215627149.png')`
            }}>
                <div className="section-overlay"></div>
                <div className="section-content">
                    <h1 className="feature-title">XFChess Play <span className="accent">Anywhere.</span></h1>

                    <p className="feature-desc">
                        Master the ancient game of strategy on the blockchain. Challenge players worldwide, 
                        compete in tournaments, and play chess anywhere, anytime.
                        Experience fair play with transparent on-chain verification and secure wagering.
                        Join a global community of chess enthusiasts and climb the competitive leaderboards.
                        Whether you're a grandmaster or a beginner, there's always a match waiting for you.
                    </p>
                    <div className="home-hero-actions">
                        <Link to={connected ? "/profile" : "/auth/register"} className="btn btn-primary" style={{ width: 'auto', padding: '0 40px' }}>
                            {connected ? "View Your Profile" : "Create Your Chess Identity"}
                        </Link>
                    </div>
                </div>
            </section>

            {/* SECTION: SANDBOX WAR TABLE */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_war_table_strategy_1775215651444.png')`,
                backgroundPosition: 'top center'
            }}>
                <div className="section-overlay" style={{ background: 'linear-gradient(to left, rgba(8, 26, 20, 0.95), rgba(8, 26, 20, 0.2))' }}></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '40px' }}>
                    <div style={{ flex: '1', padding: '20px', background: 'rgba(255, 255, 255, 0.05)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.1)', backdropFilter: 'blur(10px)' }}>
                        <img src="/chessboard-space.png" alt="Chessboard in space" style={{ display: 'block', maxWidth: '600px', width: '100%', height: 'auto', aspectRatio: '16/10', objectFit: 'cover', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }} />
                    </div>
                    <div style={{ flex: '1', textAlign: 'right' }}>
                        <h2 className="feature-title">Strategic <span className="accent">Chess.</span></h2>

                        <p className="feature-desc">
                            Experience the timeless game of chess with modern blockchain technology. 
                            Every move matters, every game tells a story. Master the 64 squares and 
                            prove your tactical prowess against players worldwide.
                            Engage in timeless strategy with cutting-edge innovation where each 
                            opening, tactic, and endgame is permanently recorded on-chain. Compete 
                            in ranked matches, climb the global leaderboards, and earn recognition 
                            as a true chess master in the decentralized arena.
                        </p>
                        <div className="home-hero-actions right">
                            <Link to="/how-to-play" className="btn btn-secondary" style={{ width: 'auto', padding: '0 32px' }}>
                                Learn the Rules
                            </Link>
                        </div>
                    </div>
                </div>
            </section>

            {/* SECTION: ECONOMY */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_economy_gold_pieces_1775215677989.png')`
            }}>
                <div className="section-overlay"></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '40px' }}>
                    <div style={{ flex: '1' }}>
                        <h2 className="feature-title">Chess <span className="accent">Wagering.</span></h2>

                        <p className="feature-desc">
                            Challenge opponents in head-to-head matches with clear stakes.
                            Winner takes all in these strategic PvP encounters where your 
                            chess skills directly translate to real rewards.
                            Put your money where your mind is - wager SOL on your chess skills 
                            in secure, transparent matches. Every game is verified on-chain, ensuring 
                            fair play and instant payouts to the victor.
                        </p>
                        
                        <div style={{ display: 'flex', gap: '12px', marginTop: '24px' }}>
                            {[
                                { label: '$2' },
                                { label: '$5' },
                                { label: '$10' }
                            ].map(tier => (
                                <button 
                                    key={tier.label}
                                    onClick={() => handleMatchClick()}
                                    className="btn"
                                    style={{ 
                                        flex: '1', 
                                        flexDirection: 'column', 
                                        height: 'auto', 
                                        padding: '16px 0',
                                        background: 'rgba(173, 92, 47, 0.1)',
                                        border: '1px solid rgba(173, 92, 47, 0.4)',
                                        borderRadius: '12px'
                                    }}
                                >
                                    <div style={{ fontSize: '1.4rem', fontWeight: 900, color: 'var(--primary)' }}>{tier.label}</div>
                                </button>
                            ))}
                        </div>
                    </div>
                    <div style={{ flex: '1', padding: '20px', background: 'rgba(255, 255, 255, 0.05)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.1)', backdropFilter: 'blur(10px)' }}>
                        <img src="/chess-wagering-ui.png" alt="Chess wagering interface" style={{ display: 'block', maxWidth: '600px', width: '100%', height: 'auto', aspectRatio: '16/10', objectFit: 'cover', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }} />
                    </div>
                </div>
            </section>

            {/* NEWS AND UPDATES */}
            <section className="section">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '40px' }}>
                    <h2 className="home-section-title" style={{ margin: 0 }}>News and Updates</h2>
                    <Link to="/blog" style={{ color: 'var(--primary)', fontSize: '0.9rem', fontWeight: 600 }}>See All</Link>
                </div>
                
                <div style={{ 
                    display: 'flex', 
                    gap: '24px',
                    overflowX: 'auto',
                    paddingBottom: '16px',
                    scrollSnapType: 'x mandatory',
                    WebkitOverflowScrolling: 'touch'
                }}>
                    {/* News Card */}
                    <div style={{ 
                        flex: '0 0 320px', 
                        background: 'var(--surface)', 
                        borderRadius: '12px', 
                        overflow: 'hidden', 
                        border: '1px solid var(--border)',
                        scrollSnapAlign: 'start'
                    }}>
                        <div style={{ height: '160px', background: 'linear-gradient(135deg, #1a3d2e 0%, #0f2a1f 100%)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                            <span style={{ fontSize: '2rem', fontWeight: 800, color: 'var(--primary)' }}>XFChess</span>
                        </div>
                        <div style={{ padding: '20px' }}>
                            <h3 style={{ fontSize: '1.1rem', marginBottom: '8px', color: 'var(--text)' }}>XFChess released!</h3>
                            <p style={{ fontSize: '0.75rem', color: 'var(--text-dim)', marginBottom: '12px' }}>1 Mar 2025</p>
                            <Link to="/news/release" style={{ color: 'var(--primary)', fontSize: '0.9rem', fontWeight: 600 }}>read more</Link>
                        </div>
                    </div>
                </div>
            </section>

            {/* FOOTER: MODDING & MULTIPLAYER */}
            <section className="section">
                <div style={{ textAlign: 'center', marginBottom: '80px' }}>
                    <Globe color="var(--primary)" size={48} style={{ margin: '0 auto 24px' }} />
                    <h2 className="home-section-title centered">Global Chess Arena.</h2>
                    <p style={{ color: 'var(--text-dim)', fontSize: '1.2rem', maxWidth: '600px', margin: '0 auto' }}>
                        Compete in real-time matches against chess enthusiasts worldwide. Join ranked tournaments, 
                        climb the leaderboards, and prove your strategic mastery in the ultimate blockchain chess ecosystem.
                    </p>
                </div>

                <div className="divider"></div>

                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '40px' }}>
                    <div style={{ maxWidth: '500px' }}>
                        <div className="section-label">DEVELOPER ECOSYSTEM</div>
                        <h3 style={{ fontSize: '1.8rem', marginBottom: '16px' }}>Build Your Chess World</h3>
                        <p style={{ color: 'var(--text-dim)' }}>
                            Leverage our open-source chess engine and blockchain infrastructure to create 
                            custom game modes, tournament systems, and innovative chess experiences. 
                            Deploy your own chess applications on the XFChess protocol.
                        </p>
                    </div>
                    <button className="btn btn-secondary" style={{ width: 'auto', padding: '0 40px' }}>
                        <GitBranch size={20} style={{ marginRight: '10px' }} /> Developer Portal
                    </button>
                </div>
            </section>

        </main>
    );
}
