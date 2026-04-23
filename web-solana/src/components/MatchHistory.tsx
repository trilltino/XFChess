import { useState, useEffect, useCallback } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import { AlertTriangle, CheckCircle2, Clock, Loader2, ExternalLink } from 'lucide-react';
import {
  getGameHistory,
  notifyDispute,
  getDisputeStatus,
  type GameHistoryRecord,
  type DisputeStatus,
} from '../lib/api';

const DISPUTE_WINDOW_SECS = 48 * 60 * 60;

// ── Dispute status badge ──────────────────────────────────────────────────────

function DisputeBadge({ gameId }: { gameId: number }) {
  const [status, setStatus] = useState<DisputeStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function poll() {
      try {
        const s = await getDisputeStatus(gameId);
        if (!cancelled) setStatus(s);
      } catch {
        /* no dispute exists */
      }
    }
    poll();
    const id = setInterval(poll, 15_000);
    return () => { cancelled = true; clearInterval(id); };
  }, [gameId]);

  if (!status) return null;

  const colour =
    status.status === 'resolved' ? '#14F195' :
    status.status === 'pending' ? '#FFB800' : '#aaa';

  const label =
    status.status === 'resolved'
      ? `Resolved: ${status.decision ?? ''}`
      : status.status === 'pending'
      ? 'Under Review'
      : 'Dismissed';

  return (
    <span
      style={{
        fontSize: '0.75rem',
        padding: '2px 8px',
        borderRadius: 12,
        background: `${colour}22`,
        color: colour,
        border: `1px solid ${colour}55`,
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
      }}
    >
      {status.status === 'pending' ? <Clock size={11} /> : <CheckCircle2 size={11} />}
      {label}
      {status.tx_sig && (
        <a
          href={`https://solscan.io/tx/${status.tx_sig}`}
          target="_blank"
          rel="noreferrer"
          style={{ color: colour, marginLeft: 2 }}
          onClick={(e) => e.stopPropagation()}
        >
          <ExternalLink size={11} />
        </a>
      )}
    </span>
  );
}

// ── Dispute modal ─────────────────────────────────────────────────────────────

interface DisputeModalProps {
  game: GameHistoryRecord;
  walletPubkey: string;
  onClose: () => void;
}

