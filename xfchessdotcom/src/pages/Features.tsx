import { Link } from 'react-router-dom';
import { Cpu, Swords, Coins, Trophy, Puzzle, Clapperboard, Users } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';

type Feature = {
    icon: LucideIcon;
    color: string;
    title: string;
    desc: string;
    to?: string;
    cta?: string;
};

const features: Feature[] = [
    {
        icon: Cpu,
        color: 'var(--primary)',
        title: 'Play Against Computer',
        desc: 'Sharpen your skills against the built-in engine. Pick a difficulty from a gentle sparring partner to a ruthless grandmaster and train at your own pace, anytime.',
        to: '/computer',
        cta: 'Play the computer',
    },
    {
        icon: Swords,
        color: 'var(--accent)',
        title: 'Player vs Player',
        desc: 'Challenge players around the world in real-time matches. Peer-to-peer networking keeps games fast and direct, so every move lands the moment you make it.',
        to: '/play',
        cta: 'Find a match',
    },
    {
        icon: Coins,
        color: 'var(--primary)',
        title: 'Wagered Play',
        desc: 'Put your money where your mind is. Set clear stakes, play winner-takes-all matches, and settle instantly to your wallet — secure, transparent, and on-chain.',
        to: '/play',
        cta: 'Start a wager',
    },
    {
        icon: Trophy,
        color: 'var(--accent)',
        title: 'Tournaments',
        desc: 'Enter structured tournaments with guaranteed prize pools. Battle through brackets, climb the leaderboard, and claim your share of the winnings.',
        to: '/tournaments',
        cta: 'Browse tournaments',
    },
    {
        icon: Puzzle,
        color: 'var(--primary)',
        title: 'Puzzles',
        desc: 'Train your tactical eye with a growing library of puzzles. Spot the combinations, drill patterns, and turn recognition into instinct over the board.',
    },
    {
        icon: Clapperboard,
        color: '#fff',
        title: 'Cinematic PGN Replay',
        desc: 'Relive any game as a cinematic replay. Load a PGN and watch the match unfold move by move in a light or heavy in-game GUI — perfect for study or spectating.',
    },
    {
        icon: Users,
        color: '#fff',
        title: 'Friends',
        desc: 'Build your circle. Add friends, see who is online, and challenge them directly to a game whenever you are both ready to play.',
    },
];

export function Features() {
    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <SeoHead meta={PAGE_METADATA.features} />
            <div className="section-label">GAME</div>
            <h2 style={{ fontSize: '3rem' }}>Features<span className="accent">.</span></h2>
            <p style={{ maxWidth: '700px', fontSize: '1.2rem', marginBottom: '48px' }}>
                Everything XFChess brings to the board — from solo training to high-stakes competition.
            </p>

            <div
                className="features-grid"
                style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: '32px', marginTop: '48px' }}
            >
                {features.map(({ icon: Icon, color, title, desc, to, cta }) => (
                    <div
                        key={title}
                        className="feature-card"
                        style={{ padding: '40px', background: 'var(--glass)', border: '1px solid var(--border)', borderRadius: '16px', transition: 'all 0.3s ease', display: 'flex', flexDirection: 'column' }}
                    >
                        <div style={{ marginBottom: '20px' }}><Icon color={color} size={40} /></div>
                        <h3 style={{ fontSize: '1.5rem', marginBottom: '16px' }}>{title}</h3>
                        <p style={{ color: 'var(--text-dim)', lineHeight: 1.6, flex: 1 }}>{desc}</p>
                        {to && cta && (
                            <Link to={to} style={{ color: 'var(--primary)', fontWeight: 700, marginTop: '20px' }}>
                                {cta} →
                            </Link>
                        )}
                    </div>
                ))}
            </div>
        </main>
    );
}

export default Features;
