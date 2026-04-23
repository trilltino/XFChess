import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';

const KycPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — IDENTITY VERIFICATION & KYC SUMMARY</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: April 2026</span>
                <span className="operator-info">XForceSolutions Ltd, registered in England and Wales</span>
              </div>
            </div>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>WHY THIS IS REQUIRED</h4>
              <p>XFChess requires identity verification for all players before any deposit or wagered match. This reflects both our legal assessment in progress (MLR 2017 / FCA cryptoasset regime SI 2026/102) and our own platform integrity standards. Regardless of final regulatory classification, we will not permit anonymous real-money play.</p>
            </div>

            <div className="legal-section">
              <h4>2. VERIFICATION REQUIREMENTS</h4>
              <p>Triggered on first deposit or first wagered match entry.</p>
              <p>Players must provide government-issued photo identification (passport, national ID, or driving licence) and complete identity verification before accessing any wagered features. Verification confirms the player is aged 18 or over and is not on any applicable sanctions list.</p>
            </div>

            <div className="legal-section">
              <h4>3. WHAT XFCHESS RECEIVES AND STORES</h4>
              <p><strong>Received:</strong> Verified status, 18+ confirmation, sanctions clear/match, full name, date of birth, country of residence</p>
              <p><strong>Not stored:</strong> Document images, raw biometric data, NFC chip data</p>
              <p>Document and biometric data stays with the verification provider. XFChess retains only the minimum required by UK AML law — 5 years from end of business relationship.</p>
            </div>

            <div className="legal-section">
              <h4>4. SANCTIONS & PEP SCREENING</h4>
              <p>All players screened at verification against:</p>
              <p>HM Treasury UK list, UN list, EU list, OFAC, and PEP databases.</p>
              <div className="legal-highlight">
                <p>A sanctions match = declined. Not subject to appeal through XFChess.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>5. TAX ID VERIFICATION</h4>
              <p>For compliance with CACF (Crypto-Asset Reporting Framework) requirements, players from certain jurisdictions must provide country-specific tax identification numbers:</p>
              <ul className="tax-list">
                <li><strong>United Kingdom:</strong> National Insurance Number (NI) — 2 letters + 6 digits + 1 letter (e.g., AB123456C)</li>
                <li><strong>Brazil:</strong> CPF (Cadastro de Pessoas Físicas) — 11 digits</li>
                <li><strong>Germany:</strong> Tax ID (Steueridentifikationsnummer) — 11 digits</li>
                <li><strong>Canada:</strong> Social Insurance Number (SIN) — 9 digits</li>
              </ul>
              <p>Tax IDs are stored using blind index hashing for privacy, allowing compliance verification without exposing raw identification data.</p>
            </div>

            <div className="legal-section">
              <h4>6. ENHANCED DUE DILIGENCE (EDD)</h4>
              <p>Players exceeding defined wager volume or deposit thresholds may be asked for proof of address and source of funds. Failure to provide within the specified timeframe results in account restrictions.</p>
            </div>

            <div className="legal-section">
              <h4>7. TRAVEL RULE</h4>
              <p>For cryptoasset transfers at or above £1,000, originator and beneficiary information is collected and transmitted where applicable under UK Travel Rule requirements (MLR 2017 as amended).</p>
            </div>

            <div className="legal-section">
              <h4>8. DATA PROTECTION</h4>
              <p>Processed under UK GDPR and Data Protection Act 2018.</p>
              <p>Legal basis: Article 6(1)(c) — compliance with legal obligation.</p>
              <p>Data is not sold. Shared only with verification provider and authorities where legally required.</p>
              <p>Data rights requests: <a href="mailto:privacy@xfchess.com">privacy@xfchess.com</a></p>
            </div>

            <div className="legal-section">
              <h4>9. DECLINED VERIFICATIONS</h4>
              <p>Common reasons: expired document, liveness fail, name mismatch, unrecognised document, sanctions match.</p>
              <p>Contact support at <a href="mailto:kyc@xfchess.com">kyc@xfchess.com</a> for case-by-case review. Sanctions matches cannot be overridden.</p>
            </div>

            <div className="legal-section">
              <h4>10. DISCLAIMER</h4>
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

export default KycPage;
