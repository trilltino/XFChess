import { Link } from 'react-router-dom';
import { SeoHead } from '../../components/SeoHead';
import { PAGE_METADATA } from '../../lib/seo/metadata';

type Feature = {
    title: string;
    desc: string;
    to?: string;
    cta?: string;
};

const FEATURES: Feature[] = [
    {
        title: 'Computer',
        desc: 'Stockfish, from a gentle sparring partner to grandmaster. In the client.',
    },
    {
        title: 'Player vs player',
        desc: 'Peer-to-peer matches. Moves land the moment you make them.',
    },
    {
        title: 'Wagered play',
        desc: 'Winner takes all. Settled on-chain, straight to your wallet. Set the wallet up in the client.',
    },
    {
        title: 'Tournaments',
        desc: 'Swiss brackets with real prize pools.',
        to: '/tournaments',
        cta: 'Browse tournaments',
    },
    { title: 'Puzzles', desc: 'A growing tactics library. Drill patterns into instinct.' },
    { title: 'PGN replay', desc: 'Load a game and watch it back move by move, in 2D or 3D.' },
    { title: 'Friends', desc: 'See who is online and challenge them directly.' },
];

export function Features() {
    return (
        <main className="mp">
            <SeoHead meta={PAGE_METADATA.features} />
            <div className="mp-eyebrow">Game</div>
            <h1 className="mp-title">Features</h1>
            <p className="mp-lede">Solo training through to high-stakes competition.</p>

            <div className="mp-rows">
                {FEATURES.map(({ title, desc, to, cta }) => (
                    <div className="mp-row" key={title}>
                        <h2 className="mp-row-k">{title}</h2>
                        <div>
                            <p className="mp-row-v">{desc}</p>
                            {to && cta && (
                                <Link to={to} className="mp-row-cta">
                                    {cta} →
                                </Link>
                            )}
                        </div>
                    </div>
                ))}
            </div>
        </main>
    );
}

export default Features;
