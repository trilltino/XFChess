import { Play, Image, Camera } from 'lucide-react';

export function Media() {
    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <div className="section-label">COMMUNITY ASSETS</div>
            <h2 style={{ fontSize: '3rem' }}>XF Media Center<span className="accent">.</span></h2>
            <p style={{ maxWidth: '700px', fontSize: '1.2rem', marginBottom: '48px' }}>
                Relive the most intense games and browse our high-fidelity visual documentation.
            </p>

            <div className="media-grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '24px', marginTop: '48px' }}>
                {[1, 2, 3, 4, 5, 6].map((i) => (
                    <div key={i} className="media-card" style={{ position: 'relative', height: '240px', background: 'var(--glass)', border: '1px solid var(--border)', borderRadius: '12px', overflow: 'hidden', cursor: 'pointer', transition: 'all 0.3s ease' }}>
                        <div style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                            <Play fill="#fff" size={32} />
                        </div>
                        <div style={{ position: 'absolute', bottom: '20px', left: '20px', fontSize: '0.8rem', color: '#fff', fontWeight: 700 }}>
                            Game Replay #{i}
                        </div>
                    </div>
                ))}
            </div>

            <div style={{ marginTop: '60px', display: 'flex', gap: '20px', flexWrap: 'wrap' }}>
                <button className="btn btn-secondary" style={{ width: 'auto', padding: '0 24px' }}><Image size={20} /> View Screenshots</button>
                <button className="btn btn-secondary" style={{ width: 'auto', padding: '0 24px' }}><Camera size={20} /> Community Photos</button>
            </div>
        </main>
    );
}
