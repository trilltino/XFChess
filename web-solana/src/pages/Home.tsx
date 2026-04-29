import { Link } from 'react-router-dom';
import chessboardSpace from '../assets/chessboard-space.png';
import chessWageringUI from '../assets/chess-wagering-ui.png';
import highFidelityChess from '../assets/high-fidelity-chess.png';
import xfchessLogo from '../assets/xfchess-logo.png';
import learnTheGame from '../assets/learn-the-game.png';

export function Home() {

    return (
        <main className="home-root">
            {/* HERO: STRATEGY / ACTION CHESS */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_medieval_battle_hero_1775215627149.png')`,
                paddingTop: '160px'
            }}>
                <div className="section-overlay"></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1' }}>
                        <h1 className="feature-title">Competitive Chess Server</h1>

                        <p className="feature-desc">
                            Challenge players worldwide, compete in tournaments, and play chess anywhere, anytime.
                            Experience wagered chess, computer opponents, and tournament game modes.
                            Earn money through your chess hustle. Challenge players around the world at any time and compete for cash prizes either through direct games or our Grand Tournaments.
                        </p>
                    </div>
                    <div style={{ flex: '0 0 320px' }}>
                        <h2 className="feature-title" style={{ marginBottom: '24px' }}>News</h2>
                        <div style={{ 
                            background: 'var(--surface)', 
                            borderRadius: '12px', 
                            overflow: 'hidden', 
                            border: '1px solid var(--border)'
                        }}>
                            <div style={{ height: '160px', background: 'linear-gradient(135deg, #1a3d2e 0%, #0f2a1f 100%)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                                <img src={xfchessLogo} alt="XFChess Logo" style={{ width: '100%', height: '100%', objectFit: 'cover' }} />
                            </div>
                            <div style={{ padding: '20px' }}>
                                <h3 style={{ fontSize: '1.1rem', marginBottom: '8px', color: 'var(--text)' }}>XFChess released</h3>
                                <Link to="/news/release" style={{ color: 'var(--primary)', fontSize: '0.9rem', fontWeight: 600 }}>read more</Link>
                            </div>
                        </div>
                    </div>
                </div>
            </section>

            {/* SECTION: SANDBOX WAR TABLE */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_war_table_strategy_1775215651444.png')`,
                backgroundPosition: 'top center'
            }}>
                <div className="section-overlay" style={{ background: 'linear-gradient(to left, rgba(8, 26, 20, 0.95), rgba(8, 26, 20, 0.2))' }}></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-start' }}>
                        <img src={chessboardSpace} alt="Chessboard in space" style={{ display: 'block', maxWidth: '300px', width: '100%', height: 'auto', aspectRatio: '16/10', objectFit: 'cover', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }} />
                    </div>
                    <div style={{ flex: '1', textAlign: 'right' }}>
                        <h2 className="feature-title">3D and 2D GUI</h2>

                        <p className="feature-desc">
                            Every move can be rendered in 2D or 3D with instant switch. Custom board sets allow every game to tell a story. Master the squares with whichever way you prefer. A Light or Heavy in-game GUI to tell spectators and players all about a game's history or to cut the noise and focus on the tension of the game.
                        </p>
                    </div>
                </div>
            </section>

            {/* SECTION: HIGH FIDELITY */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_tech_engineering_1775215698765.png')`
            }}>
                <div className="section-overlay"></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1' }}>
                        <h2 className="feature-title">Free Decentralised Chess Server</h2>

                        <p className="feature-desc">
                            XFChess is a free, open-source decentralized game server. By using Solana coupled with P2P networking protocols, we create a chess server that is owned by your machine. XFChess will always be free to access and open source. The goal is to spread the powerful game of chess to every individual and allow them, no matter their background, to learn and then hustle with their skill.
                        </p>
                    </div>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-end' }}>
                        <img src={highFidelityChess} alt="High-fidelity chess gameplay" style={{ display: 'block', maxWidth: '300px', width: '100%', height: 'auto', aspectRatio: '16/10', objectFit: 'cover', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }} />
                    </div>
                </div>
            </section>

            {/* SECTION: ECONOMY */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_economy_gold_pieces_1775215677989.png')`
            }}>
                <div className="section-overlay"></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-start' }}>
                        <img src={chessWageringUI} alt="Chess wagering interface" style={{ display: 'block', maxWidth: '300px', width: '100%', height: 'auto', objectFit: 'contain', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }} />
                    </div>
                    <div style={{ flex: '1', textAlign: 'right' }}>
                        <h2 className="feature-title">Own your Hustle</h2>

                        <p className="feature-desc">
                            Challenge opponents with clear stakes. Winner takes all in these PvP encounters. Put your money where your mind is - Wager on your chess skills in secure, transparent matches. Financial transactions settle in real-time to your wallet allowing you to access your gains. Weekly Tournaments attract larger prize pools.
                        </p>
                    </div>
                </div>
            </section>

            {/* SECTION: LEARN THE GAME */}
            <section className="fullscreen-section" style={{
                backgroundImage: `url('/C:/Users/isich/.gemini/antigravity/brain/0a9ca3a9-366a-46e5-8c2d-238d39256994/chess_medieval_battle_hero_1775215627149.png')`
            }}>
                <div className="section-overlay"></div>
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1' }}>
                        <h2 className="feature-title">Learn the Game</h2>

                        <p className="feature-desc">
                            Learn from the historic game modes represented in a 2D or 3D interface, understand the people who changed the game for centuries, common pitfalls and how to teach chess to your peers.
                        </p>
                    </div>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-end' }}>
                        <img src={learnTheGame} alt="Learn the game" style={{ display: 'block', maxWidth: '400px', width: '100%', height: 'auto', aspectRatio: '16/10', objectFit: 'cover', borderRadius: '12px', boxShadow: '0 8px 32px rgba(0, 0, 0, 0.3)' }} />
                    </div>
                </div>
            </section>

        </main>
    );
}
