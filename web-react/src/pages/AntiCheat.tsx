import { motion } from 'framer-motion';
import { ArrowLeft, AlertTriangle, Users, Bot, Clock, Eye, CheckCircle } from 'lucide-react';
import { Link } from 'react-router-dom';

const AntiCheatPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — FAIR PLAY & ANTI-CHEAT</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: March 2026</span>
                <span className="operator-info">XFChess is operated by XForcesolutions LLC, registered in England and Wales</span>
              </div>
            </div>
            <div className="legal-disclaimer">
              <AlertTriangle size={20} color="#f59e0b" />
              <div>
                <p><strong>IMPORTANT:</strong> XFChess is a real-money skill competition. The integrity of every match is the foundation of the platform. This page explains how we protect honest players, how we handle suspected violations, and what you can expect if a Fair Play review is opened on your account.</p>
              </div>
            </div>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>1. THE HUMAN ARENA AND THE BOT ARENA</h4>
              <p>XFChess operates two distinct competitive environments:</p>
              
              <div className="arena-comparison">
                <div className="arena-card human">
                  <div className="arena-header">
                    <Users size={32} color="#e63946" />
                    <h5>Human Arena</h5>
                  </div>
                  <p>For human players wagering against other humans. Engine assistance of any kind is strictly prohibited. Violations are treated as cheating under our Terms of Service and, where applicable, under Section 42 of the Gambling Act 2005.</p>
                </div>

                <div className="arena-card bot">
                  <div className="arena-header">
                    <Bot size={32} color="#e63946" />
                    <h5>Bot Arena</h5>
                  </div>
                  <p>A dedicated environment where automated players and assisted play are permitted and expected. Bot Arena participants are not subject to the same engine-detection rules as Human Arena participants.</p>
                </div>
              </div>

              <div className="legal-highlight">
                <p>The goal of our Fair Play system is not to eliminate bots from XFChess entirely — it is to ensure bots never compete in the Human Arena.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>2. HOW WE DETECT CHEATING</h4>
              <p>We use a multi-layered detection system. No single signal is treated as conclusive. Our system produces a probabilistic suspicion score for each player across each game. A review is only escalated when multiple independent signals align.</p>

              <div className="legal-subsection">
                <p>We do not publish the precise thresholds or weightings of our detection model. Doing so would provide a roadmap for evasion. What follows is an honest description of the categories of analysis we perform.</p>
              </div>

              <div className="detection-categories">
                <div className="detection-category">
                  <h5>A. Game Quality Analysis</h5>
                  <p>Every move you play is evaluated against engine-optimal play. We measure:</p>
                  <ul className="detection-list">
                    <li>How closely your moves align with the top choices of a reference engine across the full game</li>
                    <li>Your average accuracy loss per move, weighted by position complexity — high accuracy in a forced sequence is normal; consistently finding the only winning move in chaotic, branching positions is not</li>
                    <li>Whether your accuracy profile is consistent with your account's historical skill level and ELO rating</li>
                  </ul>
                  <p>Human players, regardless of strength, display natural variation in move quality. A statistical profile that is implausibly consistent — or that suddenly spikes far above a player's established baseline — is a meaningful signal.</p>
                </div>

                <div className="detection-category">
                  <h5>B. Timing Analysis</h5>
                  <p>Human players think. They spend more time on hard moves and less on obvious ones. Our system analyses:</p>
                  <ul className="detection-list">
                    <li>The distribution of your thinking time across move types</li>
                    <li>Whether your response times show the natural variance humans exhibit, including "spikes" on critical decisions</li>
                    <li>Patterns that suggest moves are being generated externally and fed to the client at a fixed interval</li>
                  </ul>
                </div>

                <div className="detection-category">
                  <h5>C. Input and Interaction Analysis</h5>
                  <p>How you interact with the board matters as well as what moves you play. We analyse interaction patterns to distinguish human motor behaviour from programmatic input. We do not disclose the specific signals we use here, as this layer is most sensitive to evasion if detailed publicly.</p>
                </div>

                <div className="detection-category">
                  <h5>D. Environment Signals</h5>
                  <p>For high-stakes Human Arena games, we perform additional environment checks intended to detect:</p>
                  <ul className="detection-list">
                    <li>Indicators of automated or virtualised environments inconsistent with normal human play</li>
                    <li>Multiple accounts sharing common hardware or session characteristics ("Sybil" patterns)</li>
                    <li>Concurrent screen-capture or overlay software that may be used to relay board state to an external engine</li>
                  </ul>
                </div>

                <div className="detection-category">
                  <h5>E. Account-Level Patterns</h5>
                  <p>We look at behaviour across games, not just within a single game. A player whose profile shifts dramatically and suddenly — particularly around wager size increases — will receive closer attention. Patterns consistent with a single operator running multiple accounts to extract funds are treated as a serious violation.</p>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>3. THE FAIR PLAY REVIEW PROCESS</h4>
              <p>Payouts from wagered matches are subject to a Fair Play Review window of up to 48 hours before SOL is released to the winner's wallet.</p>

              <div className="review-process">
                <div className="process-step">
                  <div className="step-icon">
                    <Clock size={24} color="#e63946" />
                  </div>
                  <div className="step-content">
                    <h5>Automated Review</h5>
                    <p>In the majority of matches this review is automated and instant. For matches that generate elevated suspicion scores, our system flags the game for deeper analysis.</p>
                  </div>
                </div>

                <div className="process-step">
                  <div className="step-icon">
                    <Eye size={24} color="#e63946" />
                  </div>
                  <div className="step-content">
                    <h5>Human Review</h5>
                    <p>In rare cases a human reviewer will examine the game directly. If a review is opened on your account you will be notified by email. You will not lose access to your account during a standard review. Your funds will remain in the smart contract pending the outcome.</p>
                  </div>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>4. OUTCOMES OF A FAIR PLAY REVIEW</h4>
              <p>If a review concludes that no violation occurred, your payout is released immediately and your account standing is unaffected.</p>

              <p>If a review concludes that a violation is likely, the following outcomes may apply depending on severity:</p>

              <div className="outcomes-grid">
                <div className="outcome-card">
                  <h5>Migration to Bot Arena</h5>
                  <p>Where our model determines that a player's style is statistically inconsistent with human play across a sufficient sample, they will be moved to the Bot Arena. This is not a ban — it is a reclassification. Funds already won in games prior to detection are subject to review.</p>
                </div>

                <div className="outcome-card">
                  <h5>Wager Voiding</h5>
                  <p>Under Section 42 of the Gambling Act 2005, cheating in a prize competition is a criminal offence. Our Terms of Service permit XFChess to void the wager contract where engine use in the Human Arena is established. Voided funds are returned to the losing player.</p>
                </div>

                <div className="outcome-card">
                  <h5>Account Suspension</h5>
                  <p>Confirmed or repeated violations will result in account suspension and forfeiture of any applicable Fair Play Bond (see Section 5).</p>
                </div>

                <div className="outcome-card">
                  <h5>Referral</h5>
                  <p>Serious or organised violations — including coordinated multi-account fraud — may be referred to relevant authorities.</p>
                </div>
              </div>

              <p>All decisions are subject to our appeals process (see Section 6).</p>
            </div>

            <div className="legal-section">
              <h4>5. THE FAIR PLAY BOND</h4>
              <p>For Human Arena games above a threshold wager value [TO BE CONFIRMED], players are required to post a Fair Play Bond in addition to their wager.</p>

              <div className="bond-info">
                <p>The Bond is held in the same smart contract as the wager. It is returned in full to the player at the conclusion of a clean review. If engine use is confirmed, the Bond is slashed. The slashed amount is distributed as follows: a portion to the opponent as compensation, and a portion to a player protection reserve.</p>

                <div className="bond-purpose">
                  <h5>The Bond requirement serves two purposes:</h5>
                  <ul className="bond-list">
                    <li>It raises the cost of cheating</li>
                    <li>It funds the compensation of players who were cheated before detection occurred</li>
                  </ul>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>6. PROOF OF PERSONHOOD (HUMAN ARENA)</h4>
              <p>To enter the Human Arena, players must complete our identity verification process (see our KYC/AML page) and, for higher-stakes tiers, demonstrate Proof of Personhood via a recognised decentralised identity credential — such as a World ID-compatible verification or an equivalent reputation-linked wallet signal.</p>

              <div className="legal-highlight">
                <p>This requirement exists because statistical detection is retrospective. Proof of Personhood is a prospective gatekeeping layer: it raises the real-world cost of creating a fraudulent "human" account before the first game is played.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>7. WHAT IS NOT A VIOLATION</h4>
              <p>The following are explicitly permitted and will not result in a review escalation:</p>

              <div className="permitted-actions">
                <div className="permitted-item">
                  <CheckCircle size={20} color="#27c93f" />
                  <div>
                    <h5>Using opening books or studying games</h5>
                    <p>During your own time outside of a match</p>
                  </div>
                </div>

                <div className="permitted-item">
                  <CheckCircle size={20} color="#27c93f" />
                  <div>
                    <h5>Playing in the Bot Arena</h5>
                    <p>With engine assistance of any kind</p>
                  </div>
                </div>

                <div className="permitted-item">
                  <CheckCircle size={20} color="#27c93f" />
                  <div>
                    <h5>Using accessibility tools</h5>
                    <p>That assist with input (e.g., switch-access devices) — contact support before your first wagered game if you use such a tool, so it can be noted on your account</p>
                  </div>
                </div>

                <div className="permitted-item">
                  <CheckCircle size={20} color="#27c93f" />
                  <div>
                    <h5>Playing very accurately</h5>
                    <p>Because you are simply a strong player — our model accounts for your established ELO and skill history</p>
                  </div>
                </div>
              </div>

              <p>If you believe a review was opened on your account in error, please use the appeals process.</p>
            </div>

            <div className="legal-section">
              <h4>8. APPEALS</h4>
              <p>If you believe a Fair Play decision was made in error, you may submit an appeal within 14 days of the decision notification. Appeals are reviewed by a human member of the XFChess team who was not involved in the original review. We aim to respond to all appeals within 5 business days.</p>

              <div className="legal-contact">
                <p><strong>To submit an appeal:</strong> <a href="mailto:fairplay@xfchess.com">fairplay@xfchess.com</a></p>
              </div>
            </div>

            <div className="legal-section">
              <h4>9. REPORTING SUSPECTED CHEATING</h4>
              <p>If you suspect your opponent used engine assistance in a Human Arena game, you can flag the game for review directly from your match history. Flagged games are queued for manual analysis. We take all reports seriously and will notify you of the outcome.</p>

              <div className="legal-highlight">
                <p>Reports are anonymous to the reported player.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>10. DISCLAIMER</h4>
              <p>This page describes our Fair Play system as intended at the time of writing. XFChess is a pre-launch platform and specific thresholds, bond values, and process details will be confirmed and updated prior to launch. Nothing on this page constitutes legal advice.</p>

              <div className="legal-contact">
                <p><strong>For formal legal queries regarding our Fair Play policy:</strong> <a href="mailto:isicheivalentine@gmail.com">isicheivalentine@gmail.com</a></p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default AntiCheatPage;
