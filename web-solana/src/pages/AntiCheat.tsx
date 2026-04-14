import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';

const AntiCheatPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — FAIR PLAY & ANTI-CHEAT SUMMARY</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: April 2026</span>
                <span className="operator-info">XForceSolutions Ltd, registered in England and Wales</span>
              </div>
            </div>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>2. HOW WE DETECT CHEATING</h4>
              <p>No single signal triggers action. We build a probabilistic suspicion score across:</p>
              <ul className="tax-list">
                <li>Move quality vs engine-optimal play, weighted by position complexity</li>
                <li>Thinking time patterns (humans slow down on hard moves; bots don't)</li>
                <li>Input/interaction behaviour (how you move pieces, not just what you play)</li>
                <li>Environment checks for virtualised or automated clients</li>
                <li>Account-level patterns across games, especially around wager size changes</li>
              </ul>
              <div className="legal-highlight">
                <p>Exact thresholds are not published to prevent evasion.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>3. PAYOUT REVIEW</h4>
              <p>All payouts have a 48-hour Fair Play Review window before SOL is released.</p>
              <p>Most reviews are instant and automated. Elevated scores trigger human review.</p>
              <p>You keep account access and funds remain in the smart contract during review.</p>
            </div>

            <div className="legal-section">
              <h4>4. OUTCOMES</h4>
              <p><strong>Clean review</strong> — payout released, account unaffected.</p>
              <p><strong>Borderline</strong> — account flagged for review.</p>
              <p><strong>Confirmed cheat</strong> — wager voided (returned to opponent), account suspended.</p>
              <p><strong>Organised fraud</strong> — referral to relevant authorities.</p>
              <div className="legal-highlight">
                <p>All decisions are appealable within 14 days.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>5. NOT A VIOLATION</h4>
              <ul className="tax-list">
                <li>Opening prep and game study outside matches</li>
                <li>Accessibility input tools (notify support first)</li>
                <li>Simply playing very accurately</li>
              </ul>
            </div>

            <div className="legal-section">
              <h4>6. APPEALS & REPORTING</h4>
              <p><strong>Appeals:</strong> 14-day window, reviewed by someone not involved in the original decision. Target response: 5 business days. — <a href="mailto:fairplay@xfchess.com">fairplay@xfchess.com</a></p>
              <p><strong>Suspected cheating:</strong> flag directly from match history. Anonymous to the reported player.</p>
            </div>

            <div className="legal-section">
              <h4>7. DISCLAIMER</h4>
              <p>Pre-launch platform. Implementation details subject to change.</p>
              <p>Nothing on this page is legal advice.</p>
              <div className="legal-contact">
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
