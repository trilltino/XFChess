import highFidelityChess from '../assets/high-fidelity-chess.png';
import wageredPvpSpectator from '../assets/wagered-pvp-spectator.png';
import tournamentsBoard from '../assets/tournaments-board.png';
import boardSilhouette from '../assets/board-silhouette.png';
import xfchessTitleLogo from '../assets/xfchess-title-logo.png';
import { SeoHead } from '../components/SeoHead';
import { VideoGameSchema } from '../components/StructuredData';
import { PAGE_METADATA } from '../lib/seo/metadata';

export function Home() {

    return (
        <main className="home-root">
            <SeoHead meta={PAGE_METADATA.home} />
            <VideoGameSchema />

            {/* HERO */}
            <section className="home-hero">
                <div className="home-hero-glow" />
                <img src={boardSilhouette} alt="" aria-hidden="true" className="home-hero-board-silhouette" />
                <h1 className="home-hero-title">
                    <img src={xfchessTitleLogo} alt="XFChess — Competitive Chess Server" className="home-hero-title-img" />
                </h1>
            </section>

            {/* 01 — WAGERED PVP */}
            <section className="fullscreen-section">
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1' }}>
                        <h2 className="feature-title">Winner Takes All</h2>
                        <p className="feature-desc">
                            Stake SOL and face off head-to-head. Funds lock in on-chain escrow the moment
                            both players accept, and settle instantly when the king falls.
                        </p>
                    </div>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-end' }}>
                        <img src={wageredPvpSpectator} alt="Live wagered chess match" className="home-feature-image" />
                    </div>
                </div>
            </section>

            {/* 02 — HIGH FIDELITY */}
            <section className="fullscreen-section">
                <div className="section-content" style={{ display: 'flex', flexDirection: 'row-reverse', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1' }}>
                        <h2 className="feature-title">A Real 3D Board</h2>
                        <p className="feature-desc">
                            Rendered in Bevy with full 3D pieces and board. The same engine that powers
                            the desktop client drives every match, move by move.
                        </p>
                    </div>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-start' }}>
                        <img src={highFidelityChess} alt="High-fidelity 3D chess board" className="home-feature-image" />
                    </div>
                </div>
            </section>

            {/* 03 — TOURNAMENTS */}
            <section className="fullscreen-section">
                <div className="section-content" style={{ display: 'flex', alignItems: 'center', gap: '60px' }}>
                    <div style={{ flex: '1' }}>
                        <h2 className="feature-title">Compete for Pools</h2>
                        <p className="feature-desc">
                            Join structured brackets and Swiss events with guaranteed prize pools held in
                            escrow. Climb the ladder and earn your reputation.
                        </p>
                    </div>
                    <div style={{ flex: '1', display: 'flex', justifyContent: 'flex-end' }}>
                        <img src={tournamentsBoard} alt="Tournament chess board" className="home-feature-image" />
                    </div>
                </div>
            </section>

        </main>
    );
}
