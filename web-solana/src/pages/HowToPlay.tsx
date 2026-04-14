import { Shield } from 'lucide-react';


export function HowToPlay() {
    return (
        <main className="onboarding-page">
            <div className="onboarding-container">
                <section className="onboarding-card">
                    <div className="section-label">GETTING STARTED</div>
                    <h2 className="onboarding-title">How to Play<span className="accent">.</span></h2>
                    <p className="onboarding-subtitle">
                        Enter the future of competitive chess. Follow these three simple steps to start your on-chain journey.
                    </p>

            <div className="steps" style={{ display: 'grid', gap: '40px', marginTop: '48px' }}>
                <div className="step" style={{ display: 'flex', gap: '24px' }}>
                    <div className="step-num" style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: '1.2rem', color: 'var(--primary)', fontWeight: 800 }}>01</div>
                    <div>
                        <h3 style={{ fontSize: '1.5rem', marginBottom: '12px' }}>Connect Your Wallet</h3>
                        <p>Use Phantom or Solflare to secure your account. Your wallet acts as your identity and your vault for competitive wagers.</p>
                    </div>
                </div>

                <div className="step" style={{ display: 'flex', gap: '24px' }}>
                    <div className="step-num" style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: '1.2rem', color: 'var(--primary)', fontWeight: 800 }}>02</div>
                    <div>
                        <h3 style={{ fontSize: '1.5rem', marginBottom: '12px' }}>Set Your Handle</h3>
                        <p>Create a unique username on the Solana blockchain. This will be your permanent identity across the XFChess ecosystem.</p>
                    </div>
                </div>

                <div className="step" style={{ display: 'flex', gap: '24px' }}>
                    <div className="step-num" style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: '1.2rem', color: 'var(--primary)', fontWeight: 800 }}>03</div>
                    <div>
                        <h3 style={{ fontSize: '1.5rem', marginBottom: '12px' }}>Join the Arena</h3>
                        <p>Browse global tournaments or create a custom game. Win games to increase your on-chain Elo rating and earn rewards.</p>
                    </div>
                </div>
            </div>

            <div style={{ marginTop: '80px', padding: '40px', background: 'rgba(255, 255, 255, 0.05)', border: '1px solid rgba(255, 255, 255, 0.08)', borderRadius: '16px', backdropFilter: 'blur(20px)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px', marginBottom: '20px' }}>
                    <Shield color="var(--primary)" size={32} />
                    <h3 style={{ margin: 0, fontSize: '1.4rem', fontWeight: 700 }}>Safe & Secure Play</h3>
                </div>
                <p className="onboarding-section-text">Every move is verified on-chain. We use off-chain anti-cheat oracles coupled with Solana's high-speed consensus to ensure every game is fair and final.</p>
            </div>
                </section>
            </div>
        </main>
    );
}
