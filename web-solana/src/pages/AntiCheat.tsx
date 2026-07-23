import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';

const AntiCheatPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <SeoHead meta={PAGE_METADATA.antiCheat} />
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — FAIR PLAY & ANTI-CHEAT POLICY</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: April 2026</span>
                <span className="operator-info">XForceSolutions Ltd, registered in England and Wales</span>
              </div>
            </div>
          </div>

          <div className="legal-intro">
            <p>Every ranked and wagered game on XFChess is analysed automatically for integrity. This page explains what we check, how we decide, and what happens when a violation is found.</p>
          </div>

          <div className="legal-sections">

            <div className="legal-section">
              <h4>1. SCOPE</h4>
              <p>Anti-cheat analysis runs on <strong>all PvP and tournament games</strong> where wagers are involved. Casual, practice, and computer games are excluded.</p>
              <p>Analysis runs post-game on our signing server (Hetzner CX32, co-located with the session-key infrastructure) and does not affect the speed of the match or the payout process.</p>
            </div>

            <div className="legal-section">
              <h4>2. SIGNED MOVE LOG — THE TAMPER-PROOF RECORD</h4>
              <p>Every move made in a ranked game is cryptographically signed by our session-key server at the moment it is received. The signed record includes:</p>
              <ul className="tax-list">
                <li>The move in UCI notation and the resulting board position (FEN)</li>
                <li>The player's wallet public key</li>
                <li>A server-side timestamp accurate to the millisecond</li>
                <li>A cryptographic signature binding the above together</li>
              </ul>
              <p>These records are persisted to an append-only database before the move is submitted to the Solana Ephemeral Rollup. This gives us an independently verifiable audit log for every move in every ranked game. No player can credibly claim "I didn't make that move" or dispute the timing of their decisions.</p>
              <div className="legal-highlight">
                <p>The signed move log is the authoritative source of truth for all integrity reviews. It cannot be altered without invalidating the cryptographic signatures.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>3. HOW WE DETECT CHEATING</h4>
              <p>We compute a weighted suspicion score (0.0–1.0) from three independent signals. No single signal alone causes action. Forced moves and opening theory are excluded from all signal calculations.</p>

              <div className="legal-subsection">
                <h5>A. Move Timing Analysis (weight: 40%)</h5>
                <p>Human players take measurably longer on complex positions and move faster on simple or forced ones. Engine-assisted players exhibit abnormally consistent response times regardless of position difficulty.</p>
                <p>We use the server-side signing timestamps to measure the time between moves to the millisecond. Positions are classified by phase (opening, middlegame, endgame) and filtered by complexity: positions where the top two engine moves are within 20 cp (middlegame) or 40 cp (endgame) of each other are considered low-complexity and skipped. Only non-trivial, non-forced positions contribute to the timing signal.</p>
              </div>

              <div className="legal-subsection">
                <h5>B. Move Quality vs ELO Baseline (weight: 35%)</h5>
                <p>After each game, Stockfish analyses every non-trivial position at depth 18 and records centipawn loss (CPL) — the difference between the played move and the engine's best move. We then compare the player's observed CPL against an empirical CPL-by-ELO curve: a 1200-rated player is expected to have a much higher CPL than a 2200-rated one.</p>
                <p>A player whose CPL is significantly lower than the curve predicts for their ELO rating — in other words, someone playing far above their stated strength — contributes a strong signal. A genuine strong player consistently playing at their level does not.</p>
              </div>

              <div className="legal-subsection">
                <h5>C. Top-1 Engine Move Rate on Complex Positions (weight: 25%)</h5>
                <p>For each position that passes the complexity filter, we record whether the player chose Stockfish's top-rated move. Humans at any level occasionally find the best move, but consistently selecting the engine's first choice across many complex positions — particularly under time pressure — is statistically improbable.</p>
              </div>

              <div className="legal-highlight">
                <p>Exact thresholds are not published to prevent calibrated evasion. Opening moves and clearly forced sequences are excluded from all three signals.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>4. VERDICT THRESHOLDS</h4>
              <p>The three weighted signals are combined into a single score between 0.0 and 1.0:</p>
              <ul className="tax-list">
                <li><strong>Score below 0.60 → Clean.</strong> Payout released normally. No record against the account.</li>
                <li><strong>Score 0.60–0.79 → Under Review.</strong> Human moderator examines the report before payout is released. Target review time: 48 hours.</li>
                <li><strong>Score 0.80 or above → Flagged.</strong> Payout held. Priority human review. Player notified by email.</li>
              </ul>
              <div className="legal-highlight">
                <p>Nobody is banned automatically. Every flag requires a human decision.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>5. THE INTEGRITY REPORT</h4>
              <p>For every game that reaches the Review or Flagged threshold, our system generates a written report containing:</p>
              <ul className="tax-list">
                <li>Player identities, wallet addresses, and on-chain ELO ratings at the time of the game</li>
                <li>Wager amount and tournament context (if applicable)</li>
                <li>Per-move breakdown: move played, engine top move, CPL, thinking time, complexity classification</li>
                <li>Signal scores and the weighted total that triggered review</li>
                <li>The full game in PGN format for independent verification</li>
                <li>A reviewer checklist for the human moderator</li>
              </ul>
              <p>Reports are retained for the duration of any appeal period and for regulatory purposes thereafter.</p>
            </div>

            <div className="legal-section">
              <h4>6. OUTCOMES</h4>
              <p><strong>Clean review</strong> — payout released immediately. No record against the account.</p>
              <p><strong>Borderline / Review</strong> — account remains fully active. Payout held up to 48 hours while a moderator reviews the integrity report. If no violation is found, funds are released in full.</p>
              <p><strong>Confirmed violation</strong> — wager voided and returned to the opponent. Account suspended pending further review. Player notified with the specific findings.</p>
              <p><strong>Organised fraud or collusion</strong> — all associated accounts suspended, matter referred to relevant authorities where legally required.</p>
              <div className="legal-highlight">
                <p>All decisions are appealable within 14 days of notification. Appeals are reviewed by a moderator who was not involved in the original decision.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>7. WHAT IS NOT A VIOLATION</h4>
              <ul className="tax-list">
                <li>Opening preparation and study carried out outside matches — opening moves are excluded from all signals</li>
                <li>Playing very accurately — strong play at your established ELO level will not trigger the quality signal</li>
                <li>Accessibility input tools — notify support before use so the account is flagged accordingly</li>
                <li>Time scramble games — elevated CPL under severe time pressure is expected and accounted for</li>
                <li>Long forced sequences — positions with only one legal or clearly dominant move are excluded from analysis</li>
              </ul>
            </div>

            <div className="legal-section">
              <h4>8. APPEALS & REPORTING</h4>
              <p><strong>To appeal a decision:</strong> you have 14 days from the date of notification. Email <a href="mailto:fairplay@xfchess.com">fairplay@xfchess.com</a> with your wallet address and game ID. Target response time is 5 business days. Your appeal will be reviewed by a moderator who had no involvement in the original decision.</p>
              <p><strong>To report suspected cheating:</strong> use the flag option in your match history. Reports are anonymous to the reported player. Flagged games are queued for priority review regardless of their automated score.</p>
            </div>

            <div className="legal-section">
              <h4>9. CAPACITY & INFRASTRUCTURE</h4>
              <p>Anti-cheat analysis runs as a background worker process on the same Hetzner signing server that handles session keys. Stockfish analysis at depth 18 takes approximately 15 seconds of CPU time per game. The current configuration supports approximately 15,000 analysed games per day — well in excess of expected early-platform volume. Infrastructure scales with demand; the signing server's p99 response time is not affected by background analysis.</p>
            </div>

            <div className="legal-section">
              <h4>10. DISCLAIMER</h4>
              <p>XFChess is a pre-launch platform. Implementation details described here reflect our current design and are subject to change as the system is developed and refined. Nothing on this page constitutes legal advice.</p>
              <div className="legal-contact">
                <p><strong>Fair play enquiries:</strong> <a href="mailto:fairplay@xfchess.com">fairplay@xfchess.com</a></p>
                <p><strong>Legal queries:</strong> <a href="mailto:legal@xfchess.com">legal@xfchess.com</a></p>
              </div>
            </div>

          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default AntiCheatPage;
