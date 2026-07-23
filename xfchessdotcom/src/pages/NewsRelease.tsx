import { Calendar, User, Clock, Share2, ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';

export default function NewsRelease() {
    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <SeoHead meta={PAGE_METADATA.newsRelease} />
            <div style={{ maxWidth: '900px', margin: '0 auto' }}>
                {/* Back navigation */}
                <Link to="/blog" style={{ display: 'inline-flex', alignItems: 'center', gap: '8px', color: 'var(--text-dim)', marginBottom: '32px', fontSize: '0.9rem', fontWeight: 600 }}>
                    <ArrowLeft size={16} /> Back to News
                </Link>

                {/* Article Header */}
                <div style={{ marginBottom: '48px' }}>
                    <div style={{ display: 'flex', gap: '20px', alignItems: 'center', marginBottom: '24px', fontSize: '0.9rem', color: 'var(--text-dim)' }}>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '6px', background: 'rgba(255, 255, 255, 0.05)', padding: '6px 12px', borderRadius: '20px' }}>
                            <Calendar size={14} /> April 15, 2026
                        </span>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '6px', background: 'rgba(255, 255, 255, 0.05)', padding: '6px 12px', borderRadius: '20px' }}>
                            <User size={14} /> XFChess Team
                        </span>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '6px', background: 'rgba(255, 255, 255, 0.05)', padding: '6px 12px', borderRadius: '20px' }}>
                            <Clock size={14} /> 8 min read
                        </span>
                    </div>

                    <h1 style={{ fontSize: '3.5rem', fontWeight: 900, lineHeight: 1.1, marginBottom: '24px', letterSpacing: '-0.02em' }}>
                        XFChess Mainnet Launch: The Future of <span className="accent">Decentralized Chess</span> is Here
                    </h1>

                    <p style={{ fontSize: '1.3rem', color: 'var(--text-dim)', lineHeight: 1.6, maxWidth: '700px' }}>
                        After months of rigorous testing and development, we're proud to announce the official mainnet launch of XFChess. Experience competitive chess wagering on Solana with instant settlements and immutable game records.
                    </p>
                </div>

                {/* Featured Image */}
                <div style={{ 
                    width: '100%', 
                    height: '400px', 
                    borderRadius: '16px',
                    marginBottom: '48px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    border: '1px solid var(--border)',
                    position: 'relative',
                    overflow: 'hidden'
                }}>
                    <div style={{ position: 'relative', zIndex: 1, textAlign: 'center' }}>
                        <span style={{ fontSize: '4rem', fontWeight: 900, color: 'var(--primary)' }}>XF</span>
                        <span style={{ fontSize: '4rem', fontWeight: 900, color: '#fff' }}>Chess</span>
                    </div>
                </div>

                {/* Article Content */}
                <article style={{ fontSize: '1.1rem', lineHeight: 1.8, color: 'var(--text)' }}>
                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>A New Era for Competitive Chess</h2>
                        <p style={{ marginBottom: '20px' }}>
                            Today marks a pivotal moment in the history of competitive chess. We're thrilled to announce that XFChess is now live on Solana mainnet, bringing together the timeless strategy of chess with the cutting-edge technology of blockchain. This isn't just another chess platform—it's a complete reimagining of how competitive gaming can work in a decentralized world.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            Our vision has always been clear: create a platform where skill matters, where every move is recorded immutably, and where players can compete with real stakes in a transparent, fraud-proof environment. With today's launch, that vision becomes reality.
                        </p>
                    </section>

                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>What Makes XFChess Different</h2>
                        <p style={{ marginBottom: '20px' }}>
                            Unlike traditional online chess platforms, XFChess leverages the power of Solana's high-performance blockchain to deliver features that were previously impossible:
                        </p>
                        <ul style={{ marginBottom: '20px', paddingLeft: '24px', lineHeight: 2 }}>
                            <li><strong>Instant Settlements:</strong> No more waiting days for tournament winnings. Smart contracts execute payouts immediately upon game completion.</li>
                            <li><strong>Immutable Game Records:</strong> Every move, every game, every tournament result is permanently recorded on-chain, creating an unalterable history of competitive play.</li>
                            <li><strong>True Skill-Based Wagering:</strong> Our Glicko-2 rating system ensures fair matchups based on actual skill, not just win streaks or arbitrary points.</li>
                            <li><strong>Zero Trust Required:</strong> The escrow system eliminates counterparty risk—your wager is secure from the moment you accept a match until the final checkmate.</li>
                        </ul>
                    </section>

                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>The Technology Behind XFChess</h2>
                        <p style={{ marginBottom: '20px' }}>
                            Building a real-time competitive game on a blockchain presents unique challenges. Our engineering team has spent countless hours optimizing every aspect of the platform:
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            At the core of XFChess is our custom Solana program, written in Anchor, that handles game state, escrow management, and tournament logic. We've implemented a sophisticated session key system that allows for rapid-fire gameplay without requiring wallet approvals for every single move.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            For the frontend, we've built a native application using Bevy and Rust, delivering WebGPU-powered 3D graphics that rival traditional gaming engines. The result is a smooth, responsive experience that doesn't compromise on visual fidelity.
                        </p>
                    </section>

                    {/* Content Image */}
                    <div style={{ 
                        width: '100%', 
                        height: '300px', 
                        borderRadius: '12px',
                        marginBottom: '48px',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        border: '1px solid var(--border)',
                        position: 'relative',
                        overflow: 'hidden'
                    }}>
                        <div style={{ position: 'relative', zIndex: 1, color: 'var(--text-dim)', fontSize: '1.2rem' }}>
                            [Screenshot: XFChess 3D Board with Real-time Move Validation]
                        </div>
                    </div>

                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>Tournaments and Competitive Play</h2>
                        <p style={{ marginBottom: '20px' }}>
                            Competitive chess is at the heart of XFChess. Our tournament system supports a variety of formats, from quick blitz matches to extended classical games. Players can organize their own tournaments or participate in official XFChess events with prize pools funded by the community.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            The bracket system is fully on-chain, meaning tournament progress is transparent and verifiable by anyone. No more disputes about results or suspicious withdrawals—everything is recorded immutably on Solana.
                        </p>
                    </section>

                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>Security and Fair Play</h2>
                        <p style={{ marginBottom: '20px' }}>
                            We take security seriously. XFChess implements multiple layers of protection:
                        </p>
                        <ul style={{ marginBottom: '20px', paddingLeft: '24px', lineHeight: 2 }}>
                            <li><strong>Anti-Cheat Measures:</strong> Our engine validates every move against official chess rules, preventing illegal moves and ensuring fair play.</li>
                            <li><strong>Secure Escrow:</strong> Wagers are locked in smart contracts until game completion, eliminating the possibility of one party backing out.</li>
                            <li><strong>Identity Verification:</strong> Optional KYC integration for players who want to participate in higher-stakes matches with verified identities.</li>
                            <li><strong>Regular Audits:</strong> Our smart contracts undergo regular security audits to identify and address potential vulnerabilities.</li>
                        </ul>
                    </section>

                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>What's Next for XFChess</h2>
                        <p style={{ marginBottom: '20px' }}>
                            The mainnet launch is just the beginning. We have an ambitious roadmap ahead:
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            <strong>Q2 2026:</strong> Mobile app release with full feature parity, allowing players to compete on the go. Integration with additional Solana wallets for broader accessibility.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            <strong>Q3 2026:</strong> Advanced analytics dashboard for players to track their performance over time, identify weaknesses, and improve their game. AI-powered move suggestions for training purposes.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            <strong>Q4 2026:</strong> Team tournaments and clan warfare modes. Players can form teams, compete in clan battles, and climb team leaderboards together.
                        </p>
                    </section>

                    <section style={{ marginBottom: '48px' }}>
                        <h2 style={{ fontSize: '2rem', fontWeight: 800, marginBottom: '24px', marginTop: '0' }}>Join the Revolution</h2>
                        <p style={{ marginBottom: '20px' }}>
                            Whether you're a grandmaster looking for serious competition or a casual player wanting to test your skills, XFChess has something for you. Download the client, connect your wallet, and start playing today.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            Follow our development blog for regular updates and follow us on Twitter for the latest news and announcements.
                        </p>
                        <p style={{ marginBottom: '20px' }}>
                            The future of competitive chess is decentralized. The future is XFChess.
                        </p>
                    </section>
                </article>

                {/* Share Section */}
                <div style={{ 
                    borderTop: '1px solid var(--border)', 
                    borderBottom: '1px solid var(--border)', 
                    padding: '32px 0', 
                    marginBottom: '48px',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    flexWrap: 'wrap',
                    gap: '20px'
                }}>
                    <div>
                        <span style={{ fontSize: '0.9rem', color: 'var(--text-dim)', marginRight: '12px' }}>Share this article:</span>
                        <button style={{ background: 'none', border: 'none', color: 'var(--text)', cursor: 'pointer', padding: '8px' }}>
                            <Share2 size={20} />
                        </button>
                    </div>
                    <div style={{ fontSize: '0.9rem', color: 'var(--text-dim)' }}>
                        Category: <span style={{ color: 'var(--primary)', fontWeight: 600 }}>Announcement</span>
                    </div>
                </div>

                {/* Related Articles */}
                <section>
                    <h3 style={{ fontSize: '1.8rem', fontWeight: 800, marginBottom: '32px' }}>Related Articles</h3>
                    <div style={{ display: 'grid', gap: '24px' }}>
                        <Link to="/blog" style={{ 
                            display: 'flex', 
                            gap: '20px', 
                            padding: '20px',
                            background: 'var(--surface)',
                            borderRadius: '12px',
                            border: '1px solid var(--border)',
                            textDecoration: 'none',
                            color: 'inherit',
                            transition: 'all 0.2s'
                        }}>
                            <div style={{ flex: 1 }}>
                                <div style={{ fontSize: '0.85rem', color: 'var(--text-dim)', marginBottom: '8px' }}>
                                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}><Calendar size={12} /> March 28, 2026</span>
                                </div>
                                <h4 style={{ fontSize: '1.2rem', fontWeight: 700, marginBottom: '8px' }}>Glicko-2 Elo Rating On-Chain</h4>
                                <p style={{ fontSize: '0.95rem', color: 'var(--text-dim)', marginBottom: 0 }}>Our new ranking algorithm is now fully immutable on Solana.</p>
                            </div>
                        </Link>

                        <Link to="/blog" style={{ 
                            display: 'flex', 
                            gap: '20px', 
                            padding: '20px',
                            background: 'var(--surface)',
                            borderRadius: '12px',
                            border: '1px solid var(--border)',
                            textDecoration: 'none',
                            color: 'inherit',
                            transition: 'all 0.2s'
                        }}>
                            <div style={{ flex: 1 }}>
                                <div style={{ fontSize: '0.85rem', color: 'var(--text-dim)', marginBottom: '8px' }}>
                                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}><Calendar size={12} /> April 3, 2026</span>
                                </div>
                                <h4 style={{ fontSize: '1.2rem', fontWeight: 700, marginBottom: '8px' }}>Mainnet Migration & Identity Vaulting</h4>
                                <p style={{ fontSize: '0.95rem', color: 'var(--text-dim)', marginBottom: 0 }}>We are officially migrating to mainnet with CARF-compliant identity vaulting.</p>
                            </div>
                        </Link>
                    </div>
                </section>
            </div>
        </main>
    );
}
