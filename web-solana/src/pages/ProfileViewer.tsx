import { useState, useEffect } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PublicKey } from '@solana/web3.js';
import { Search, Shield, Trophy, Loader2 } from 'lucide-react';
import { getAnchorProgram, fetchPlayerProfile, createPlayerProfile, fetchProfileByUsername } from '../lib/anchor_client';

export function ProfileViewer() {
    const { connection } = useConnection();
    const wallet = useWallet();
    const [searchQuery, setSearchQuery] = useState('');
    
    const [profile, setProfile] = useState<any>(null);
    const [loading, setLoading] = useState(false);
    const [creationLoading, setCreationLoading] = useState(false);
    const [newUsername, setNewUsername] = useState('');
    const [error, setError] = useState<string | null>(null);

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

    const handleSearch = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!wallet.connected || !wallet.publicKey) {
            setError("Connect wallet to search profiles.");
            return;
        }

        setLoading(true);
        setError(null);

        try {
            // Try to parse as Solana address first
            try {
                const pk = new PublicKey(searchQuery);
                await loadProfile(pk);
            } catch (err) {
                // Not a valid address, treat as username
                const program = getAnchorProgram(connection, wallet);
                const profile = await fetchProfileByUsername(program, searchQuery);
                if (profile) {
                    setProfile(profile);
                } else {
                    setError("Profile not found for this username or address.");
                }
            }
        } catch (err: any) {
            setError(err.message || "Failed to search profile.");
        } finally {
            setLoading(false);
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

    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
            <div style={{ maxWidth: '800px', margin: '0 auto', padding: '0 20px' }}>
                <div className="section-label">LAYER LOOKUP</div>
                <h2 style={{ fontSize: '2.5rem', textAlign: 'center' }}>Global Directory<span className="accent">.</span></h2>
                
                <form onSubmit={handleSearch} style={{ display: 'flex', gap: '12px', marginBottom: '40px', maxWidth: '600px', margin: '0 auto 40px auto' }}>
                    <input
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        placeholder="Look up player profile"
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
                                
                                {/* Launch Game Button */}
                                <div style={{ marginTop: '24px', textAlign: 'center' }}>
                                    <a 
                                        href={`xfchess://launch?pubkey=${wallet.publicKey?.toBase58()}&username=${profile.data.username || ''}`}
                                        className="btn btn-primary"
                                        style={{ display: 'inline-flex', alignItems: 'center', gap: '8px', padding: '16px 32px', fontSize: '1.1rem' }}
                                    >
                                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                            <polygon points="5 3 19 12 5 21 5 3"></polygon>
                                        </svg>
                                        Launch Game
                                    </a>
                                    <p style={{ fontSize: '0.8rem', color: 'var(--text-dim)', marginTop: '8px' }}>
                                        Requires XFChess desktop app
                                    </p>
                                </div>
                            </div>
                        )}

                                {(!searchQuery || searchQuery === wallet.publicKey?.toBase58()) && (
                                    <div style={{ background: 'rgba(255, 255, 255, 0.02)', padding: '30px', borderRadius: '12px', marginTop: '20px', border: '1px solid var(--border)' }}>
                                        <form onSubmit={handleCreateProfile} style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                                            <input
                                                type="text"
                                                value={newUsername}
                                                onChange={(e) => setNewUsername(e.target.value)}
                                                placeholder="Create Username"
                                                required
                                                maxLength={20}
                                                style={{ padding: '16px', borderRadius: '8px', border: '1px solid var(--primary)', background: 'var(--bg)', color: '#fff', textAlign: 'center', fontSize: '1.2rem', fontWeight: 600 }}
                                            />
                                            <button type="submit" className="btn btn-primary" disabled={creationLoading || !newUsername}>
                                                {creationLoading ? <Loader2 className="spinner" /> : "Create"}
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
            </div>
        </main>
    );
}
