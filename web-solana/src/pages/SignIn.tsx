import { useState, useEffect, useCallback, useRef } from 'react';
import bs58 from 'bs58';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { WalletReadyState } from '@solana/wallet-adapter-base';
import { Loader2, Shield, ShieldCheck, Trophy, Zap, ChevronRight, RefreshCw, Cpu, X } from 'lucide-react';
import { getAnchorProgram, fetchPlayerProfile, createPlayerProfile } from '../lib/anchor_client';
import { useNavigate } from 'react-router-dom';

// ─── Live SOL price hook ─────────────────────────────────────────────────────
interface SolPrice { usd: number; gbp: number; updatedAt: Date | null; loading: boolean; error: boolean; }

function useSolPrice(): SolPrice & { refresh: () => void } {
    const [price, setPrice] = useState<SolPrice>({ usd: 0, gbp: 0, updatedAt: null, loading: true, error: false });
    const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const fetch_ = useCallback(async () => {
        setPrice(p => ({ ...p, loading: true, error: false }));
        try {
            const r = await fetch(
                'https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd,gbp',
                { cache: 'no-store' }
            );
            if (!r.ok) throw new Error('bad status');
            const d = await r.json();
            setPrice({ usd: d.solana.usd, gbp: d.solana.gbp, updatedAt: new Date(), loading: false, error: false });
        } catch {
            setPrice(p => ({ ...p, loading: false, error: true }));
        }
    }, []);

    useEffect(() => {
        fetch_();
        timerRef.current = setInterval(fetch_, 60_000);
        return () => { if (timerRef.current) clearInterval(timerRef.current); };
    }, [fetch_]);

    return { ...price, refresh: fetch_ };
}

