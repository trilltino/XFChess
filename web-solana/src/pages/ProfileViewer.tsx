import { useState, useEffect, useCallback } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PublicKey } from '@solana/web3.js';
import { Search, Shield, Trophy, Loader2, ShieldCheck, AlertTriangle, Gamepad2, Download } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { getAnchorProgram, fetchPlayerProfile, createPlayerProfile } from '../lib/anchor_client';
import { useKycStatus } from '../lib/useKycStatus';

export function ProfileViewer() {
    const { connection } = useConnection();
    const wallet = useWallet();
    const navigate = useNavigate();
    const { kycStatus, kycLoading } = useKycStatus();
    const [searchQuery, setSearchQuery] = useState('');
    
    const [profile, setProfile] = useState<any>(null);
    const [loading, setLoading] = useState(false);
    const [creationLoading, setCreationLoading] = useState(false);
    const [newUsername, setNewUsername] = useState('');
    const [error, setError] = useState<string | null>(null);
    const [showDownloadPrompt, setShowDownloadPrompt] = useState(false);
    const [isLaunching, setIsLaunching] = useState(false);

    // Initial load: fetch connected wallet's profile if any
    useEffect(() => {
        if (wallet.connected && wallet.publicKey) {
            loadProfile(wallet.publicKey);
        } else {
            setProfile(null);
            setError(null);
        }
    }, [wallet.connected, wallet.publicKey]);

    const loadProfile = async (pubkey: PublicKey) => {
        setLoading(true);
        setError(null);
        try {
            const program = getAnchorProgram(connection, wallet);
            const p = await fetchPlayerProfile(program, pubkey);
            if (p) {
                setProfile(p);
            } else {
                setProfile(null);
                if (wallet.publicKey?.toBase58() !== pubkey.toBase58()) {
                    setError("Profile not found for this address.");
                }
            }
        } catch (err: any) {
            console.error(err);
            setError(err.message || "Failed to load profile.");
        } finally {
            setLoading(false);
        }
    };

    const handleSearch = (e: React.FormEvent) => {
        e.preventDefault();
        try {
            const pk = new PublicKey(searchQuery);
            loadProfile(pk);
        } catch (err) {
            setError("Invalid Solana public key format.");
        }
    };

    const handleCreateProfile = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!wallet.connected || !wallet.publicKey) return;
        
        setCreationLoading(true);
        setError(null);
        try {
            const program = getAnchorProgram(connection, wallet);
            console.log("Current Cluster:", connection.rpcEndpoint);
            console.log("Program ID:", program.programId.toBase58());
            
            await createPlayerProfile(program, wallet.publicKey, newUsername);
            setTimeout(() => {
                loadProfile(wallet.publicKey!);
            }, 1500); // give time for the network to process
        } catch (err: any) {
            console.error(err);
            let msg = err.message || "Failed to create profile.";
            if (msg.includes("already in use") || (err.logs && err.logs.some((l: string) => l.includes("already in use")))) {
                msg = "Username already taken or outdated profile. Please try a different username or use a new wallet.";
            }
            setError(msg);
        } finally {
            setCreationLoading(false);
        }
    };

    const handleLaunchGame = useCallback(() => {
        setIsLaunching(true);
        setShowDownloadPrompt(false);
        
        // Try to open the XFChess app via custom protocol
        const protocolUrl = 'xfchess://launch';
        
        // Use a hidden iframe to detect if protocol is registered
        // If the app isn't installed, we'll show the download prompt after a short delay
        const iframe = document.createElement('iframe');
        iframe.style.display = 'none';
        document.body.appendChild(iframe);
        
        // Also try window.location for browsers that support it
        try {
            window.location.href = protocolUrl;
        } catch (e) {
            // Ignore - protocol handlers may throw in some browsers
        }
        
        // If app is not installed, we'll show download prompt after delay
        // The blur event would fire if app opened, but we'll use a simpler approach:
        // Just show a message that we're trying to launch, and provide download option
        setTimeout(() => {
            setIsLaunching(false);
            // Show download prompt as fallback (user can dismiss if game launched)
            setShowDownloadPrompt(true);
        }, 2000);
        
        // Cleanup iframe
        setTimeout(() => {
            document.body.removeChild(iframe);
        }, 100);
    }, []);


    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <div className="section-label">LAYER LOOKUP</div>
            <h2 style={{ fontSize: '2.5rem' }}>Global Directory<span className="accent">.</span></h2>
            
            <form onSubmit={handleSearch} style={{ display: 'flex', gap: '12px', marginBottom: '40px', maxWidth: '600px' }}>
                <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="Enter Solana Base58 Address..."
                    style={{ flex: 1, padding: '16px 20px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--glass)', color: '#fff', fontSize: '1rem' }}
                />
                <button type="submit" className="btn btn-primary" style={{ width: 'auto', padding: '0 32px' }} disabled={loading}>
                    {loading ? <Loader2 className="spinner" /> : <Search />}
                    Search
                </button>
            </form>

            <div className="profile-section-wrap" style={{ marginTop: '0', padding: '0', display: 'block' }}>
                {wallet.connected ? (
                    <div className="profile-card">
                        {loading && <div style={{ textAlign: 'center' }}><Loader2 className="spinner" style={{ margin: '0 auto', width: '30px', height: '30px', color: 'var(--primary)' }} /></div>}
                        
                        {/* KYC Banner — only shown for connected wallet's own profile */}
                        {wallet.connected && !kycLoading && kycStatus && !kycStatus.verified && (
                            <div style={{
                                display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '16px',
                                padding: '16px 20px', marginBottom: '24px',
                                background: 'rgba(255, 170, 0, 0.08)', border: '1px solid rgba(255, 170, 0, 0.35)',
                                borderRadius: '10px'
                            }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                                    <AlertTriangle size={20} color="#FFAA00" style={{ flexShrink: 0 }} />
                                    <div>
                                        <div style={{ fontWeight: 700, fontSize: '0.95rem', color: '#FFAA00', marginBottom: '2px' }}>
                                            Identity Verification Required for Wagered Games
                                        </div>
                                        <div style={{ fontSize: '0.82rem', color: 'var(--text-dim)' }}>
                                            CARF 2026 compliance requires identity verification before joining tournaments or placing wagers.
                                        </div>
                                    </div>
                                </div>
                                <button
                                    onClick={() => navigate('/verify')}
                                    className="btn btn-primary"
                                    style={{ whiteSpace: 'nowrap', width: 'auto', padding: '0 20px', fontSize: '0.85rem', height: '38px', flexShrink: 0 }}
                                >
                                    <ShieldCheck size={15} style={{ marginRight: '8px' }} />
                                    Verify Now
                                </button>
                            </div>
                        )}

                        {wallet.connected && !kycLoading && kycStatus?.verified && (
                            <div style={{
                                display: 'flex', alignItems: 'center', gap: '10px',
                                padding: '10px 16px', marginBottom: '20px',
                                background: 'rgba(20, 241, 149, 0.06)', border: '1px solid rgba(20, 241, 149, 0.25)',
                                borderRadius: '8px', fontSize: '0.85rem', color: '#14F195'
                            }}>
                                <ShieldCheck size={16} />
                                Identity verified — eligible for wagered tournaments
                            </div>
                        )}

                        {!loading && profile && (
                            <div>
                                <div className="connected-header">
                                    <div className="connected-avatar">
                                        <Shield color="#fff" />
                                    </div>
                                    <div className="connected-meta">
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '8px' }}>
                                            <h3 style={{ margin: 0, fontSize: '2rem', fontWeight: 900 }}>
                                                {profile.data.username || (wallet.publicKey?.toBase58() === profile.pubkey.toBase58() ? "Set Your Username" : "Anonymous")}
                                            </h3>
                                            {profile.data.isVerified && (
                                                <span style={{ fontSize: '0.8rem', background: 'rgba(20, 241, 149, 0.1)', color: '#14F195', padding: '4px 12px', borderRadius: '12px', border: '1px solid rgba(20, 241, 149, 0.3)' }}>
                                                    Verified 
                                                </span>
                                            )}
                                        </div>
                                    </div>
                                </div>

                                <div className="connected-stats">
                                    <div className="cs e">
                                        <div className="v">{Math.round((profile.data.eloRating ?? 120000) / 100)}</div>
                                        <div className="l">Elo Rating</div>
                                    </div>
                                    <div className="cs">
                                        <div className="v">{profile.data.wins || 0}</div>
                                        <div className="l">Wins</div>
                                    </div>
                                    <div className="cs">
                                        <div className="v">{profile.data.losses || 0}</div>
                                        <div className="l">Losses</div>
                                    </div>
                                    <div className="cs">
                                        <div className="v">{profile.data.winStreak || 0}</div>
                                        <div className="l">Streak</div>
                                    </div>
                                </div>

                                <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginTop: '20px' }}>
                                    <button
                                        className="btn btn-primary"
                                        style={{ background: 'linear-gradient(135deg, #ad5c2f, #f4bb44)', border: 'none' }}
                                        onClick={handleLaunchGame}
                                        disabled={isLaunching}
                                    >
                                        {isLaunching ? (
                                            <Loader2 size={18} className="spinner" style={{ marginRight: '8px' }} />
                                        ) : (
                                            <Gamepad2 size={18} style={{ marginRight: '8px' }} />
                                        )}
                                        {isLaunching ? 'Launching...' : 'Launch Game'}
                                    </button>
                                    
                                    <div style={{ display: 'flex', gap: '12px' }}>
                                        <button
                                            className="btn btn-secondary"
                                            style={{ flex: 1 }}
                                            onClick={() => navigate('/download')}
                                        >
                                            <Download size={18} style={{ marginRight: '8px' }} />
                                            Download App
                                        </button>
                                        <button
                                            className="btn btn-secondary"
                                            style={{ flex: 1 }}
                                            onClick={() => navigate('/download')}
                                        >
                                            <Trophy size={18} style={{ marginRight: '8px' }} />
                                            Tournaments
                                        </button>
                                    </div>
                                </div>

                                {/* Download Prompt - shown if app not detected */}
                                {showDownloadPrompt && (
                                    <div style={{
                                        marginTop: '16px',
                                        padding: '16px 20px',
                                        background: 'rgba(173, 92, 47, 0.08)',
                                        border: '1px solid rgba(173, 92, 47, 0.3)',
                                        borderRadius: '10px',
                                        textAlign: 'center'
                                    }}>
                                        <p style={{ fontSize: '0.9rem', marginBottom: '12px', color: 'var(--text-dim)' }}>
                                            Didn't launch? The XFChess desktop app may not be installed.
                                        </p>
                                        <button
                                            className="btn btn-primary"
                                            style={{ width: 'auto', padding: '10px 24px', fontSize: '0.9rem' }}
                                            onClick={() => navigate('/download')}
                                        >
                                            <Download size={16} style={{ marginRight: '8px' }} />
                                            Download XFChess
                                        </button>
                                        <button
                                            className="btn btn-secondary"
                                            style={{ width: 'auto', padding: '10px 24px', fontSize: '0.9rem', marginLeft: '8px' }}
                                            onClick={() => setShowDownloadPrompt(false)}
                                        >
                                            Dismiss
                                        </button>
                                    </div>
                                )}

                                <p style={{ textAlign: 'center', fontSize: '0.75rem', color: 'var(--text-dim)', marginTop: '16px', opacity: 0.6 }}>
                                    Click "Launch Game" to open the XFChess desktop client.
                                </p>
                            </div>
                        )}

                                {(!searchQuery || searchQuery === wallet.publicKey?.toBase58()) && (
                                    <div style={{ background: 'rgba(255, 255, 255, 0.02)', padding: '30px', borderRadius: '12px', marginTop: '20px', border: '1px solid var(--border)' }}>
                                        <p style={{ marginBottom: '20px', fontSize: '1.1rem' }}>Welcome to XFChess! Choose a handle to start your competitive journey.</p>
                                        <form onSubmit={handleCreateProfile} style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                                            <input
                                                type="text"
                                                value={newUsername}
                                                onChange={(e) => setNewUsername(e.target.value)}
                                                placeholder="Enter Username"
                                                required
                                                maxLength={20}
                                                style={{ padding: '16px', borderRadius: '8px', border: '1px solid var(--primary)', background: 'var(--bg)', color: '#fff', textAlign: 'center', fontSize: '1.2rem', fontWeight: 600 }}
                                            />
                                            <button type="submit" className="btn btn-primary" disabled={creationLoading || !newUsername}>
                                                {creationLoading ? <Loader2 className="spinner" /> : "Secure Profile"}
                                            </button>
                                        </form>
                                    </div>
                                )}

                        
                        {error && !loading && (
                            <div style={{ color: 'var(--primary)', marginTop: '20px', padding: '16px', background: 'rgba(230, 57, 70, 0.1)', borderRadius: '8px', border: '1px solid rgba(230, 57, 70, 0.3)' }}>
                                {error}
                            </div>
                        )}
                    </div>
                ) : (
                    <div style={{ textAlign: 'center', padding: '60px 0', border: '1px dashed var(--border)', borderRadius: '12px', background: 'var(--glass)' }}>
                        <Trophy size={48} style={{ opacity: 0.3, marginBottom: '20px', color: 'var(--primary)' }} />
                        <h3 style={{ fontSize: '1.2rem', marginBottom: '8px' }}>Wallet Disconnected</h3>
                        <p style={{ color: 'var(--text-dim)' }}>Connect your Solana wallet to view or create your player profile.</p>
                    </div>
                )}
            </div>
        </main>
    );
}
