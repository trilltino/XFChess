import { useWallet } from '@solana/wallet-adapter-react';
import { Link } from 'react-router-dom';

export function Friends() {
    const { connected } = useWallet();

    return (
        <div className="page-container" style={{ paddingTop: '120px', minHeight: '100vh', display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
            <div style={{ maxWidth: 600, width: '100%', padding: '0 24px' }}>
                <h1 style={{ fontSize: '2rem', fontWeight: 800, color: 'var(--text)', marginBottom: '12px' }}>
                    Friends
                </h1>
                <p style={{ color: 'var(--text-dim)', fontSize: '14px', marginBottom: '32px' }}>
                    Challenge friends, track rivalries, and play together.
                </p>
                {!connected ? (
                    <div style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '12px', padding: '32px', textAlign: 'center' }}>
                        <p style={{ color: 'var(--text-dim)', marginBottom: '16px' }}>Connect your wallet to see your friends.</p>
                    </div>
                ) : (
                    <div style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '12px', padding: '32px', textAlign: 'center' }}>
                        <p style={{ color: 'var(--text-dim)', marginBottom: '16px' }}>Friends list coming soon.</p>
                        <Link to="/players" className="btn-primary" style={{ display: 'inline-block', padding: '10px 24px', borderRadius: '8px', textDecoration: 'none', fontSize: '14px', fontWeight: 700 }}>
                            Browse Players
                        </Link>
                    </div>
                )}
            </div>
        </div>
    );
}
