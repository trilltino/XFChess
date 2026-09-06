import highFidelityChess from '../../assets/high-fidelity-chess.png';
import wageredPvpSpectator from '../../assets/wagered-pvp-spectator.png';
import tournamentsBoard from '../../assets/tournaments-board.png';
import boardSilhouette from '../../assets/board-silhouette.png';
import xfchessTitleLogo from '../../assets/xfchess-title-logo.png';
import { SeoHead } from '../../components/SeoHead';
import { VideoGameSchema } from '../../components/StructuredData';
import { PAGE_METADATA } from '../../lib/seo/metadata';

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
                        <h2 className="feature-title">Stake Your Rating</h2>
                        <p className="feature-desc">
                            Play casual games, enter high-stakes brackets, or back your rating in competitive 1v1s.
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
                        <h2 className="feature-title">2D or 3D</h2>
                        <p className="feature-desc">
                            Toggle seamlessly between a classic 2D board and an immersive 3D view mid-match.
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
                        <h2 className="feature-title">Open Source</h2>
                        <p className="feature-desc">
                            Every line of code is public under AGPL-3.0. Build from source, or host your own instance.
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
