import { useWallet } from '@solana/wallet-adapter-react';

export function Puzzles() {
    const { connected } = useWallet();

    return (
        <div className="page-container" style={{ paddingTop: '120px', minHeight: '100vh', display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
            <div style={{ maxWidth: 600, width: '100%', padding: '0 24px' }}>
                <h1 style={{ fontSize: '2rem', fontWeight: 800, color: 'var(--text)', marginBottom: '12px' }}>
                    Puzzles
                </h1>
                <p style={{ color: 'var(--text-dim)', fontSize: '14px', marginBottom: '32px' }}>
                    Sharpen your tactics with daily chess puzzles.
                </p>
                {!connected ? (
                    <div style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '12px', padding: '32px', textAlign: 'center' }}>
                        <p style={{ color: 'var(--text-dim)' }}>Connect your wallet to access puzzles.</p>
                    </div>
                ) : (
                    <div style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '12px', padding: '32px', textAlign: 'center' }}>
                        <p style={{ color: 'var(--text-dim)' }}>Daily puzzles coming soon. Check back shortly!</p>
                    </div>
                )}
            </div>
        </div>
    );
}
