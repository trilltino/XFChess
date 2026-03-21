import { motion } from 'framer-motion';
import { ArrowLeft, Crown, Star, Shield, Calculator, CreditCard, AlertTriangle, CheckCircle, Clock, Users, TrendingUp } from 'lucide-react';
import { Link } from 'react-router-dom';

const MembershipPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — MEMBERSHIP, TIERS & SUBSCRIPTION TERMS</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: March 2026</span>
                <span className="operator-info">XFChess is operated by XForcesolutions LLC, registered in England and Wales</span>
              </div>
            </div>
            <div className="legal-disclaimer">
              <AlertTriangle size={20} color="#f59e0b" />
              <div>
                <p><strong>NOTE:</strong> XFChess is a pre-launch platform. Exact fee amounts marked [TBC] will be confirmed prior to launch and this page will be updated. All prices shown are in GBP.</p>
              </div>
            </div>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>1. WHAT IS XFCHESS MEMBERSHIP?</h4>
              <p>XFChess operates a tiered competition structure. All players start at Bronze level and progress as their activity and verification status develops.</p>

              <p>Membership is free. A Power Player subscription (£6.99/month) is an optional upgrade that removes the per-game platform fee for subscribers, replacing it with a flat monthly charge. For players who compete regularly, the subscription will typically be more cost-effective than paying per game.</p>

              <div className="membership-highlight">
                <p><strong>We are not in the business of monetising inactivity.</strong> If you are not playing regularly, the free tier with per-game fees will cost you less. We encourage you to use the break-even calculator below to assess which option suits your usage.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>2. COMPETITION TIERS</h4>
              <p>XFChess has three competition levels. Entry to each tier depends on your wager amount for that match, not your subscription status.</p>

              <div className="tiers-grid">
                <div className="tier-card bronze">
                  <div className="tier-header">
                    <Crown size={24} color="#cd7f32" />
                    <h5>BRONZE</h5>
                    <span className="tier-wager">Entry wager: £2.00 per side</span>
                  </div>
                  <div className="tier-pricing">
                    <div className="price-option">
                      <span className="label">Without subscription:</span>
                      <span className="price">£2.30 total</span>
                      <span className="breakdown"> (£2.00 wager + £0.30 platform fee)</span>
                    </div>
                    <div className="price-option">
                      <span className="label">With Power Player subscription:</span>
                      <span className="price">£2.00 total</span>
                      <span className="breakdown"> (platform fee waived)</span>
                    </div>
                  </div>
                  <div className="tier-suitable">
                    <span className="suitable-label">Suitable for:</span>
                    <span className="suitable-for">casual and beginner players</span>
                  </div>
                </div>

                <div className="tier-card silver">
                  <div className="tier-header">
                    <Star size={24} color="#c0c0c0" />
                    <h5>SILVER</h5>
                    <span className="tier-wager">Entry wager: £5.00 per side</span>
                  </div>
                  <div className="tier-pricing">
                    <div className="price-option">
                      <span className="label">Without subscription:</span>
                      <span className="price">£5.75 total</span>
                      <span className="breakdown"> (£5.00 wager + £0.75 platform fee)</span>
                    </div>
                    <div className="price-option">
                      <span className="label">With Power Player subscription:</span>
                      <span className="price">£5.00 total</span>
                      <span className="breakdown"> (platform fee waived)</span>
                    </div>
                  </div>
                  <div className="tier-suitable">
                    <span className="suitable-label">Suitable for:</span>
                    <span className="suitable-for">competitive and intermediate players</span>
                  </div>
                </div>

                <div className="tier-card gold">
                  <div className="tier-header">
                    <Crown size={24} color="#ffd700" />
                    <h5>GOLD</h5>
                    <span className="tier-wager">Entry wager: £10.00 per side</span>
                  </div>
                  <div className="tier-pricing">
                    <div className="price-option">
                      <span className="label">Without subscription:</span>
                      <span className="price">£11.50 total</span>
                      <span className="breakdown"> (£10.00 wager + £1.50 platform fee)</span>
                    </div>
                    <div className="price-option">
                      <span className="label">With Power Player subscription:</span>
                      <span className="price">£10.00 total</span>
                      <span className="breakdown"> (platform fee waived)</span>
                    </div>
                  </div>
                  <div className="tier-suitable">
                    <span className="suitable-label">Access:</span>
                    <span className="suitable-for">Power Player accounts only (requires completion of enhanced verification)</span>
                  </div>
                  <div className="tier-suitable">
                    <span className="suitable-label">Suitable for:</span>
                    <span className="suitable-for">experienced, high-frequency players</span>
                  </div>
                </div>
              </div>

              <div className="wager-explanation">
                <p>In all tiers, your wager is held directly in a Solana smart contract and is paid to your opponent if you lose, or returned to you plus your opponent's wager if you win. XFChess does not take a percentage of the wager pool. Our revenue is the platform fee only.</p>
              </div>

              <div className="total-cost-disclosure">
                <h5><AlertTriangle size={20} color="#e63946" /> TOTAL COST DISCLOSURE (DRIP PRICING COMPLIANCE)</h5>
                <p>Before you confirm entry to any match, a single screen will show:</p>
                <div className="cost-breakdown">
                  <div className="cost-item">
                    <span>Wager:</span>
                    <span>£[X.XX]</span>
                  </div>
                  <div className="cost-item">
                    <span>Platform fee:</span>
                    <span>£[X.XX] (£0.00 if subscribed)</span>
                  </div>
                  <div className="cost-item">
                    <span>Solana network fee:</span>
                    <span>~£[X.XX] (real-time estimate; actual may vary slightly)</span>
                  </div>
                  <div className="cost-divider"></div>
                  <div className="cost-item total">
                    <span>Total leaving your wallet:</span>
                    <span>£[X.XX]</span>
                  </div>
                </div>
                <p><strong>You will never be shown a lower number early in the flow and a higher number at confirmation.</strong></p>
              </div>
            </div>

            <div className="legal-section">
              <h4>3. THE POWER PLAYER SUBSCRIPTION — BREAK-EVEN GUIDE</h4>
              <p>The Power Player subscription costs £6.99 per month. The platform fee is waived on all matches while your subscription is active.</p>

              <p>At what point does the subscription pay for itself?</p>

              <div className="break-even-grid">
                <div className="break-even-card">
                  <div className="tier-badge bronze">Bronze</div>
                  <div className="break-even-details">
                    <span className="fee-per-game">£0.30 fee per game</span>
                    <span className="break-even-point">break-even at 24 games/month</span>
                  </div>
                </div>

                <div className="break-even-card">
                  <div className="tier-badge silver">Silver</div>
                  <div className="break-even-details">
                    <span className="fee-per-game">£0.75 fee per game</span>
                    <span className="break-even-point">break-even at 10 games/month</span>
                  </div>
                </div>

                <div className="break-even-card">
                  <div className="tier-badge gold">Gold</div>
                  <div className="break-even-details">
                    <span className="fee-per-game">£1.50 fee per game</span>
                    <span className="break-even-point">break-even at 5 games/month</span>
                  </div>
                </div>
              </div>

              <div className="calculator-info">
                <p>If you play across multiple tiers, your personal break-even will fall somewhere in between. A calculator on your account dashboard will show your projected monthly saving based on your recent match history before you commit to subscribing.</p>
                <p><strong>There is no pressure to subscribe.</strong> The calculator is there to help you make the right call for your usage pattern.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>4. POWER PLAYER VERIFICATION REQUIREMENT</h4>
              <p>To access Gold tier and to maintain an active subscription, players must complete enhanced identity verification. This is separate from the standard KYC check required to enter any wagered match.</p>

              <div className="verification-requirement">
                <Shield size={24} color="#e63946" />
                <div>
                  <p>At the point a player's cumulative wager total approaches £100, they will be prompted to complete enhanced verification before proceeding further. This involves an additional AML source-of-funds check conducted via our verification partner, Didit. The purpose is to meet our obligations under UK anti-money laundering regulations for higher-activity accounts.</p>
                  <p>Players who do not wish to complete enhanced verification may continue playing at Bronze and Silver tiers within their pre-verified limits.</p>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>5. SUBSCRIPTION BILLING</h4>
              <div className="billing-points">
                <div className="billing-item">
                  <CreditCard size={20} color="#e63946" />
                  <div>
                    <h5>Monthly Billing</h5>
                    <p>The Power Player subscription is billed monthly, on the same date each month (the "billing date"), starting from the date you first subscribe.</p>
                  </div>
                </div>

                <div className="billing-item">
                  <TrendingUp size={20} color="#e63946" />
                  <div>
                    <h5>Payment Method</h5>
                    <p>Payment is taken from your XFChess wallet balance or from the payment method on file.</p>
                  </div>
                </div>

                <div className="billing-item">
                  <Calculator size={20} color="#e63946" />
                  <div>
                    <h5>Pricing</h5>
                    <p>The price is £6.99/month inclusive of any applicable VAT. We will always show you the VAT-inclusive price. We will never add VAT on top of an advertised price at checkout.</p>
                  </div>
                </div>

                <div className="billing-item">
                  <AlertTriangle size={20} color="#f59e0b" />
                  <div>
                    <h5>Price Changes</h5>
                    <p>If your subscription price changes, we will notify you with no less than 30 days' notice before the new price takes effect and remind you of your right to cancel before the change applies.</p>
                  </div>
                </div>
              </div>

              <div className="vat-note">
                <h5>VAT NOTE:</h5>
                <p>XFChess subscription fees are subject to UK VAT at the standard rate (20%) once XFChess is VAT-registered (triggered at £90,000 annual taxable turnover). During the period prior to VAT registration, the £6.99 figure is the total you pay. Once VAT-registered, the displayed price will remain £6.99 inclusive — the VAT component (currently £1.16) is accounted for within that figure, not added on top.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>6. YOUR CANCELLATION RIGHTS</h4>
              <p>You can cancel your Power Player subscription at any time. There is no minimum term and no cancellation fee.</p>

              <div className="cancellation-sections">
                <div className="cancellation-section">
                  <h5><Users size={20} color="#e63946" /> HOW TO CANCEL:</h5>
                  <p>Cancel directly from your account dashboard via the "Manage Subscription" page. Cancellation is a single action — there is no phone call required, no email approval needed, and no retention flow that you must navigate through.</p>
                </div>

                <div className="cancellation-section">
                  <h5><Clock size={20} color="#e63946" /> WHAT HAPPENS WHEN YOU CANCEL:</h5>
                  <p>Your subscription remains active until the end of the billing period you have already paid for. You will not receive a refund for the unused portion of the current month unless you cancel within the cooling-off period described below, or unless we have materially changed the subscription terms.</p>
                </div>

                <div className="cancellation-section">
                  <h5><CheckCircle size={20} color="#27c93f" /> COOLING-OFF PERIOD (NEW SUBSCRIBERS):</h5>
                  <p>When you first subscribe, you have 14 days from the date of subscription to cancel and receive a full refund of your first monthly payment, provided you have not played any wagered matches using the subscription benefit during that period. If you have used the subscription to play matches within the 14-day window, a proportionate deduction reflecting the value of matches played may apply.</p>
                </div>

                <div className="cancellation-section">
                  <h5><AlertTriangle size={20} color="#f59e0b" /> RENEWAL REMINDER:</h5>
                  <p>We will send you a reminder notification at least [3–7 days — exact period TBC] before your monthly billing date. This reminder will state the amount due, the date it will be charged, and how to cancel. This is a commitment we are building into our subscription system ahead of the DMCCA subscription rules, which are expected to come into force in Autumn 2026. We intend to be fully compliant with those rules before they take effect.</p>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>7. REFUNDS</h4>
              <p>Outside the 14-day cooling-off period, subscription fees are non-refundable unless:</p>

              <div className="refund-conditions">
                <div className="refund-item">
                  <AlertTriangle size={20} color="#e63946" />
                  <div>
                    <h5>Service Unavailability</h5>
                    <p>XFChess has been unavailable for more than 72 consecutive hours during your billing period due to a fault on our side, in which case a pro-rata credit will be applied to your account</p>
                  </div>
                </div>

                <div className="refund-item">
                  <AlertTriangle size={20} color="#e63946" />
                  <div>
                    <h5>Material Changes</h5>
                    <p>We have changed the subscription terms materially and you cancel as a result of that change, in which case you are entitled to a refund of the unused portion</p>
                  </div>
                </div>

                <div className="refund-item">
                  <AlertTriangle size={20} color="#e63946" />
                  <div>
                    <h5>Fair Play Review</h5>
                    <p>A Fair Play Review voids a match you paid to enter — in that case, the match entry cost (wager + platform fee) is returned; the subscription fee itself is not affected</p>
                  </div>
                </div>
              </div>

              <p>To request a refund, contact: <a href="mailto:billing@xfchess.com">billing@xfchess.com</a></p>
            </div>

            <div className="legal-section">
              <h4>8. FREE TIER — NO SUBSCRIPTION REQUIRED</h4>
              <div className="free-tier-highlight">
                <CheckCircle size={24} color="#27c93f" />
                <div>
                  <p>Everything on XFChess except Gold tier and the subscription fee-waiver benefit is available without a subscription. You will always have a free way to play and pay per game. We will never force you to subscribe to access the core product.</p>
                </div>
              </div>
            </div>

            <div className="legal-section">
              <h4>9. CHANGES TO THIS PAGE</h4>
              <p>XFChess is a pre-launch platform and specific pricing, thresholds, and verification requirements will be confirmed prior to launch. We will update this page accordingly. If you subscribe and we change the terms materially, we will notify you before the change takes effect and you will have the right to cancel without penalty.</p>
            </div>

            <div className="legal-section">
              <h4>10. CONTACT</h4>
              <div className="contact-info">
                <div className="contact-item">
                  <h5>Billing queries:</h5>
                  <p><a href="mailto:billing@xfchess.com">billing@xfchess.com</a></p>
                </div>
                <div className="contact-item">
                  <h5>Subscription disputes:</h5>
                  <p><a href="mailto:support@xfchess.com">support@xfchess.com</a></p>
                </div>
                <div className="contact-item">
                  <h5>Legal / regulatory:</h5>
                  <p><a href="mailto:legal@xfchess.com">legal@xfchess.com</a></p>
                </div>
              </div>

              <div className="legal-highlight">
                <p><strong>[ALL CONTACT DETAILS TO BE CONFIRMED PRIOR TO LAUNCH]</strong></p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default MembershipPage;
