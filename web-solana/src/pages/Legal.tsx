import { motion } from 'framer-motion';
import { ArrowLeft, AlertTriangle } from 'lucide-react';
import { Link } from 'react-router-dom';

const LegalPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — LEGAL & REGULATORY COMPLIANCE</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: April 2026</span>
                <span className="operator-info">XFChess is operated by XForceSolutions Ltd, registered in England and Wales</span>
              </div>
            </div>
            <div className="legal-disclaimer">
              <AlertTriangle size={20} color="#f59e0b" />
              <div>
                <p><strong>NOTE:</strong> XFChess is a pre-launch platform currently undergoing regulatory and legal review. The positions described on this page represent our current good-faith interpretation of applicable law and our intended compliance roadmap. We are in the process of engaging qualified legal, tax, and regulatory advisors. This page will be updated as those processes conclude.</p>
              </div>
            </div>
          </div>

          <div className="legal-intro">
            <p>This page sets out XFChess's current understanding of its legal and regulatory position and its intended approach to compliance as the platform develops.</p>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>1. NATURE OF THE SERVICE — PRIZE COMPETITION, NOT GAMBLING</h4>
              <p>It is our understanding that XFChess operates as a prize competition rather than a gambling product.</p>

              <div className="legal-subsection">
                <p>Under the Gambling Act 2005, a product is only classified as gambling where the outcome is determined wholly or substantially by chance. Chess is a game of pure skill, recognised as such internationally. Under section 14 of the Gambling Act 2005, competitions whose results are determined by skill, judgment, or knowledge fall outside the gambling regime, provided the skill element is sufficient to prevent a significant proportion of participants from winning. We believe chess clearly satisfies this standard.</p>
              </div>

              <div className="legal-subsection">
                <p>XFChess operates on a peer-to-peer basis. Players wager directly against one another. XFChess does not act as a bookmaker, does not hold or benefit from wager funds, and does not operate a house-edge model. Our intended revenue model is a fixed platform fee per game.</p>
              </div>

              <div className="legal-highlight">
                <p>We are seeking formal legal advice to confirm this classification prior to launch. If that advice indicates a Gambling Operating Licence is required, we will not launch the wagering functionality until one is obtained.</p>
              </div>

              <div className="legal-subsection">
                <p>To preserve the skill-game classification, XFChess does not intend to introduce randomised gameplay elements, random matchmaking based on chance, or luck-based mechanics. Any such change would be subject to legal review before implementation.</p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default LegalPage;