// ─── API helpers ────────────────────────────────────────────────────────────
const API = import.meta.env.VITE_BACKEND_URL || ''; // Relative for proxying via Tauri
async function apiPost<T>(path: string, body: unknown): Promise<T> {
    const r = await fetch(`${API}${path}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
    });
    if (!r.ok) {
        const t = await r.text();
        throw new Error(t || `${r.status}`);
    }
    const ct = r.headers.get('content-type') ?? '';
    return ct.includes('application/json') ? r.json() : (null as T);
}

// ─── Flow steps ─────────────────────────────────────────────────────────────
type FlowStep = 'identity' | 'credentials' | 'wallet_login' | 'connect_wallet' | 'profile';

interface AuthResult { token: string; username: string }

// ─── Shared style helpers ────────────────────────────────────────────────────
const card: React.CSSProperties = {
    maxWidth: 460,
    width: '92%',
    margin: '0 auto',
    padding: '36px 40px',
    background: 'rgba(8,26,20,0.85)',
    border: '1px solid rgba(255,255,255,0.08)',
    borderRadius: 20,
    backdropFilter: 'blur(24px)',
    WebkitBackdropFilter: 'blur(24px)',
    boxShadow: '0 20px 60px rgba(0,0,0,0.6), 0 0 60px rgba(173,92,47,0.08)',
};

const input: React.CSSProperties = {
    width: '100%',
    padding: '13px 16px',
    borderRadius: 10,
    border: '1px solid rgba(255,255,255,0.1)',
    background: 'rgba(255,255,255,0.04)',
    color: '#fff',
    fontSize: 15,
    outline: 'none',
    fontFamily: 'inherit',
    transition: 'border-color 0.2s',
};

const primaryBtn: React.CSSProperties = {
    width: '100%',
    padding: '14px 0',
    borderRadius: 10,
    border: 'none',
    background: 'linear-gradient(135deg, #ad5c2f, #8c4a26)',
    color: '#fff',
    fontSize: 15,
    fontWeight: 700,
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 8,
    boxShadow: '0 4px 20px rgba(173,92,47,0.35)',
    transition: 'all 0.2s',
    letterSpacing: '0.02em',
};

const walletBtn: React.CSSProperties = {
    width: '100%',
    padding: '16px 20px',
    borderRadius: 12,
    border: '1px solid rgba(255,255,255,0.1)',
    background: 'rgba(255,255,255,0.03)',
    color: '#fff',
    fontSize: 15,
    fontWeight: 700,
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    gap: 14,
    textAlign: 'left' as const,
    transition: 'all 0.2s',
    letterSpacing: '0.01em',
};

// ─── Step dots ───────────────────────────────────────────────────────────────
function StepDots({ current }: { current: 0 | 1 | 2 }) {
    return (
        <div style={{ display: 'flex', gap: 6, justifyContent: 'center', marginBottom: 28 }}>
            {[0, 1, 2].map(i => (
                <div key={i} style={{
                    width: i === current ? 20 : 6, height: 6, borderRadius: 3,
                    background: i <= current ? '#ad5c2f' : 'rgba(255,255,255,0.12)',
                    transition: 'all 0.3s',
                }} />
            ))}
        </div>
    );
}

// ─── Error box ───────────────────────────────────────────────────────────────
function ErrBox({ msg }: { msg: string }) {
    return (
        <div style={{
            padding: '10px 14px', borderRadius: 10,
            background: 'rgba(173,92,47,0.1)', border: '1px solid rgba(173,92,47,0.4)',
            color: '#f4bb44', fontSize: 13, marginBottom: 16
        }}>⚠ {msg}</div>
    );
}

// ─── Identity picker ────────────────────────────────────────────────────────
function IdentityStep({
    onWallet, onEmail,
}: {
    onWallet: () => void;
    onEmail: (mode: 'login' | 'register') => void;
}) {
    return (
        <div style={{ ...card, maxWidth: 440 }}>
            <div style={{ textAlign: 'center', marginBottom: 32 }}>
                <div style={{ fontSize: 36, marginBottom: 6 }}>♛</div>
                <h2 style={{ fontSize: 22, fontWeight: 900, margin: '0 0 4px', letterSpacing: '-0.03em' }}>
                    <span style={{ color: '#ad5c2f' }}>XF</span>Chess
                </h2>
                <p style={{ color: 'rgba(255,255,255,0.4)', fontSize: 13, margin: '8px 0 0', letterSpacing: '0.04em', textTransform: 'uppercase' }}>
                    Choose your identity path
                </p>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                <button
                    style={identityBtn}
                    onClick={onWallet}
                    onMouseEnter={e => applyHover(e, true)}
                    onMouseLeave={e => applyHover(e, false)}
                >
                    <div style={identityIcon}><Shield size={20} color="#ad5c2f" /></div>
                    <div style={{ flex: 1, textAlign: 'left' as const }}>
                        <div style={{ fontWeight: 800, fontSize: 14 }}>Login with Wallet</div>
                        <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.4)', marginTop: 2 }}>Instant sign-in for existing users</div>
                    </div>
                    <ChevronRight size={16} style={{ color: 'rgba(255,255,255,0.25)' }} />
                </button>

                <button
                    style={identityBtn}
                    onClick={() => onEmail('register')}
                    onMouseEnter={e => applyHover(e, true)}
                    onMouseLeave={e => applyHover(e, false)}
                >
                    <div style={identityIcon}><Zap size={20} color="#ad5c2f" /></div>
                    <div style={{ flex: 1, textAlign: 'left' as const }}>
                        <div style={{ fontWeight: 800, fontSize: 14 }}>Create Account</div>
                        <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.4)', marginTop: 2 }}>Use Email + Password</div>
                    </div>
                    <ChevronRight size={16} style={{ color: 'rgba(255,255,255,0.25)' }} />
                </button>

            </div>

        </div>
    );
}

function applyHover(e: React.MouseEvent<HTMLButtonElement>, on: boolean) {
    const el = e.currentTarget as HTMLButtonElement;
    el.style.borderColor = on ? '#ad5c2f' : 'rgba(255,255,255,0.08)';
    el.style.background = on ? 'rgba(173,92,47,0.08)' : 'rgba(255,255,255,0.025)';
}

const identityBtn: React.CSSProperties = {
    width: '100%',
    padding: '16px 18px',
    borderRadius: 12,
    border: '1px solid rgba(255,255,255,0.08)',
    background: 'rgba(255,255,255,0.025)',
    color: '#fff',
    cursor: 'pointer',
    display: 'flex',
    alignItems: 'center',
    gap: 14,
    transition: 'all 0.18s',
};

const identityIcon: React.CSSProperties = {
    width: 40,
    height: 40,
    borderRadius: 10,
    background: 'rgba(173,92,47,0.12)',
    border: '1px solid rgba(173,92,47,0.25)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    flexShrink: 0,
};

// ─── Step 1: Email + Password ────────────────────────────────────────────────
function CredentialsStep({
    mode, onAuth
}: {
    mode: 'login' | 'register';
    onAuth: (r: AuthResult) => void;
}) {
    const [m, setM] = useState(mode);
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [loading, setLoading] = useState(false);
    const [err, setErr] = useState<string | null>(null);

    const submit = async () => {
        setErr(null);
        if (!email || !password) { setErr('Email and password are required'); return; }
        setLoading(true);
        try {
            const body = { email, password, username: email.split('@')[0] };
            const path = m === 'login' ? '/api/auth/login-email' : '/api/auth/register-email';
            const res = await apiPost<AuthResult>(path, body);
            localStorage.setItem('xfchess_token', res.token);
            localStorage.setItem('xfchess_username', res.username);
            localStorage.setItem('xfchess_email', email);
            onAuth(res);
        } catch (e: any) {
            const msg = e.message || '';
            setErr(msg.includes('404') || msg.includes('Invalid') ? 'Invalid email or password' : msg);
        } finally { setLoading(false); }
    };

    return (
        <div style={card}>
            <StepDots current={0} />

            <div style={{ textAlign: 'center', marginBottom: 28 }}>
                <div style={{ fontSize: 36, marginBottom: 4 }}>♛</div>
                <h2 style={{ fontSize: 22, fontWeight: 900, margin: 0, letterSpacing: '-0.03em' }}>
                    <span style={{ color: '#ad5c2f' }}>XF</span>Chess
                </h2>
                <p style={{ color: 'rgba(255,255,255,0.5)', fontSize: 13, marginTop: 6 }}>
                    {m === 'login'
                        ? 'Sign in — then connect your wallet to play'
                        : 'Create your on-chain identity'}
                </p>
            </div>

            {err && <ErrBox msg={err} />}



            <div style={{ marginBottom: 14 }}>
                <label style={{ display: 'block', fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,0.4)', marginBottom: 6, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                    Email
                </label>
                <input
                    style={input}
                    type="email"
                    value={email}
                    onChange={e => setEmail(e.target.value)}
                    placeholder="you@example.com"
                    onKeyDown={e => e.key === 'Enter' && submit()}
                    onFocus={e => (e.target.style.borderColor = '#ad5c2f')}
                    onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.1)')}
                />
            </div>

            <div style={{ marginBottom: 24 }}>
                <label style={{ display: 'block', fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,0.4)', marginBottom: 6, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                    Password
                </label>
                <input
                    style={input}
                    type="password"
                    value={password}
                    onChange={e => setPassword(e.target.value)}
                    placeholder="••••••••"
                    onKeyDown={e => e.key === 'Enter' && submit()}
                    onFocus={e => (e.target.style.borderColor = '#ad5c2f')}
                    onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.1)')}
                />
            </div>

            <button style={{ ...primaryBtn, opacity: loading ? 0.7 : 1 }} onClick={submit} disabled={loading}>
                {loading ? <Loader2 size={16} className="spinner" /> : <Zap size={16} />}
                {m === 'login' ? 'Sign In' : 'Create Account'}
            </button>

            <p style={{ textAlign: 'center', marginTop: 20, fontSize: 13, color: 'rgba(255,255,255,0.4)' }}>
                {m === 'login' ? "No account? " : "Already have one? "}
                <button
                    onClick={() => { setM(m === 'login' ? 'register' : 'login'); setErr(null); }}
                    style={{ background: 'none', border: 'none', color: '#ad5c2f', fontWeight: 700, cursor: 'pointer', fontSize: 13 }}
                >
                    {m === 'login' ? 'Create one' : 'Sign in'}
                </button>
            </p>
        </div>
    );
}

// ─── Step 2: Connect Wallet ───────────────────────────────────────────────────
function ConnectWalletStep({ username, onConnected }: { username: string; onConnected: () => void }) {
    const { select, wallets, connected, connecting, publicKey } = useWallet();
    const [err, setErr] = useState<string | null>(null);
    const isTauri = !!(window as any).__TAURI__;

    useEffect(() => {
        if (connected) {
            // Link wallet to account in backend
            const email = localStorage.getItem('xfchess_email');
            const wallet = publicKey?.toBase58();
            if (email && wallet) {
                apiPost('/api/auth/link-wallet', { email, wallet }).catch(console.error);
            }
            onConnected();
        }
    }, [connected, publicKey, onConnected]);

    const handleSelect = (name: string) => {
        setErr(null);
        try {
            select(name as any);
        } catch (e: any) {
            setErr(e.message || 'Connection failed');
        }
    };

    // Only show Phantom and Solflare
    const displayed = wallets.filter(w =>
        w.adapter.name === 'Phantom' || w.adapter.name === 'Solflare'
    );

    return (
        <div style={card}>
            <StepDots current={1} />

            <div style={{ textAlign: 'center', marginBottom: 28 }}>
                <div style={{ fontSize: 36, marginBottom: 4 }}>♛</div>
                <h2 style={{ fontSize: 22, fontWeight: 900, margin: 0 }}>
                    Connect Your Wallet
                </h2>
                <p style={{ color: 'rgba(255,255,255,0.5)', fontSize: 13, marginTop: 6 }}>
                    Hey <strong style={{ color: '#fff' }}>{username}</strong> — connect your Solana wallet to retrieve your player profile and play wagered games.
                </p>
            </div>

            {err && <ErrBox msg={err} />}

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                {displayed.map(w => {
                    const notInstalled =
                        w.readyState === WalletReadyState.NotDetected ||
                        w.readyState === WalletReadyState.Unsupported;
                    if (notInstalled) {
                        const isExtensionWallet = w.adapter.name === 'Phantom' || w.adapter.name === 'Solflare';
                        
                        if (isTauri && isExtensionWallet) {
                            return (
                                <button
                                    key={w.adapter.name}
                                    style={{ ...walletBtn, opacity: 0.9, borderColor: 'rgba(173,92,47,0.3)' }}
                                    onClick={() => {
                                        const url = window.location.href.split('?')[0]; // Open clean URL in browser
                                        if ((window as any).__TAURI__) {
                                            (window as any).__TAURI__.invoke('open_external_browser', { url });
                                        } else {
                                            window.open(url, '_blank');
                                        }
                                    }}
                                    onMouseEnter={e => {
                                        (e.currentTarget as HTMLButtonElement).style.borderColor = '#ad5c2f';
                                        (e.currentTarget as HTMLButtonElement).style.background = 'rgba(173,92,47,0.12)';
                                    }}
                                    onMouseLeave={e => {
                                        (e.currentTarget as HTMLButtonElement).style.borderColor = 'rgba(173,92,47,0.3)';
                                        (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.03)';
                                    }}
                                >
                                    <img src={w.adapter.icon} alt={w.adapter.name} width={28} height={28} style={{ borderRadius: 6 }} />
                                    <span style={{ flex: 1 }}>{w.adapter.name}</span>
                                    <span style={{ fontSize: 11, color: '#ad5c2f', fontWeight: 700 }}>Open in Browser →</span>
                                </button>
                            );
                        }

                        return (
                            <a
                                key={w.adapter.name}
                                href={w.adapter.url}
                                target='_blank'
                                rel='noreferrer'
                                style={{
                                    ...walletBtn, textDecoration: 'none', opacity: 0.75,
                                    border: '1px dashed rgba(255,255,255,0.15)',
                                }}
                                onMouseEnter={e => {
                                    (e.currentTarget as HTMLAnchorElement).style.borderColor = '#ad5c2f';
                                    (e.currentTarget as HTMLAnchorElement).style.opacity = '1';
                                }}
                                onMouseLeave={e => {
                                    (e.currentTarget as HTMLAnchorElement).style.borderColor = 'rgba(255,255,255,0.15)';
                                    (e.currentTarget as HTMLAnchorElement).style.opacity = '0.75';
                                }}
                            >
                                <img src={w.adapter.icon} alt={w.adapter.name} width={28} height={28} style={{ borderRadius: 6, opacity: 0.6 }} />
                                <span style={{ flex: 1, color: 'rgba(255,255,255,0.45)' }}>{w.adapter.name} — not installed</span>
                                <span style={{ fontSize: 11, color: '#ad5c2f', fontWeight: 700 }}>Install →</span>
                            </a>
                        );
                    }
                    return (
                        <button
                            key={w.adapter.name}
                            style={walletBtn}
                            disabled={connecting}
                            onClick={() => handleSelect(w.adapter.name)}
                            onMouseEnter={e => {
                                (e.currentTarget as HTMLButtonElement).style.borderColor = '#ad5c2f';
                                (e.currentTarget as HTMLButtonElement).style.background = 'rgba(173,92,47,0.12)';
                            }}
                            onMouseLeave={e => {
                                (e.currentTarget as HTMLButtonElement).style.borderColor = 'rgba(255,255,255,0.1)';
                                (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.03)';
                            }}
                        >
                            <img src={w.adapter.icon} alt={w.adapter.name} width={28} height={28} style={{ borderRadius: 6 }} />
                            <span style={{ flex: 1 }}>Sign with {w.adapter.name}</span>
                            {connecting
                                ? <Loader2 size={16} style={{ animation: 'spin 0.7s linear infinite' }} />
                                : <ChevronRight size={16} style={{ color: 'rgba(255,255,255,0.3)' }} />
                            }
                        </button>
                    );
                })}

                {connected && publicKey && !localStorage.getItem('xfchess_use_hot') && (
                    <button
                        style={{ ...walletBtn, background: 'rgba(100, 50, 200, 0.1)', border: '1px solid rgba(100, 50, 200, 0.3)' }}
                        onClick={() => {
                            const moonpayUrl = `https://buy.moonpay.com?apiKey=pk_test_123&currencyCode=sol&walletAddress=${publicKey.toBase58()}`;
                            window.open(moonpayUrl, '_blank');
                        }}
                    >
                        <span style={{ fontSize: 20 }}>💳</span>
                        <span style={{ flex: 1, fontWeight: 700, fontSize: 13, color: '#9b59b6' }}>Buy SOL with MoonPay</span>
                        <ChevronRight size={16} style={{ color: 'rgba(155,89,182,0.5)' }} />
                    </button>
                )}

                <div style={{ margin: '12px 0', height: 1, background: 'rgba(255,255,255,0.06)' }} />

            </div>

            <p style={{ fontSize: 11, color: 'rgba(255,255,255,0.2)', textAlign: 'center', marginTop: 20 }}>
                Wallet keys never leave your browser.
            </p>
        </div>
    );
}

