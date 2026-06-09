import { useState } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PublicKey } from '@solana/web3.js';
import { Search, Shield, Trophy, Loader2 } from 'lucide-react';
import {
  getAnchorProgram,
  fetchPlayerProfile,
  fetchProfileByUsername,
} from '../lib/anchor_client';

export function Players() {
  const { connection } = useConnection();
  const wallet = useWallet();
  const [searchQuery, setSearchQuery] = useState('');
  const [profile, setProfile] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!wallet.connected || !wallet.publicKey) {
      setError('Connect wallet to search profiles.');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      try {
        const pk = new PublicKey(searchQuery);
        const program = getAnchorProgram(connection, wallet);
        const p = await fetchPlayerProfile(program, pk);
        if (p) setProfile(p);
        else setError('Profile not found for this address.');
      } catch {
        const program = getAnchorProgram(connection, wallet);
        const p = await fetchProfileByUsername(program, searchQuery);
        if (p) setProfile(p);
        else setError('Profile not found for this username or address.');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to search profile.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
      <div style={{ maxWidth: '800px', margin: '0 auto', padding: '0 20px' }}>
        <div className="section-label">Player Lookup</div>
        <h2 style={{ fontSize: '2.5rem', textAlign: 'center' }}>
          Global Directory<span className="accent">.</span>
        </h2>

        <form
          onSubmit={handleSearch}
          style={{
            display: 'flex',
            gap: '12px',
            marginTop: '32px',
            marginBottom: '40px',
            maxWidth: '600px',
            marginLeft: 'auto',
            marginRight: 'auto',
          }}
        >
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Look up player profile"
            style={{
              flex: 1,
              padding: '16px 20px',
              borderRadius: 8,
              border: '1px solid var(--border)',
              background: 'var(--glass)',
              color: '#fff',
              fontSize: '1rem',
            }}
          />
          <button
            type="submit"
            className="btn btn-primary"
            style={{ width: 'auto', padding: '0 32px' }}
            disabled={loading}
          >
            {loading ? <Loader2 className="spinner" /> : <Search />} Search
          </button>
        </form>

        {wallet.connected ? (
          <div className="profile-section-wrap" style={{ marginTop: 0, padding: 0, display: 'block' }}>
            <div className="profile-card">
              {loading && (
                <div style={{ textAlign: 'center' }}>
                  <Loader2
                    className="spinner"
                    style={{ margin: '0 auto', width: 30, height: 30, color: 'var(--primary)' }}
                  />
                </div>
              )}

              {!loading && profile && (
                <div>
                  <div className="connected-header">
                    <div className="connected-avatar">
                      <Shield color="#fff" />
                    </div>
                    <div className="connected-meta">
                      <h3 style={{ margin: 0, fontSize: '2rem', fontWeight: 900 }}>
                        {profile.data.username || 'Anonymous'}
                      </h3>
                      {profile.data.isVerified && (
                        <span
                          style={{
                            fontSize: '0.8rem',
                            background: 'rgba(20, 241, 149, 0.1)',
                            color: '#ffffff',
                            padding: '4px 12px',
                            borderRadius: 12,
                            border: '1px solid rgba(20, 241, 149, 0.3)',
                          }}
                        >
                          Verified
                        </span>
                      )}
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
                </div>
              )}

              {error && !loading && (
                <div
                  style={{
                    color: 'var(--primary)',
                    marginTop: 20,
                    padding: 16,
                    background: 'rgba(230, 57, 70, 0.1)',
                    borderRadius: 8,
                    border: '1px solid rgba(230, 57, 70, 0.3)',
                  }}
                >
                  {error}
                </div>
              )}
            </div>
          </div>
        ) : (
          <div
            style={{
              textAlign: 'center',
              padding: '60px 0',
              border: '1px dashed var(--border)',
              borderRadius: 12,
              background: 'var(--glass)',
            }}
          >
            <Trophy size={48} style={{ opacity: 0.3, marginBottom: 20, color: 'var(--primary)' }} />
            <h3 style={{ fontSize: '1.2rem', marginBottom: 8 }}>Wallet Disconnected</h3>
            <p style={{ color: 'var(--text-dim)' }}>
              Connect your Solana wallet to look up player profiles.
            </p>
          </div>
        )}
      </div>
    </main>
  );
}

