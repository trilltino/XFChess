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
                <span className="review-date">Last reviewed: March 2026</span>
                <span className="operator-info">XFChess is operated by XForcesolutions LLC, registered in England and Wales</span>
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
                <p>XFChess operates on a peer-to-peer basis. Players wager directly against one another. XFChess does not act as a bookmaker, does not hold or benefit from wager funds, and does not operate a house-edge model. Our intended revenue model is a fixed match fee per game.</p>
              </div>

              <div className="legal-highlight">
                <p>We are seeking formal legal advice to confirm this classification prior to launch. If that advice indicates a Gambling Operating Licence is required, we will not launch the wagering functionality until one is obtained.</p>
              </div>

              <div className="legal-subsection">
                <p>To preserve the skill-game classification, XFChess does not intend to introduce randomised gameplay elements, random matchmaking based on chance, or luck-based mechanics. Any such change would be subject to legal review before implementation.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>2. GAMBLING COMMISSION</h4>
              <p>Based on our current understanding, XFChess does not require a Gambling Operating Licence from the UK Gambling Commission. We are seeking independent legal confirmation of this position.</p>
              
              <div className="legal-contact">
                <p>If you believe our classification is incorrect, or if you have a concern about responsible play, you may contact the UK Gambling Commission at <a href="https://www.gamblingcommission.gov.uk" target="_blank" rel="noopener noreferrer">www.gamblingcommission.gov.uk</a>.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>3. TAX — HMRC</h4>
              
              <div className="tax-subsection">
                <h5>A. VAT</h5>
                <p>It is our understanding that platform fees charged by XFChess for arranging skill-game competitions are standard-rated for UK VAT purposes at 20%, as they do not qualify for the gambling exemption under Group 4 of Schedule 9 to the Value Added Tax Act 1994 (which applies to games of chance, not skill).</p>
                
                <p>Prior to launch, XFChess intends to engage a qualified accountant to confirm this position and to monitor our turnover against the mandatory VAT registration threshold (currently £90,000 per annum). We will register for VAT and charge the standard rate on platform fees once that threshold is reached or in advance if advised to do so. VAT, where applicable, would apply only to the platform fee retained by XFChess — not to the wager amounts passed between players, which we understand to be outside the scope of VAT as stake money.</p>
              </div>

              <div className="tax-subsection">
                <h5>B. Remote Gaming Duty (RGD)</h5>
                <p>Remote Gaming Duty is levied on profits derived from "the playing of a game of chance for a prize by remote means." Because chess is a game of skill, our current understanding is that XFChess platform fees are not subject to RGD. We intend to obtain specialist gambling tax advice to confirm this prior to launch.</p>
              </div>

              <div className="tax-subsection">
                <h5>C. Player Tax Obligations</h5>
                <p>Our general understanding is that winnings received by individual UK-resident players in a skill-based prize competition are not subject to income tax in the ordinary course of play. However:</p>
                <ul className="tax-list">
                  <li>Players who participate with commercial frequency or deploy automated tools may find that HMRC classifies their winnings as Trading Income subject to Income Tax and National Insurance. Players in this category should seek independent tax advice.</li>
                  <li>UK residents disposing of cryptoasset winnings may have Capital Gains Tax obligations and should seek independent advice.</li>
                  <li>XFChess does not provide tax advice to players. Nothing on this page constitutes tax advice.</li>
                </ul>
              </div>
            </div>

            <div className="legal-section">
              <h4>4. FCA CRYPTOASSET REGULATION</h4>
              <p>XFChess intends to facilitate the transfer of Solana (SOL) tokens between players as part of its prize competition mechanics. We are currently assessing whether and to what extent this activity falls within the scope of the FCA's cryptoasset regulatory regime.</p>
              
              <div className="legal-timeline">
                <p>The FCA has confirmed that a cryptoasset authorisation application gateway will open on 30 September 2026, with the window closing 28 February 2027. The new regime is scheduled to come into force on 25 October 2027.</p>
              </div>

              <p>XFChess intends to engage FCA-specialist legal counsel to determine our precise regulatory status and, if required, to submit an application for cryptoasset authorisation during the 30 September 2026 – 28 February 2027 gateway window ahead of the October 2027 commencement date.</p>

              <p>We are currently assessing our obligations under the Money Laundering, Terrorist Financing and Transfer of Funds (Information on the Payer) Regulations 2017. If registration with the FCA under the MLRs is required ahead of the gateway opening, we will complete that registration before accepting any player funds.</p>
            </div>

            <div className="legal-section">
              <h4>5. ANTI-MONEY LAUNDERING (AML) AND KNOW YOUR CUSTOMER (KYC)</h4>
              <p>XFChess intends to implement mandatory identity verification before any player may deposit funds or enter a wagered match. Our intended design is that no wagered match can be joined without a successful identity check confirming the player is aged 18 or over.</p>
              
              <p>Our planned KYC process will require players to provide government-issued photo identification and proof of address at minimum, with enhanced due diligence for higher-value accounts.</p>

              <p>We are also reviewing our obligations under the FATF Travel Rule, which requires collection and transmission of identifying information about the originator and beneficiary of cryptoasset transfers above certain thresholds.</p>

              <p>The specific KYC provider and process will be confirmed prior to launch. Player data will be processed in accordance with our Privacy Policy and UK GDPR.</p>
            </div>

            <div className="legal-section">
              <h4>6. CONSUMER PROTECTION — PRICING TRANSPARENCY</h4>
              <p>XFChess intends to comply fully with the Digital Markets, Competition and Consumers Act 2024 and the Consumer Protection from Unfair Trading Regulations (as updated), which prohibit drip pricing. These rules have been in force since 6 April 2025.</p>

              <p>Our intended design is that before a player confirms entry to any wagered match, a single screen will display the full total cost to participate, broken down as follows:</p>

              <div className="pricing-breakdown">
                <div className="pricing-item">
                  <span>Wager amount:</span>
                  <span>£[X.XX] (held in smart contract; returned to winner)</span>
                </div>
                <div className="pricing-item">
                  <span>XFChess platform fee:</span>
                  <span>£0.85 per player</span>
                </div>
                <div className="pricing-item">
                  <span>Solana network fee:</span>
                  <span>~£[X.XX] (real-time estimate; actual may vary slightly)</span>
                </div>
                <div className="pricing-divider"></div>
                <div className="pricing-item total">
                  <span>Total deducted from wallet:</span>
                  <span>£[X.XX]</span>
                </div>
              </div>

              <p>The Solana network fee is set by the Solana network, not by XFChess, but as it is a required cost of participation we intend to include it in the pre-commitment total. Where the actual network fee differs from the estimate, the actual amount will appear in the post-match transaction summary.</p>

              <div className="legal-highlight">
                <p>Players will not encounter any cost that was not disclosed prior to confirming entry. We will have this flow reviewed against CMA guidance before launch.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>7. RESTRICTED TERRITORIES</h4>
              <p>XFChess is intended to operate under UK law for players in jurisdictions where participation in a peer-to-peer skill-game prize competition is lawful.</p>

              <p>We do not intend to offer services to players located in the following territories:</p>

              <ul className="restricted-territories">
                <li><strong>United States of America</strong> (including all states and territories). Multiple US states treat peer-to-peer wagering on skill games as unlawful under state gambling law, and the federal legal position remains complex. We do not have authorisation to serve US-located players.</li>
                <li><strong>People's Republic of China</strong></li>
                <li><strong>The Netherlands</strong> (where online games for money require a Kansspelautoriteit licence)</li>
                <li><strong>Any jurisdiction</strong> where participation would be unlawful under applicable local law</li>
              </ul>

              <p>Prior to launch, we intend to implement IP-level geofencing and KYC-level residency checks to enforce these restrictions. Players who circumvent geographic restrictions using VPNs or other means will have their accounts suspended and funds returned where possible.</p>

              <div className="legal-highlight">
                <p>By using XFChess, players confirm they are not located in a restricted territory and that their participation is lawful under the laws of their jurisdiction. Players are responsible for ensuring compliance with their local laws.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>8. RESPONSIBLE PLAY</h4>
              <p>Although XFChess is a skill-based prize competition and not a regulated gambling product, we take player welfare seriously.</p>

              <ul className="responsible-play-list">
                <li>XFChess is intended for adults aged 18 and over. Mandatory age verification is part of our planned KYC process.</li>
                <li>Do not participate with funds you cannot afford to lose.</li>
                <li>If competitive play is affecting your wellbeing or finances, please seek support via GamCare (<a href="https://www.gamcare.org.uk" target="_blank" rel="noopener noreferrer">www.gamcare.org.uk</a>) or the National Gambling Helpline: <strong>0808 8020 133</strong>.</li>
              </ul>
            </div>

            <div className="legal-section">
              <h4>9. SMART CONTRACT SECURITY</h4>
              <p>It is our intention that all player wager funds will be held in and settled via Solana smart contracts, such that XFChess does not hold custody of player funds at any point during a match. Funds would be released directly from the smart contract to the winner upon verified game completion.</p>

              <p>We intend to commission an independent smart contract security audit before the wagering functionality goes live. Details of the auditing firm and a link to the published report will be added to this page prior to launch.</p>
            </div>

            <div className="legal-section">
              <h4>10. DISCLAIMER</h4>
              <p>This page reflects our current good-faith understanding of our legal and regulatory obligations and our intended approach to compliance. It does not constitute legal or tax advice. The regulatory landscape for skill-based prize competitions, cryptoassets, and online gaming is evolving, and positions stated here may change as we obtain formal professional advice and as regulation develops.</p>

              <p>XFChess will update this page as our legal and regulatory review progresses and as the platform approaches launch.</p>

              <div className="legal-contact">
                <p><strong>For legal or regulatory enquiries:</strong> <a href="mailto:isicheivalentine@gmail.com">isicheivalentine@gmail.com</a></p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default LegalPage;