// ─── Wager conversion table ───────────────────────────────────────────────────
function WagerTable({ profile }: { profile: any }) {
    const { usd, gbp, updatedAt, loading: priceLoading, error: priceError, refresh } = useSolPrice();

    const LAMPORTS = 1_000_000_000;
    const totalWagered = Number(profile.data.totalWagered ?? profile.data.total_wagered ?? 0) / LAMPORTS;
    const totalWon     = Number(profile.data.totalWon    ?? profile.data.total_won    ?? 0) / LAMPORTS;
    const netPnl       = totalWon - totalWagered;

    const fmt = (sol: number, rate: number, sym: string) =>
        `${sym}${(sol * rate).toLocaleString('en-GB', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

    const rows = [
        { label: 'Total Wagered', sol: totalWagered, pnl: false },
        { label: 'Total Won',     sol: totalWon,     pnl: false },
        { label: 'Net P&L',       sol: netPnl,       pnl: true  },
    ];

    const thStyle: React.CSSProperties = {
        padding: '8px 10px', textAlign: 'left' as const, fontSize: 10,
        fontWeight: 700, color: 'rgba(255,255,255,0.35)',
        letterSpacing: '0.08em', textTransform: 'uppercase' as const,
        borderBottom: '1px solid rgba(255,255,255,0.06)',
    };
    const tdStyle: React.CSSProperties = {
        padding: '11px 10px', fontSize: 13, fontWeight: 700,
        borderBottom: '1px solid rgba(255,255,255,0.04)',
        verticalAlign: 'middle' as const,
    };

    const timeStr = updatedAt
        ? updatedAt.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
        : '\u2014';

    return (
        <div style={{
            marginBottom: 20,
            background: 'rgba(255,255,255,0.02)',
            border: '1px solid rgba(255,255,255,0.06)',
            borderRadius: 12, overflow: 'hidden',
        }}>
            <div style={{
                display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                padding: '12px 14px', borderBottom: '1px solid rgba(255,255,255,0.06)',
            }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span style={{ fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,0.4)', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                        Wager Activity
                    </span>

                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    {priceLoading && <Loader2 size={12} style={{ color: '#ad5c2f', animation: 'spin 0.8s linear infinite' }} />}
                    {!priceLoading && usd > 0 && (
                        <span style={{ fontSize: 11, color: 'rgba(255,255,255,0.3)', fontFamily: 'monospace' }}>
                            1 SOL = ${usd.toFixed(2)} \u00b7 \u00a3{gbp.toFixed(2)} \u00b7 {timeStr}
                        </span>
                    )}
                    {priceError && <span style={{ fontSize: 11, color: '#f87171' }}>Price unavailable</span>}
                    <button
                        onClick={refresh}
                        title="Refresh price"
                        style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'rgba(255,255,255,0.3)', padding: 2, display: 'flex' }}
                    >
                        <RefreshCw size={12} />
                    </button>
                </div>
            </div>

            {totalWagered === 0 ? (
                <p style={{ margin: 0, padding: '20px', fontSize: 13, color: 'rgba(255,255,255,0.25)', textAlign: 'center' }}>
                    No wagered games played yet.
                </p>
            ) : (
                <table style={{ width: '100%', borderCollapse: 'collapse' as const }}>
                    <thead>
                        <tr>
                            <th style={thStyle}>Metric</th>
                            <th style={{ ...thStyle, textAlign: 'right' as const }}>SOL</th>
                            <th style={{ ...thStyle, textAlign: 'right' as const }}>USD ($)</th>
                            <th style={{ ...thStyle, textAlign: 'right' as const }}>GBP (\u00a3)</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows.map(row => {
                            const isNeg = row.pnl && row.sol < 0;
                            const isPos = row.pnl && row.sol >= 0;
                            const col = isNeg ? '#f87171' : isPos ? '#14F195' : 'rgba(255,255,255,0.8)';
                            const pre = row.pnl ? (row.sol >= 0 ? '+' : '') : '';
                            return (
                                <tr key={row.label}>
                                    <td style={{ ...tdStyle, color: 'rgba(255,255,255,0.5)', fontSize: 12, fontWeight: 600 }}>
                                        {row.label}
                                    </td>
                                    <td style={{ ...tdStyle, textAlign: 'right' as const, color: col, fontFamily: 'monospace' }}>
                                        {pre}{row.sol.toFixed(4)}
                                    </td>
                                    <td style={{ ...tdStyle, textAlign: 'right' as const, color: !usd ? 'rgba(255,255,255,0.3)' : col, fontFamily: 'monospace' }}>
                                        {usd ? `${pre}${fmt(Math.abs(row.sol), usd, '$')}` : '\u2014'}
                                    </td>
                                    <td style={{ ...tdStyle, textAlign: 'right' as const, color: !gbp ? 'rgba(255,255,255,0.3)' : col, fontFamily: 'monospace' }}>
                                        {gbp ? `${pre}${fmt(Math.abs(row.sol), gbp, '\u00a3')}` : '\u2014'}
                                    </td>
                                </tr>
                            );
                        })}
                    </tbody>
                </table>
            )}
        </div>
    );
}

// ─── Step 3: Solana Profile ───────────────────────────────────────────────────
function ProfileStep() {
    const { connection } = useConnection();
    const wallet = useWallet();
    const navigate = useNavigate();
    const [profile, setProfile] = useState<any>(null);
    const [loading, setLoading] = useState(true);
    const [createHandle, setCreateHandle] = useState('');
    const [country, setCountry] = useState('GB');
    const [taxId, setTaxId] = useState('');
    const [creating, setCreating] = useState(false);
    const [err, setErr] = useState<string | null>(null);

    const countries = [
        { code: 'GB', label: 'United Kingdom', taxLabel: 'NI Number' },
        { code: 'BR', label: 'Brazil', taxLabel: 'CPF' },
        { code: 'CA', label: 'Canada', taxLabel: 'SIN' },
        { code: 'DE', label: 'Germany', taxLabel: 'Tax ID' },
    ];

    const currentCountry = countries.find(c => c.code === country);

    // AI Setup state
    const [showAiSetup, setShowAiSetup] = useState(false);
    const [aiDifficulty, setAiDifficulty] = useState(1);
    const [aiSide, setAiSide] = useState<'white' | 'black' | 'random'>('random');

    useEffect(() => {
        const useHot = localStorage.getItem('xfchess_use_hot') === 'true';
        if (useHot) {
            setProfile({
                data: {
                    username: localStorage.getItem('xfchess_username') || 'Hot Player',
                    eloRating: 120000,
                    wins: 0,
                    losses: 0,
                    isVerified: false,
                    totalWagered: 0,
                    totalWon: 0,
                }
            });
            setLoading(false);
        } else if (wallet.connected && wallet.publicKey) {
            loadProfile();
        }
    }, [wallet.connected, wallet.publicKey]);

    const loadProfile = async () => {
        if (!wallet.publicKey) return;
        setLoading(true);
        setErr(null);
        try {
            // Check backend first: existing wallet-registered users skip profile creation.
            try {
                const pk = wallet.publicKey.toBase58();
                const r = await fetch(`${API}/api/auth/check-wallet/${pk}`);
                if (r.ok) {
                    const data = await r.json();
                    if (data?.registered && data?.username) {
                        localStorage.setItem('xfchess_username', data.username);
                        localStorage.setItem('xfchess_wallet', pk);
                        if (data.token) localStorage.setItem('xfchess_token', data.token);
                        navigate('/');
                        return;
                    }
                }
            } catch {
                // Not fatal — fall through to on-chain lookup.
            }

            const program = getAnchorProgram(connection, wallet);
            const p = await fetchPlayerProfile(program, wallet.publicKey);
            setProfile(p);
        } catch (e: any) {
            setErr(e.message || 'Failed to load profile');
        } finally {
            setLoading(false);
        }
    };

    const handleCreate = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!wallet.publicKey || !createHandle) return;
        setCreating(true);
        setErr(null);
        try {
            const program = getAnchorProgram(connection, wallet);
            // 1. On-chain initialization (Username + Country ONLY, no PII)
            await createPlayerProfile(program, wallet.publicKey, createHandle, country);
            
            // 2. Backend registration (timestamp in seconds for signature verification)
            let authToken = localStorage.getItem('xfchess_token');
            try {
                const timestamp = Math.floor(Date.now() / 1000);
                const message = `xfchess:register:${timestamp}`;
                const encodedMessage = new TextEncoder().encode(message);
                const signature = await wallet.signMessage!(encodedMessage);
                const sigStrB58 = bs58.encode(signature);
                const auth = await apiPost<{ token: string; username: string }>('/api/auth/register', {
                    wallet: wallet.publicKey.toBase58(),
                    signature: sigStrB58,
                    timestamp,
                    username: createHandle,
                });
                authToken = auth.token;
                localStorage.setItem('xfchess_token', auth.token);
                localStorage.setItem('xfchess_username', auth.username);
                localStorage.setItem('xfchess_wallet', wallet.publicKey.toBase58());
            } catch (e: any) {
                // 409 = already registered — existing token still valid
                if (!e.message?.includes('409') && !e.message?.includes('already')) {
                    console.warn("Backend registration skipped/failed:", e);
                }
            }

            // 3. Sync on-chain username → SQLite (canonical source of truth)
            if (authToken) {
                try {
                    const backendUrl = (import.meta.env.VITE_BACKEND_URL as string | undefined) || 'http://localhost:8090';
                    const r = await fetch(`${backendUrl}/api/auth/sync-profile`, {
                        method: 'POST',
                        headers: { Authorization: `Bearer ${authToken}` },
                    });
                    if (r.ok) {
                        const { username: synced } = await r.json();
                        localStorage.setItem('xfchess_username', synced);
                    }
                } catch (e) {
                    console.warn('sync-profile non-critical:', e);
                }
            }

            // Wait for chain confirmation then reload
            setTimeout(loadProfile, 1800);
        } catch (e: any) {
            let msg = e.message || 'Failed to create profile';
            if (msg.includes('already in use')) msg = 'Username already taken. Please try another.';
            setErr(msg);
        } finally {
            setCreating(false);
        }
    };

    const pk = wallet.publicKey?.toBase58() ?? '';
    const short = pk ? `${pk.slice(0, 6)}…${pk.slice(-4)}` : '';

    return (
        <div style={{ ...card, maxWidth: 520 }}>
            <StepDots current={2} />

            {loading && (
                <div style={{ textAlign: 'center', padding: '40px 0' }}>
                    <Loader2 size={32} style={{ color: '#ad5c2f', animation: 'spin 0.8s linear infinite' }} />
                    <p style={{ color: 'rgba(255,255,255,0.4)', marginTop: 12, fontSize: 13 }}>Loading on-chain profile…</p>
                </div>
            )}

            {err && !loading && <ErrBox msg={err} />}

            {!loading && profile && (
                <>
                    {/* Avatar + Name */}
                    <div style={{
                        display: 'flex', alignItems: 'center', gap: 16,
                        padding: '20px', background: 'rgba(255,255,255,0.03)',
                        borderRadius: 12, border: '1px solid rgba(255,255,255,0.06)',
                        marginBottom: 20,
                    }}>
                        <div style={{
                            width: 56, height: 56, borderRadius: '50%',
                            background: 'linear-gradient(135deg, #ad5c2f, #8c4a26)',
                            display: 'flex', alignItems: 'center', justifyContent: 'center',
                            flexShrink: 0,
                        }}>
                            <Shield size={24} color="#fff" />
                        </div>
                        <div>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                                <h3 style={{ margin: 0, fontSize: 24, fontWeight: 900 }}>
                                    {profile.data.username || 'Unnamed'}
                                </h3>
                                {profile.data.isVerified && (
                                    <span style={{
                                        fontSize: 11, background: 'rgba(20,241,149,0.1)',
                                        color: '#14F195', padding: '3px 10px', borderRadius: 10,
                                        border: '1px solid rgba(20,241,149,0.3)',
                                    }}>
                                        <ShieldCheck size={11} style={{ verticalAlign: 'middle', marginRight: 4 }} />
                                        Verified
                                    </span>
                                )}
                            </div>
                            <p style={{ margin: '4px 0 0', fontSize: 12, color: 'rgba(255,255,255,0.35)', fontFamily: 'monospace' }}>{short}</p>
                        </div>
                    </div>

                    {/* Stats grid */}
                    <div style={{
                        display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)',
                        gap: 10, marginBottom: 16,
                    }}>
                        {[
                            { label: 'ELO', value: Math.round((profile.data.eloRating || profile.data.elo_rating || 120000) / 100) },
                            { label: 'Wins', value: profile.data.wins ?? 0 },
                            { label: 'Losses', value: profile.data.losses ?? 0 },
                            { label: 'Streak', value: profile.data.winStreak ?? profile.data.win_streak ?? 0 },
                        ].map(stat => (
                            <div key={stat.label} style={{
                                padding: '14px 8px', background: 'rgba(255,255,255,0.03)',
                                borderRadius: 10, border: '1px solid rgba(255,255,255,0.06)',
                                textAlign: 'center',
                            }}>
                                <div style={{ fontSize: 22, fontWeight: 900, color: '#ad5c2f' }}>{stat.value}</div>
                                <div style={{ fontSize: 10, color: 'rgba(255,255,255,0.4)', marginTop: 2, letterSpacing: '0.06em', textTransform: 'uppercase' }}>{stat.label}</div>
                            </div>
                        ))}
                    </div>

                    {/* Wager table with live currency conversion */}
                    <WagerTable profile={profile} />

                    {/* Play Options */}
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginTop: '10px' }}>
                        <button
                            style={{ ...primaryBtn, background: 'linear-gradient(135deg, #ad5c2f, #f4bb44)' }}
                            onClick={async () => {
                                const useHot = localStorage.getItem('xfchess_use_hot') === 'true';
                                const body = {
                                    pubkey: wallet.publicKey?.toBase58() || "hot-wallet-dummy",
                                    hot: useHot,
                                    username: profile.data.username
                                };
                                try {
                                    await apiPost('/api/game/launch', body);
                                } catch (e) {
                                    navigate('/download');
                                }
                            }}
                        >
                            <Zap size={16} />
                            Host Wagered Match
                        </button>
                        
                        <div style={{ display: 'flex', gap: '12px' }}>
                            <button
                                style={{ ...primaryBtn, flex: 1, background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', boxShadow: 'none', fontSize: 13 }}
                                onClick={() => setShowAiSetup(true)}
                            >
                                <Cpu size={16} />
                                Play Computer
                            </button>

                            <button
                                style={{ ...primaryBtn, flex: 1, background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', boxShadow: 'none', fontSize: 13 }}
                                onClick={async () => {
                                    const useHot = localStorage.getItem('xfchess_use_hot') === 'true';
                                    const body = {
                                        pubkey: wallet.publicKey?.toBase58() || "hot-wallet-dummy",
                                        hot: useHot,
                                        username: profile.data.username
                                    };
                                    try {
                                        await apiPost('/api/game/launch', { ...body, token: localStorage.getItem('xfchess_token') });
                                    } catch (e) {
                                        navigate('/play');
                                    }
                                }}
                            >
                                <ChevronRight size={16} />
                                Join Game
                            </button>
                            <button
                                style={{ ...primaryBtn, flex: 1, background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', boxShadow: 'none' }}
                                onClick={() => navigate('/download')}
                            >
                                <Trophy size={16} />
                                Tournaments
                            </button>
                        </div>
                    </div>


                    {/* AI Configuration Modal */}
                    {showAiSetup && (
                        <div style={modalOverlay}>
                            <div style={modalContent}>
                                <div style={modalHeader}>
                                    <h2 style={{ fontSize: 28, fontWeight: 300, color: 'rgba(255,255,255,0.8)', margin: 0, fontFamily: '"Space Grotesk", sans-serif' }}>
                                        Game setup
                                    </h2>
                                    <button onClick={() => setShowAiSetup(false)} style={closeBtn}><X size={20} /></button>
                                </div>

                                <div style={setupSection}>
                                    <div style={setupLabel}>Strength (ELO equivalent)</div>
                                    <div style={strengthGrid}>
                                        {[1, 2, 3, 4, 5, 6, 7, 8].map(lvl => {
                                            const elos = [0, 400, 700, 1000, 1300, 1600, 1900, 2200, 2500];
                                            return (
                                                <button
                                                    key={lvl}
                                                    onClick={() => setAiDifficulty(lvl)}
                                                    style={{
                                                        ...strengthBtn,
                                                        background: aiDifficulty === lvl ? '#ad5c2f' : 'rgba(255,255,255,0.03)',
                                                        color: aiDifficulty === lvl ? '#fff' : 'rgba(255,255,255,0.4)',
                                                        borderColor: aiDifficulty === lvl ? '#ad5c2f' : 'rgba(255,255,255,0.1)',
                                                    }}
                                                    title={`${elos[lvl]} ELO`}
                                                >
                                                    {lvl}
                                                </button>
                                            );
                                        })}
                                    </div>
                                    <div style={{ textAlign: 'center', fontSize: 11, color: '#ad5c2f', marginTop: 8, fontWeight: 700, textTransform: 'uppercase' }}>
                                        {[0, 400, 700, 1000, 1300, 1600, 1900, 2200, 2500][aiDifficulty]} ELO EQUIVALENT
                                    </div>
                                </div>

                                <div style={setupSection}>
                                    <div style={setupLabel}>Side</div>
                                    <div style={sideGrid}>
                                        <button 
                                            onClick={() => setAiSide('black')}
                                            style={{ ...sideBtn, background: aiSide === 'black' ? '#ad5c2f' : 'rgba(255,255,255,0.03)', borderColor: aiSide === 'black' ? '#ad5c2f' : 'rgba(255,255,255,0.1)' }}
                                        >
                                            <span style={{ fontSize: 24 }}>♟</span>
                                            <div>Black</div>
                                        </button>
                                        <button 
                                            onClick={() => setAiSide('random')}
                                            style={{ ...sideBtn, background: aiSide === 'random' ? '#ad5c2f' : 'rgba(255,255,255,0.03)', borderColor: aiSide === 'random' ? '#ad5c2f' : 'rgba(255,255,255,0.1)' }}
                                        >
                                            <span style={{ fontSize: 24 }}>☯</span>
                                            <div>Random</div>
                                        </button>
                                        <button 
                                            onClick={() => setAiSide('white')}
                                            style={{ ...sideBtn, background: aiSide === 'white' ? '#ad5c2f' : 'rgba(255,255,255,0.03)', borderColor: aiSide === 'white' ? '#ad5c2f' : 'rgba(255,255,255,0.1)' }}
                                        >
                                            <span style={{ fontSize: 24 }}>♙</span>
                                            <div>White</div>
                                        </button>
                                    </div>
                                </div>

                                <button
                                    style={launchBtn}
                                    onClick={async () => {
                                        const finalSide = aiSide === 'random' ? (Math.random() > 0.5 ? 'white' : 'black') : aiSide;
                                        const useHot = localStorage.getItem('xfchess_use_hot') === 'true';
                                        const body = {
                                            pubkey: wallet.publicKey?.toBase58() || "hot-wallet-dummy",
                                            hot: useHot,
                                            username: profile.data.username,
                                            ai_difficulty: aiDifficulty,
                                            ai_side: finalSide === 'white' ? 'black' : 'white' // AI plays opposite of player
                                        };
                                        try {
                                            await apiPost('/api/game/launch', { ...body, token: localStorage.getItem('xfchess_token') });
                                        } catch (e) {
                                            navigate('/play');
                                        }
                                    }}
                                >
                                    <Cpu size={18} style={{ marginRight: 10 }} />
                                    Play against computer
                                </button>
                            </div>
                        </div>
                    )}
                </>
            )}

            {!loading && !profile && (
                <>
                    <div style={{
                        padding: '24px', background: 'rgba(255,255,255,0.02)',
                        borderRadius: 12, border: '1px dashed rgba(255,255,255,0.1)',
                        marginBottom: 20, textAlign: 'center',
                    }}>
                        <Trophy size={36} style={{ color: '#ad5c2f', opacity: 0.5, marginBottom: 12 }} />
                        <h3 style={{ margin: '0 0 8px', fontSize: 18, fontWeight: 800 }}>No On-Chain Profile Found</h3>
                        <p style={{ color: 'rgba(255,255,255,0.4)', fontSize: 13, margin: 0 }}>
                            Create your username on Solana to start your competitive journey.
                        </p>
                    </div>

                    <form onSubmit={handleCreate} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                        <div style={{ marginBottom: 4 }}>
                            <label style={{ display: 'block', fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,0.4)', marginBottom: 6, letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                                Chess Handle (Username)
                            </label>
                            <input
                                style={{ ...input, fontSize: 18, fontWeight: 700, textAlign: 'center' }}
                                value={createHandle}
                                onChange={e => setCreateHandle(e.target.value)}
                                placeholder="YourChessHandle"
                                maxLength={20}
                                required
                                onFocus={e => (e.target.style.borderColor = '#ad5c2f')}
                                onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.1)')}
                            />
                        </div>

                        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                                <label style={{ fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,0.4)', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                                    Country
                                </label>
                                <select
                                    style={{ ...input, padding: '10px 12px' }}
                                    value={country}
                                    onChange={e => setCountry(e.target.value)}
                                >
                                    {countries.map(c => (
                                        <option key={c.code} value={c.code} style={{ background: '#1a1a17' }}>{c.label}</option>
                                    ))}
                                </select>
                            </div>
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                                <label style={{ fontSize: 11, fontWeight: 700, color: 'rgba(255,255,255,0.4)', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
                                    {currentCountry?.taxLabel ?? 'Tax ID'}
                                </label>
                                <input
                                    style={{ ...input, padding: '10px 12px' }}
                                    value={taxId}
                                    onChange={e => setTaxId(e.target.value)}
                                    placeholder="Required"
                                    required
                                />
                            </div>
                        </div>
                        <button type="submit" style={{ ...primaryBtn, opacity: creating || !createHandle || !taxId ? 0.6 : 1, marginTop: 8 }} disabled={creating || !createHandle || !taxId}>
                            {creating ? <Loader2 size={16} className="spinner" /> : <Zap size={16} />}
                            Initialize Profile
                        </button>
                    </form>
                </>
            )}
        </div>
    );
}

// ─── Root: orchestrates the steps ───────────────────────────────────────────
export function SignIn(_: { defaultMode?: 'login' | 'register' } = {}) {
    const [step, setStep] = useState<FlowStep>(() => {
        const params = new URLSearchParams(window.location.search);
        const s = params.get('step');
        if (s === 'connect_wallet') return 'connect_wallet';
        if (s === 'identity') return 'identity';
        return 'identity';
    });
    const [credMode, setCredMode] = useState<'login' | 'register'>('login');
    const [authUser, setAuthUser] = useState<AuthResult | null>(null);
    const { connected } = useWallet();

    // If already authed + wallet connected on mount, jump to profile
    useEffect(() => {
        const token = localStorage.getItem('xfchess_token');
        const user = localStorage.getItem('xfchess_username');
        if (token && user) {
            setAuthUser({ token, username: user });
            if (connected) {
                setStep('profile');
            } else {
                setStep('connect_wallet');
            }
        }
    }, []);

    const handleAuth = (r: AuthResult) => {
        setAuthUser(r);
        setStep('connect_wallet');
    };

    const handleConnected = useCallback(() => {
        setStep('profile');
    }, []);

    return (
        <main style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', paddingTop: 80, paddingBottom: 40 }}>
            {step === 'identity' && (
                <IdentityStep
                    onWallet={() => setStep('wallet_login')}
                    onEmail={(mode) => { setCredMode(mode); setStep('credentials'); }}
                />
            )}
            {step === 'wallet_login' && (
                <ConnectWalletStep username="" onConnected={handleConnected} />
            )}
            {step === 'credentials' && (
                <CredentialsStep mode={credMode} onAuth={handleAuth} />
            )}
            {step === 'connect_wallet' && (
                <ConnectWalletStep username={authUser?.username ?? 'Player'} onConnected={handleConnected} />
            )}
            {step === 'profile' && (
                <ProfileStep />
            )}
        </main>
    );
}

// ─── Styles ──────────────────────────────────────────────────────────────────
const modalOverlay: React.CSSProperties = {
    position: 'fixed', inset: 0, zIndex: 1000,
    background: 'rgba(0,0,0,0.85)', backdropFilter: 'blur(8px)',
    display: 'flex', alignItems: 'center', justifyContent: 'center'
};

const modalContent: React.CSSProperties = {
    width: '92%', maxWidth: 480, padding: 32,
    background: '#1a1a17', border: '1px solid rgba(255,255,255,0.08)', borderRadius: 16,
    boxShadow: '0 20px 80px rgba(0,0,0,0.8)'
};

const modalHeader: React.CSSProperties = {
    display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 32
};

const closeBtn: React.CSSProperties = {
    background: 'none', border: 'none', color: 'rgba(255,255,255,0.3)', cursor: 'pointer'
};

const setupSection: React.CSSProperties = {
    marginBottom: 32, textAlign: 'center'
};

const setupLabel: React.CSSProperties = {
    fontSize: 14, color: 'rgba(255,255,255,0.6)', marginBottom: 16, letterSpacing: '0.04em', textTransform: 'uppercase'
};

const strengthGrid: React.CSSProperties = {
    display: 'grid', gridTemplateColumns: 'repeat(8, 1fr)', gap: 4, background: 'rgba(255,255,255,0.02)', padding: 4, borderRadius: 8
};

const strengthBtn: React.CSSProperties = {
    height: 44, border: '1px solid transparent', borderRadius: 6, cursor: 'pointer', fontSize: 14, fontWeight: 700, transition: 'all 0.2s', display: 'flex', alignItems: 'center', justifyContent: 'center'
};

const sideGrid: React.CSSProperties = {
    display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12
};

const sideBtn: React.CSSProperties = {
    padding: '16px 0', border: '1px solid transparent', borderRadius: 8, cursor: 'pointer', color: '#fff', fontSize: 12, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8, transition: 'all 0.2s'
};

const launchBtn: React.CSSProperties = {
    width: '100%', padding: '16px 0', borderRadius: 8, border: 'none', background: '#ad5c2f', color: '#fff', fontSize: 16, fontWeight: 700, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'filter 0.2s'
};
