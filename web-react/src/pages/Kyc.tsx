import { motion } from 'framer-motion';
import { ArrowLeft, Shield, UserCheck, Camera, FileText, AlertTriangle, Clock, Eye, CheckCircle } from 'lucide-react';
import { Link } from 'react-router-dom';

const KycPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — IDENTITY VERIFICATION & KYC</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: March 2026</span>
                <span className="operator-info">XFChess is operated by XForcesolutions LLC, registered in England and Wales</span>
              </div>
            </div>
            <div className="legal-disclaimer">
              <AlertTriangle size={20} color="#f59e0b" />
              <div>
                <p><strong>NOTE:</strong> XFChess is a pre-launch platform. The verification process described here reflects our intended implementation. Specific flows and provider details will be confirmed prior to launch and this page will be updated accordingly.</p>
              </div>
            </div>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>1. WHY WE REQUIRE IDENTITY VERIFICATION</h4>
              <p>Identity verification is a legal requirement, not a commercial choice.</p>
              
              <p>Under the UK Money Laundering, Terrorist Financing and Transfer of Funds (Information on the Payer) Regulations 2017, and in anticipation of our obligations under the forthcoming FCA cryptoasset regulatory regime, XFChess is required to:</p>
              
              <div className="verification-requirements">
                <div className="requirement-item">
                  <UserCheck size={24} color="#e63946" />
                  <div>
                    <h5>Confirm Identity</h5>
                    <p>Confirm the identity of players before they handle funds on the platform</p>
                  </div>
                </div>

                <div className="requirement-item">
                  <Shield size={24} color="#e63946" />
                  <div>
                    <h5>Age Verification</h5>
                    <p>Verify that players are aged 18 or over</p>
                  </div>
                </div>

                <div className="requirement-item">
                  <AlertTriangle size={24} color="#e63946" />
                  <div>
                    <h5>Sanctions Screening</h5>
                    <p>Screen players against UK and international sanctions lists</p>
                  </div>
                </div>

                <div className="requirement-item">
                  <FileText size={24} color="#e63946" />
                  <div>
                    <h5>Enhanced Due Diligence</h5>
                    <p>Apply enhanced due diligence for higher-value accounts</p>
                  </div>
                </div>

                <div className="requirement-item">
                  <Camera size={24} color="#e63946" />
                  <div>
                    <h5>FATF Travel Rule</h5>
                    <p>Comply with the FATF Travel Rule regarding the originator and beneficiary of cryptoasset transfers</p>
                  </div>
                </div>
              </div>

              <div className="legal-highlight">
                <p><strong>We cannot accept deposits or allow players to enter wagered matches without a completed identity check. This is not negotiable and no exceptions will be made.</strong></p>
              </div>
            </div>

            <div className="legal-section">
              <h4>2. HOW VERIFICATION WORKS — POWERED BY DIDIT</h4>
              <p>XFChess intends to use Didit (didit.me) as its identity verification provider. Didit is an AI-powered identity platform that allows players to verify their identity quickly, without uploading sensitive documents directly to XFChess.</p>

              <div className="verification-steps">
                <div className="step-card">
                  <div className="step-number">1</div>
                  <div className="step-content">
                    <h5>Trigger</h5>
                    <p>When a player attempts to deposit funds or join the Human Arena for the first time, they will be prompted to complete identity verification if their wallet is not already verified.</p>
                  </div>
                </div>

                <div className="step-card">
                  <div className="step-number">2</div>
                  <div className="step-content">
                    <h5>Didit Hosted Flow</h5>
                    <p>The player is directed to a Didit-hosted verification page. This step happens on Didit's infrastructure, not on XFChess servers.</p>
                  </div>
                </div>

                <div className="step-card">
                  <div className="step-number">3</div>
                  <div className="step-content">
                    <h5>Document and Liveness Check</h5>
                    <p>The player completes a short verification process involving:</p>
                    <ul className="step-list">
                      <li>Capture of a government-issued photo ID (passport, national ID card, or driving licence)</li>
                      <li>A passive liveness check — a brief face scan to confirm the player is a real, present individual and not a photo or deepfake</li>
                      <li>A 1:1 face match between the live selfie and the ID document photo</li>
                    </ul>
                    <p>Didit supports over 14,000 government-issued document types from more than 220 countries.</p>
                  </div>
                </div>

                <div className="step-card">
                  <div className="step-number">4</div>
                  <div className="step-content">
                    <h5>Automated Decision</h5>
                    <p>Didit's AI analyses the submission in real time and returns one of three outcomes: Approved, Declined, or Under Review. The typical verification completes in under 30 seconds.</p>
                  </div>
                </div>

                <div className="step-card">
                  <div className="step-number">5</div>
                  <div className="step-content">
                    <h5>Confirmation</h5>
                    <p>If approved, the player's wallet is flagged as verified and they may proceed to deposit and play. XFChess receives a confirmation signal — not a copy of the player's documents.</p>
                  </div>
                </div>

                <div className="step-card">
                  <div className="step-number">6</div>
                  <div className="step-content">
                    <h5>Reusable Credential</h5>
                    <p>Once verified through Didit, players hold a reusable digital identity credential. If Didit is supported by other platforms in future, the player may not need to re-upload documents elsewhere. XFChess benefits from this because we do not need to store or manage identity documents ourselves.</p>
                  </div>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>3. WHAT DATA XFCHESS RECEIVES AND STORES</h4>
              <p>This is the core of our privacy approach: XFChess is designed to receive the minimum data necessary for compliance.</p>

              <div className="data-comparison">
                <div className="data-category received">
                  <h5><CheckCircle size={20} color="#27c93f" /> What XFChess intends to receive from Didit:</h5>
                  <ul className="data-list">
                    <li>A verified status signal (Approved / Declined / Under Review)</li>
                    <li>Confirmation that the player is aged 18 or over</li>
                    <li>Confirmation that the player is not on a UK or international sanctions watchlist</li>
                    <li>The player's full name and date of birth (required for AML record-keeping)</li>
                    <li>The player's country of residence (required for restricted territory enforcement)</li>
                  </ul>
                </div>

                <div className="data-category not-received">
                  <h5><AlertTriangle size={20} color="#f59e0b" /> What XFChess does NOT intend to store:</h5>
                  <ul className="data-list">
                    <li>Copies of identity documents (passports, driving licences, etc.)</li>
                    <li>Raw biometric data (facial scan images)</li>
                    <li>NFC chip data</li>
                  </ul>
                  <p>Document images and biometric data are processed and held by Didit in accordance with their own privacy policy and data retention controls. XFChess intends to implement data retention periods in line with the minimum required by UK AML regulations (currently five years from the end of the business relationship).</p>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>4. SANCTIONS AND WATCHLIST SCREENING</h4>
              <p>As part of the Didit verification flow, XFChess intends to screen all players against:</p>

              <div className="sanctions-list">
                <div className="sanctions-item">
                  <h5>HM Treasury UK financial sanctions list</h5>
                </div>
                <div className="sanctions-item">
                  <h5>UN consolidated sanctions list</h5>
                </div>
                <div className="sanctions-item">
                  <h5>EU consolidated sanctions list</h5>
                </div>
                <div className="sanctions-item">
                  <h5>OFAC (US) sanctions list</h5>
                </div>
                <div className="sanctions-item">
                  <h5>Politically Exposed Persons (PEP) databases</h5>
                </div>
              </div>

              <div className="legal-highlight">
                <p><strong>Players who appear on any applicable sanctions list will be declined at the verification stage and will not be permitted to use the platform. This is a legal requirement and is not subject to appeal through XFChess.</strong></p>
              </div>
            </div>

            <div className="legal-section">
              <h4>5. AGE VERIFICATION</h4>
              <p>XFChess is for adults aged 18 and over. No player may deposit funds or enter a wagered match without completing a verification check that confirms their age.</p>

              <div className="legal-highlight">
                <p><strong>"Intended for adults" is not sufficient. Our system is designed as a hard gate: verification is required before access, not after. A player who cannot confirm they are 18 or over at the point of verification will not be able to proceed.</strong></p>
              </div>
            </div>

            <div className="legal-section">
              <h4>6. ENHANCED DUE DILIGENCE (EDD)</h4>
              <p>For players whose activity exceeds defined thresholds — including high cumulative wager volumes or large individual deposits — XFChess may request additional documentation, including proof of address and source of funds information.</p>

              <p>Players in this category will be contacted directly. Failure to provide requested documentation within the specified timeframe will result in account restrictions until the matter is resolved.</p>
            </div>

            <div className="legal-section">
              <h4>7. ONGOING MONITORING</h4>
              <p>KYC is not a one-time event. XFChess intends to conduct periodic re-screening of active players against sanctions lists and to flag accounts whose activity patterns are inconsistent with their verified profile. This may result in a request for updated documentation.</p>
            </div>

            <div className="legal-section">
              <h4>8. TRAVEL RULE COMPLIANCE</h4>
              <p>The FATF Travel Rule requires that, for cryptoasset transfers above certain thresholds, originator and beneficiary information is collected and, where applicable, transmitted to the receiving institution. XFChess intends to implement Travel Rule compliance procedures as part of our pre-launch regulatory preparation, in line with the requirements applicable to our transaction volumes and counterparty relationships.</p>
            </div>

            <div className="legal-section">
              <h4>9. DATA PROTECTION</h4>
              <p>All personal data collected during the verification process will be handled in accordance with the UK General Data Protection Regulation (UK GDPR) and the Data Protection Act 2018.</p>

              <div className="data-protection-points">
                <div className="protection-item">
                  <h5>Legal Basis</h5>
                  <p>The legal basis for processing is compliance with a legal obligation (Article 6(1)(c) UK GDPR) for AML/KYC requirements, and legitimate interests for fraud prevention.</p>
                </div>

                <div className="protection-item">
                  <h5>Data Rights</h5>
                  <p>Players have the right to access, correct, and (subject to legal retention obligations) request deletion of their personal data.</p>
                </div>

                <div className="protection-item">
                  <h5>Data Sharing</h5>
                  <p>Data will not be sold or shared with third parties except as required by law or as described in this page (i.e. with Didit for the purposes of verification, and with relevant authorities where legally required).</p>
                </div>
              </div>

              <p>For full details, see our Privacy Policy [LINK].</p>
              <p>To exercise your data rights: <a href="mailto:privacy@xfchess.com">privacy@xfchess.com</a></p>
            </div>

            <div className="legal-section">
              <h4>10. WHAT HAPPENS IF YOUR VERIFICATION IS DECLINED</h4>
              <p>If Didit declines a verification, the player will receive an explanation of the reason where it is possible to provide one without disclosing information that would assist fraud.</p>

              <p>Common reasons for decline include: document not recognised, liveness check failed, document expired, name mismatch, or a sanctions list match.</p>

              <p>Players who believe their verification was declined in error may contact Didit directly through their support process, or contact XFChess at <a href="mailto:kyc@xfchess.com">kyc@xfchess.com</a>. We will assess on a case-by-case basis and, where appropriate, request a manual review through Didit.</p>

              <div className="legal-highlight">
                <p><strong>We cannot override a sanctions list match.</strong></p>
              </div>
            </div>

            <div className="legal-section">
              <h4>11. DISCLAIMER</h4>
              <p>This page describes our intended KYC and identity verification process as of the date of last review. XFChess is pre-launch and specific implementation details are subject to change as development and regulatory review progresses. This page will be updated to reflect the confirmed process prior to launch.</p>

              <p>Nothing on this page constitutes legal advice.</p>

              <div className="legal-contact">
                <p><strong>For regulatory or legal enquiries:</strong> <a href="mailto:legal@xfchess.com">legal@xfchess.com</a></p>
                <p><strong>For general enquiries:</strong> <a href="mailto:isicheivalentine@gmail.com">isicheivalentine@gmail.com</a></p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default KycPage;