function DisputeModal({ game, walletPubkey, onClose }: DisputeModalProps) {
  const { sendTransaction } = useWallet();
  const { connection } = useConnection();
  const [reason, setReason] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [caseId, setCaseId] = useState<string | null>(null);

  const PROGRAM_ID = import.meta.env.VITE_PROGRAM_ID ?? 'C624Z53FYEVDYVkMWSQ1KPQm4o1Jmdhpc5movSSBnezf';

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!reason.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const programId = new PublicKey(PROGRAM_ID);
      const gameIdNum = BigInt(game.id);
      const gameIdBytes = new Uint8Array(8);
      new DataView(gameIdBytes.buffer).setBigUint64(0, gameIdNum, true);

      const [gamePda] = PublicKey.findProgramAddressSync([Buffer.from('game'), gameIdBytes], programId);
      const [disputePda] = PublicKey.findProgramAddressSync([Buffer.from('dispute'), gameIdBytes], programId);

      // Anchor discriminator for dispute_game: sha256("global:dispute_game")[0..8]
      const disc = new Uint8Array([0x47, 0xd0, 0x8b, 0xb0, 0xe8, 0x4d, 0x2a, 0xc5]);

      const reasonBytes = new TextEncoder().encode(reason);
      const evidenceHash = new Uint8Array(32); // zero hash — VPS fills this in on resolution

      const data = new Uint8Array([
        ...disc,
        ...gameIdBytes,
        ...new Uint8Array(new Uint32Array([reasonBytes.length]).buffer),
        ...reasonBytes,
        ...evidenceHash,
      ]);

      const ix = new TransactionInstruction({
        programId,
        keys: [
          { pubkey: gamePda, isSigner: false, isWritable: true },
          { pubkey: disputePda, isSigner: false, isWritable: true },
          { pubkey: new PublicKey(walletPubkey), isSigner: true, isWritable: true },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data: Buffer.from(data),
      });

      const tx = new Transaction().add(ix);
      const sig = await sendTransaction(tx, connection);
      await connection.confirmTransaction(sig, 'confirmed');

      const resp = await notifyDispute({
        game_id: Number(game.id),
        challenger_wallet: walletPubkey,
        reason,
        tx_signature: sig,
      });

      setCaseId(resp.case_id);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Transaction failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.7)',
        display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        style={{
          background: 'var(--card-bg, #1a1a2e)', border: '1px solid var(--border)',
          borderRadius: 12, padding: 32, maxWidth: 480, width: '90%',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {caseId ? (
          <div style={{ textAlign: 'center' }}>
            <CheckCircle2 size={48} color="#14F195" style={{ margin: '0 auto 16px' }} />
            <h3 style={{ fontSize: '1.3rem', fontWeight: 800, marginBottom: 8 }}>Dispute Submitted</h3>
            <p style={{ color: 'var(--text-dim)', marginBottom: 8 }}>Case ID: <strong style={{ color: '#14F195' }}>{caseId}</strong></p>
            <p style={{ color: 'var(--text-dim)', fontSize: '0.9rem', marginBottom: 24 }}>
              The moderator has been emailed. You will be notified when a decision is made.
            </p>
            <button className="btn btn-primary" onClick={onClose}>Close</button>
          </div>
        ) : (
          <form onSubmit={handleSubmit}>
            <h3 style={{ fontSize: '1.2rem', fontWeight: 800, marginBottom: 4 }}>Raise a Dispute</h3>
            <p style={{ color: 'var(--text-dim)', fontSize: '0.85rem', marginBottom: 20 }}>
              Game #{game.id} — escrow will be frozen on-chain until a moderator reviews.
            </p>
            <textarea
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Describe the issue (e.g. engine assistance, disconnect abuse, no-show…)"
              rows={4}
              required
              maxLength={200}
              style={{
                width: '100%', padding: '12px 14px', borderRadius: 8,
                border: '1px solid var(--border)', background: 'var(--glass)',
                color: '#fff', fontSize: '0.95rem', resize: 'vertical',
                boxSizing: 'border-box',
              }}
            />
            <p style={{ fontSize: '0.75rem', color: 'var(--text-dim)', marginTop: 4, marginBottom: 16 }}>
              {reason.length}/200
            </p>
            {error && (
              <div style={{ color: '#ff8080', fontSize: '0.85rem', marginBottom: 12 }}>
                <AlertTriangle size={14} style={{ display: 'inline', marginRight: 4 }} />
                {error}
              </div>
            )}
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                type="submit"
                className="btn btn-primary"
                disabled={loading || !reason.trim()}
                style={{ flex: 1 }}
              >
                {loading ? <Loader2 size={16} className="spinner" /> : 'Submit Dispute'}
              </button>
              <button type="button" className="btn" onClick={onClose} style={{ flex: 1 }}>
                Cancel
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}

// ── Main MatchHistory component ───────────────────────────────────────────────

interface MatchHistoryProps {
  wallet: string;
}

export function MatchHistory({ wallet }: MatchHistoryProps) {
  const [games, setGames] = useState<GameHistoryRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedGame, setSelectedGame] = useState<GameHistoryRecord | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const { games: g } = await getGameHistory(wallet);
      setGames(g);
    } catch {
      setGames([]);
    } finally {
      setLoading(false);
    }
  }, [wallet]);

  useEffect(() => { refresh(); }, [refresh]);

  const now = Math.floor(Date.now() / 1000);

  const canDispute = (g: GameHistoryRecord): boolean => {
    if (g.status !== 'completed') return false;
    if (!g.end_time) return false;
    return now - g.end_time < DISPUTE_WINDOW_SECS;
  };

  const formatDate = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: '2-digit' });

  const resultLabel = (g: GameHistoryRecord) => {
    if (!g.winner) return 'Draw';
    if (g.winner === wallet) return 'Win';
    return 'Loss';
  };

  const resultColor = (g: GameHistoryRecord) => {
    if (!g.winner) return '#aaa';
    return g.winner === wallet ? '#14F195' : '#ff8080';
  };

  const opponentDisplay = (g: GameHistoryRecord) => {
    const isWhite = g.player_white === wallet;
    const opp = isWhite ? g.player_black : g.player_white;
    const oppName = isWhite ? g.black_username : g.white_username;
    if (!opp) return '—';
    return oppName || `${opp.slice(0, 4)}…${opp.slice(-4)}`;
  };

  if (loading) {
    return (
      <div style={{ textAlign: 'center', padding: 32 }}>
        <Loader2 className="spinner" size={24} style={{ color: 'var(--primary)' }} />
      </div>
    );
  }

  if (!games.length) {
    return (
      <div style={{ textAlign: 'center', padding: 24, color: 'var(--text-dim)', fontSize: '0.9rem' }}>
        No games recorded yet.
      </div>
    );
  }

  return (
    <div style={{ maxWidth: 700, margin: '0 auto' }}>
      <h3 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: 16, textAlign: 'center' }}>
        Match History
      </h3>
      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.9rem' }}>
          <thead>
            <tr style={{ color: 'var(--text-dim)', borderBottom: '1px solid var(--border)' }}>
              {['Date', 'Opponent', 'Result', 'Wager', 'Dispute'].map((h) => (
                <th key={h} style={{ padding: '8px 12px', textAlign: 'left', fontWeight: 600 }}>{h}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {games.map((g) => (
              <tr
                key={g.id}
                style={{ borderBottom: '1px solid rgba(255,255,255,0.06)' }}
              >
                <td style={{ padding: '10px 12px', color: 'var(--text-dim)' }}>
                  {g.start_time ? formatDate(g.start_time) : '—'}
                </td>
                <td style={{ padding: '10px 12px' }}>{opponentDisplay(g)}</td>
                <td style={{ padding: '10px 12px', fontWeight: 700, color: resultColor(g) }}>
                  {resultLabel(g)}
                </td>
                <td style={{ padding: '10px 12px' }}>
                  {g.stake_amount > 0 ? `${g.stake_amount} SOL` : 'Free'}
                </td>
                <td style={{ padding: '10px 12px' }}>
                  <DisputeBadge gameId={Number(g.id)} />
                  {canDispute(g) && (
                    <button
                      className="btn-small"
                      style={{ marginLeft: 8, fontSize: '0.75rem' }}
                      onClick={() => setSelectedGame(g)}
                    >
                      <AlertTriangle size={11} style={{ display: 'inline', marginRight: 3 }} />
                      Raise
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {selectedGame && (
        <DisputeModal
          game={selectedGame}
          walletPubkey={wallet}
          onClose={() => { setSelectedGame(null); refresh(); }}
        />
      )}
    </div>
  );
}
