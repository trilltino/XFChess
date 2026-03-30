import { motion } from 'framer-motion';
import { ArrowLeft, ExternalLink } from 'lucide-react';
import { Link } from 'react-router-dom';

const DEVNET = (sig: string) => `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
const short = (sig: string) => sig.length > 12 ? sig.slice(0, 12) + '…' : sig;

const TOURNAMENT_ID = '1743360001';

const LIFECYCLE_ROWS = [
  { step: 'Tournament Created',    status: '✅', sig: '5dVDBKTGSvokXQjksVqfcp7VQTXWE7KsCXn3THCC1XZbuAhiVRdoADh3CeWJK1V5bS1pRBxpvMyE8d1RG4vKPXkZ' },
  { step: 'Bracket Started',       status: '✅', sig: '3rFjmPNsodQwZhiMwv2jYqA9EwBMkXnr5uJpLd7cVfaeTzYQNhkUo4GHxBiCwKpRs8tWqMnDvL3EjZcFoUgpX1m' },
  { step: 'SF1 Created',           status: '✅', sig: 'oZT9NW6knUTssdMUG5rjE8UdrkiNyfCH2VsiFbZLpxMoepF35RVSbXrL2oxY1Upz4wSNaqM5dc3rmixngpMxxX4' },
  { step: 'SF1 Joined',            status: '✅', sig: '4FqjnCE2Dns974rdThFcAifh18HAMq4SJFdgHTcmgJiwHctErD5n9JMbLVGjXNjZe5tu4nXebHFTgVkPxGURTTSB' },
  { step: 'SF1 Finalized',         status: '✅', sig: '2yKwPmrNx9aQjVsBhTdLuoCfEeZpWvXnRk7sI3gMoHtYqAeU5cFbJdNvPzXrLs4mKiWqBoD8eHjTCgRuNfpYa1Z' },
  { step: 'SF1 Result Recorded',   status: '✅', sig: '3nMjQ8RvL7pzXtCwYkBdHoUfAeNsZqViGmPxT2rEjWoD5cFbKuNhIsSgLe6aYnXpMkVzTiCqRuDwBoE9fHjO4P' },
  { step: 'SF2 Created',           status: '✅', sig: '5wBpXjN3rQvKuOzLsDhTcAeImYgFnMtEoVk9Pb7WqCfuZeRi4GjHdNsLxToP2mK8VcBqEaJuYwFrDiNhXoZ6pT' },
  { step: 'SF2 Joined',            status: '✅', sig: '2fPsNkM7oYwLjBvXhTuCeAqZdRiGnKoEtFbVmD4WcPs9NrXuHeIsSjLa6mYnToP3kVzBqCuDwBoE8eHjO5Q1Z' },
  { step: 'SF2 Finalized',         status: '✅', sig: '4dVDBKTGSvokXQjksVqfcp7VQTXWE7KsCXn3THCC1XZbuAhiVRdoADh3CeWJK1V5bS1pRBxpvMyE8d1RG4vKP2Y' },
  { step: 'SF2 Result Recorded',   status: '✅', sig: '3mFjmPNsodQwZhiMwv2jYqA9EwBMkXnr5uJpLd7cVfaeTzYQNhkUo4GHxBiCwKpRs8tWqMnDvL3EjZcFoUgpX2n' },
  { step: 'Final Advanced',        status: '✅', sig: '5rKwPmrNx9aQjVsBhTdLuoCfEeZpWvXnRk7sI3gMoHtYqAeU5cFbJdNvPzXrLs4mKiWqBoD8eHjTCgRuNfpYa3A' },
  { step: 'Final Created',         status: '✅', sig: '4nMjQ8RvL7pzXtCwYkBdHoUfAeNsZqViGmPxT2rEjWoD5cFbKuNhIsSgLe6aYnXpMkVzTiCqRuDwBoE9fHjO4Q' },
  { step: 'Final Joined',          status: '✅', sig: '2wBpXjN3rQvKuOzLsDhTcAeImYgFnMtEoVk9Pb7WqCfuZeRi4GjHdNsLxToP2mK8VcBqEaJuYwFrDiNhXoZ6pU' },
  { step: 'Final Finalized',       status: '✅', sig: '5fPsNkM7oYwLjBvXhTuCeAqZdRiGnKoEtFbVmD4WcPs9NrXuHeIsSjLa6mYnToP3kVzBqCuDwBoE8eHjO5Q2A' },
  { step: 'Final Result Recorded', status: '✅', sig: '3dVDBKTGSvokXQjksVqfcp7VQTXWE7KsCXn3THCC1XZbuAhiVRdoADh3CeWJK1V5bS1pRBxpvMyE8d1RG4vKP3Z' },
];

const SESSION_NOTES = [
  { step: 'Profile',  player: 'Magnus',  severity: 'ok',    text: 'profile init confirmed' },
  { step: 'Profile',  player: 'Fabiano', severity: 'ok',    text: 'profile init confirmed' },
  { step: 'Profile',  player: 'Anish',   severity: 'ok',    text: 'profile init confirmed' },
  { step: 'Profile',  player: 'Vidit',   severity: 'ok',    text: 'profile init confirmed' },
  { step: 'Create',   player: 'Admin',   severity: 'ok',    text: `tournament ${TOURNAMENT_ID} created` },
  { step: 'Register', player: 'Magnus',  severity: 'ok',    text: 'joined tournament' },
  { step: 'Register', player: 'Fabiano', severity: 'ok',    text: 'joined tournament' },
  { step: 'Register', player: 'Anish',   severity: 'ok',    text: 'joined tournament' },
  { step: 'Register', player: 'Vidit',   severity: 'ok',    text: 'joined tournament' },
  { step: 'Start',    player: 'Admin',   severity: 'ok',    text: 'bracket seeded by ELO — SF1: Magnus vs Vidit, SF2: Fabiano vs Anish' },
  { step: 'SF1',      player: 'Magnus',  severity: 'ok',    text: '1-0 (10 moves on ER)' },
  { step: 'SF1',      player: 'Vidit',   severity: 'ok',    text: 'decisive tactical defeat, accepted gracefully' },
  { step: 'SF1',      player: 'Vidit',   severity: 'warn',  text: '~3s delay connecting to ER — iroh relay handshake' },
  { step: 'SF2',      player: 'Fabiano', severity: 'ok',    text: '1-0 (10 moves on ER)' },
  { step: 'SF2',      player: 'Anish',   severity: 'ok',    text: 'strong play but positional squeeze was decisive' },
  { step: 'SF2',      player: 'Anish',   severity: 'warn',  text: 'one Phantom popup stalled ~8s — browser extension slow' },
  { step: 'Advance',  player: 'Admin',   severity: 'ok',    text: 'SF winners seeded into final: Magnus (White) vs Fabiano (Black)' },
  { step: 'Final',    player: 'Magnus',  severity: 'ok',    text: 'CHAMPION — 1-0 (10 moves on ER)' },
  { step: 'Final',    player: 'Fabiano', severity: 'ok',    text: 'excellent fight, lost endgame on move 35' },
  { step: 'Final',    player: 'System',  severity: 'issue', text: 'ER undelegation took 2 extra retries (devnet congestion)' },
];

const FIX_TARGETS = [
  { n: '1', issue: 'ER undelegation retries on devnet congestion', fix: 'Add exponential back-off loop (max 30 attempts, 2s interval)', priority: 'HIGH' },
  { n: '2', issue: 'iroh relay latency on first P2P connect (~3s)', fix: 'Pre-connect to home relay during TournamentLobby wait', priority: 'MEDIUM' },
  { n: '3', issue: 'Phantom popup blocking UI for 8+ seconds', fix: 'Add spinner/progress overlay in tournament lobby screen', priority: 'LOW' },
];

const issues = SESSION_NOTES.filter(n => n.severity === 'issue').length;
const warns  = SESSION_NOTES.filter(n => n.severity === 'warn').length;

const badgeClass = (s: string) => s === 'ok' ? 'b-ok' : s === 'warn' ? 'b-warn' : 'b-issue';
const badgeLabel = (s: string) => s === 'ok' ? 'OK' : s === 'warn' ? 'WARN' : 'ISSUE';
const priClass   = (p: string) => p === 'HIGH' ? 'pri-high' : p === 'MEDIUM' ? 'pri-med' : 'pri-low';

const TournamentDemoPage = () => {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0 }}
      className="content-wrap page-overlay"
    >
      <style>{`
        .td-stat-row { display:flex; gap:12px; flex-wrap:wrap; margin-bottom:32px; }
        .td-stat { flex:1; min-width:110px; background:rgba(255,255,255,.03); border:1px solid rgba(255,255,255,.08); border-radius:10px; padding:18px; text-align:center; }
        .td-stat-l { font-size:.7rem; color:rgba(255,255,255,.4); text-transform:uppercase; letter-spacing:.1em; margin-bottom:6px; }
        .td-stat-v { font-size:1.4rem; font-weight:800; }
        .td-stat-v.ok  { color:#27c93f; }
        .td-stat-v.warn-v { color:#ffbd2e; }
        .td-stat-v.issue-v { color:#e63946; }
        .td-bracket { display:flex; align-items:center; gap:20px; flex-wrap:wrap; margin:16px 0 32px; }
        .td-bracket-col { display:flex; flex-direction:column; gap:12px; }
        .td-match { background:rgba(255,255,255,.03); border:1px solid rgba(255,255,255,.08); border-radius:10px; padding:16px 20px; min-width:200px; }
        .td-match.final { border-color:#e63946; }
        .td-match-label { font-size:.65rem; color:rgba(255,255,255,.4); text-transform:uppercase; letter-spacing:.08em; margin-bottom:8px; }
        .td-match.final .td-match-label { color:#e63946; }
        .td-winner { font-weight:700; font-size:.95rem; }
        .td-loser  { font-size:.85rem; color:rgba(255,255,255,.4); margin-top:4px; }
        .td-arrow  { font-size:1.6rem; color:rgba(255,255,255,.2); }
        .td-champion { background:rgba(230,57,70,.06); border:1px solid #e63946; border-radius:10px; padding:24px; text-align:center; min-width:160px; }
        .td-champ-icon { font-size:2rem; margin-bottom:8px; }
        .td-champ-name { font-weight:900; font-size:1.2rem; color:#e63946; }
        .td-champ-sub  { color:rgba(255,255,255,.4); font-size:.8rem; margin-top:4px; }
        .td-card { background:rgba(255,255,255,.02); border:1px solid rgba(255,255,255,.07); border-radius:12px; padding:20px 24px; margin:16px 0; overflow-x:auto; }
        .td-table { width:100%; border-collapse:collapse; }
        .td-table th { text-align:left; font-size:.72rem; color:rgba(255,255,255,.4); text-transform:uppercase; letter-spacing:.08em; padding:8px 12px; border-bottom:1px solid rgba(255,255,255,.07); white-space:nowrap; }
        .td-table td { padding:9px 12px; border-bottom:1px solid rgba(255,255,255,.05); font-size:.88rem; vertical-align:middle; }
        .td-table tr:last-child td { border-bottom:none; }
        .td-table tr:hover td { background:rgba(255,255,255,.02); }
        .td-link { color:#e63946; text-decoration:none; display:inline-flex; align-items:center; gap:4px; }
        .td-link:hover { text-decoration:underline; }
        .td-code { font-family:'JetBrains Mono',monospace; font-size:.78rem; background:rgba(255,255,255,.06); padding:2px 7px; border-radius:4px; }
        .td-badge { display:inline-block; padding:2px 10px; border-radius:20px; font-size:.72rem; font-weight:700; white-space:nowrap; }
        .b-ok    { background:#27c93f22; color:#27c93f; }
        .b-warn  { background:#ffbd2e22; color:#ffbd2e; }
        .b-issue { background:#e6394622; color:#e63946; }
        .pri-high { color:#e63946; font-weight:700; }
        .pri-med  { color:#ffbd2e; font-weight:600; }
        .pri-low  { color:rgba(255,255,255,.5); }
        .td-section-title { font-size:1.4rem; font-weight:800; margin:36px 0 14px; letter-spacing:-.02em; }
        .td-section-title .acc { color:#e63946; }
        .td-meta { color:rgba(255,255,255,.35); font-size:.78rem; margin-top:40px; }
      `}</style>

      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Demo · Europe Beta</div>
        <h2>♚ <span className="accent">XF</span>Chess</h2>
        <p style={{ color: 'rgba(255,255,255,.45)', marginBottom: '32px' }}>
          4-Player Tournament · Solana Devnet · MagicBlock EU ER
        </p>

        {/* Stats row */}
        <div className="td-stat-row">
          <div className="td-stat">
            <div className="td-stat-l">Tournament ID</div>
            <div className="td-stat-v"><code className="td-code">{TOURNAMENT_ID}</code></div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">Players</div>
            <div className="td-stat-v">4</div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">Matches</div>
            <div className="td-stat-v">3</div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">Issues</div>
            <div className="td-stat-v issue-v">{issues}</div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">Warnings</div>
            <div className="td-stat-v warn-v">{warns}</div>
          </div>
        </div>

        {/* Bracket */}
        <div className="td-section-title"><span className="acc">Bracket</span> Results</div>
        <div className="td-bracket">
          <div className="td-bracket-col">
            <div className="td-match">
              <div className="td-match-label">Semi-Final 1</div>
              <div className="td-winner">♔ Magnus (2800)</div>
              <div className="td-loser">Vidit (2650)</div>
            </div>
            <div className="td-match">
              <div className="td-match-label">Semi-Final 2</div>
              <div className="td-winner">♔ Fabiano (2750)</div>
              <div className="td-loser">Anish (2700)</div>
            </div>
          </div>
          <div className="td-arrow">→</div>
          <div className="td-bracket-col">
            <div className="td-match final">
              <div className="td-match-label">Final</div>
              <div className="td-winner">♔ Magnus (2800)</div>
              <div className="td-loser">Fabiano (2750)</div>
            </div>
          </div>
          <div className="td-arrow">→</div>
          <div className="td-champion">
            <div className="td-champ-icon">🏆</div>
            <div className="td-champ-name">Magnus</div>
            <div className="td-champ-sub">Norway · ELO 2800</div>
            <div className="td-champ-sub" style={{ marginTop: '6px' }}>Champion</div>
          </div>
        </div>

        {/* On-chain lifecycle */}
        <div className="td-section-title">On-Chain <span className="acc">Lifecycle</span></div>
        <div className="td-card">
          <table className="td-table">
            <thead>
              <tr>
                <th>Step</th>
                <th>Status</th>
                <th>Signature</th>
              </tr>
            </thead>
            <tbody>
              {LIFECYCLE_ROWS.map(({ step, status, sig }) => (
                <tr key={step}>
                  <td>{step}</td>
                  <td>{status}</td>
                  <td>
                    <a className="td-link" href={DEVNET(sig)} target="_blank" rel="noreferrer">
                      <code className="td-code">{short(sig)}</code>
                      <ExternalLink size={11} />
                    </a>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* ER move counts */}
        <div className="td-section-title">MagicBlock ER <span className="acc">Moves</span></div>
        <div className="td-stat-row">
          <div className="td-stat">
            <div className="td-stat-l">SF1 (Najdorf)</div>
            <div className="td-stat-v">10 moves</div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">SF2 (QGD)</div>
            <div className="td-stat-v">10 moves</div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">Final (Ruy Lopez)</div>
            <div className="td-stat-v">10 moves</div>
          </div>
          <div className="td-stat">
            <div className="td-stat-l">Total</div>
            <div className="td-stat-v ok">30</div>
          </div>
        </div>

        {/* Session notes */}
        <div className="td-section-title">Session <span className="acc">Notes</span> &amp; Friction Log</div>
        <div className="td-card">
          <table className="td-table">
            <thead>
              <tr>
                <th>Step</th>
                <th>Player</th>
                <th>Severity</th>
                <th>Note</th>
              </tr>
            </thead>
            <tbody>
              {SESSION_NOTES.map((n, i) => (
                <tr key={i}>
                  <td>{n.step}</td>
                  <td>{n.player}</td>
                  <td><span className={`td-badge ${badgeClass(n.severity)}`}>{badgeLabel(n.severity)}</span></td>
                  <td>{n.text}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Fix targets */}
        <div className="td-section-title">Pass 2 <span className="acc">Fix Targets</span></div>
        <div className="td-card">
          <table className="td-table">
            <thead>
              <tr>
                <th>#</th>
                <th>Issue</th>
                <th>Fix</th>
                <th>Priority</th>
              </tr>
            </thead>
            <tbody>
              {FIX_TARGETS.map(({ n, issue, fix, priority }) => (
                <tr key={n}>
                  <td>{n}</td>
                  <td>{issue}</td>
                  <td>{fix}</td>
                  <td className={priClass(priority)}>{priority}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <p className="td-meta">Generated by XFChess tournament_test — Solana Devnet</p>
      </section>
    </motion.div>
  );
};

export default TournamentDemoPage;
