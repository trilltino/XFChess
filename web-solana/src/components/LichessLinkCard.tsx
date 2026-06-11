import { useState } from 'react';

interface LichessLinkCardProps {
  walletPubkey: string | null;
  lichessUsername?: string;
  lichessBlitz?: number;
  lichessRapid?: number;
  lichessBullet?: number;
  lichessVerified?: boolean;
}

// Lichess knight icon (simplified SVG path)
function LichessIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 45 45" fill="currentColor" style={{ flexShrink: 0 }}>
      <path d="M22 10c10.5 1 16.5 8 16 29H15c0-9 10-6.5 8-21" />
      <path d="M24 18c.38 5.12-2.07 7.04-5 8" strokeWidth="1.5" stroke="currentColor" fill="none" />
      <circle cx="15" cy="15" r="2" />
    </svg>
  );
}

export function LichessLinkCard({
  walletPubkey,
  lichessUsername,
  lichessBlitz,
  lichessRapid,
  lichessBullet,
  lichessVerified,
}: LichessLinkCardProps) {
  const [linking, setLinking] = useState(false);

  const handleLink = async () => {
    if (!walletPubkey || linking) return;
    setLinking(true);
    try {
      const { initLichessLink } = await import('../lib/api/lichess');
      const { authUrl } = await initLichessLink(walletPubkey);
      const popup = window.open(authUrl, 'lichess_oauth', 'width=600,height=700');
      if (!popup) {
        alert('Popup blocked — please allow popups for this site and try again.');
      }
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to start Lichess link');
    } finally {
      setLinking(false);
    }
  };

  // Already linked — show ratings card
  if (lichessUsername) {
    return (
      <div
        style={{
          padding: '12px 16px',
          borderRadius: 8,
          background: 'rgba(20, 241, 149, 0.05)',
          border: '1px solid rgba(20, 241, 149, 0.2)',
          marginTop: 16,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
          <span style={{ color: '#14f195' }}>
            <LichessIcon />
          </span>
          <span style={{ fontWeight: 700, color: '#14f195', fontSize: '0.95rem' }}>
            {lichessUsername}
          </span>
          {lichessVerified && (
            <span
              style={{
                fontSize: '0.7rem',
                background: 'rgba(20,241,149,0.12)',
                color: '#14f195',
                padding: '2px 8px',
                borderRadius: 10,
                border: '1px solid rgba(20,241,149,0.3)',
              }}
            >
              Verified
            </span>
          )}
        </div>
        <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap' }}>
          {lichessRapid !== undefined && lichessRapid > 0 && (
            <span style={{ fontSize: '0.8rem', color: 'var(--text-dim)' }}>
              Rapid{' '}
              <strong style={{ color: '#fff' }}>{Math.round(lichessRapid / 100)}</strong>
            </span>
          )}
          {lichessBlitz !== undefined && lichessBlitz > 0 && (
            <span style={{ fontSize: '0.8rem', color: 'var(--text-dim)' }}>
              Blitz{' '}
              <strong style={{ color: '#fff' }}>{Math.round(lichessBlitz / 100)}</strong>
            </span>
          )}
          {lichessBullet !== undefined && lichessBullet > 0 && (
            <span style={{ fontSize: '0.8rem', color: 'var(--text-dim)' }}>
              Bullet{' '}
              <strong style={{ color: '#fff' }}>{Math.round(lichessBullet / 100)}</strong>
            </span>
          )}
        </div>
      </div>
    );
  }

  // Viewing another player who hasn't linked
  if (!walletPubkey) {
    return (
      <div style={{ marginTop: 16, textAlign: 'center' }}>
        <span style={{ fontSize: '0.8rem', color: 'var(--text-dim)' }}>Lichess not linked</span>
      </div>
    );
  }

  // Own profile, not yet linked
  return (
    <div style={{ marginTop: 16, textAlign: 'center' }}>
      <button
        onClick={handleLink}
        disabled={linking}
        className="btn btn-secondary"
        style={{ fontSize: '0.9rem', padding: '8px 16px' }}
      >
        {linking ? 'Opening…' : 'Link Lichess Account'}
      </button>
      <p style={{ fontSize: '0.75rem', color: 'var(--text-dim)', marginTop: 4 }}>
        Seed your ELO from your Lichess rating — no wallet signature needed
      </p>
    </div>
  );
}
