import { motion } from 'framer-motion';
import { ArrowLeft, Crown, Star, Shield, Calculator, CreditCard, AlertTriangle, CheckCircle, Clock, Users, TrendingUp } from 'lucide-react';
import { Link } from 'react-router-dom';

const MembershipPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="page-hero">
          <div className="card" style={{background: 'linear-gradient(135deg, rgba(230, 57, 70, 0.05), rgba(245, 158, 11, 0.05))', border: '1px solid rgba(230, 57, 70, 0.2)', backdropFilter: 'blur(10px)'}}>
            <div className="card-header" style={{textAlign: 'center', paddingBottom: '24px'}}>
              <div style={{display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '12px', marginBottom: '16px'}}>
                <Crown size={32} color="#e63946" />
                <h3 className="card-title" style={{fontSize: '2em', fontWeight: '700', background: 'linear-gradient(135deg, #e63946, #f59e0b)', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', margin: 0}}>
                  XFCHESS MEMBERSHIP
                </h3>
                <Crown size={32} color="#f59e0b" />
              </div>
              <p className="card-subtitle" style={{fontSize: '1.1em', color: 'var(--text-dim)', marginBottom: '8px'}}>
                TIERS & SUBSCRIPTION TERMS
              </p>
              <div className="card-subtitle" style={{display: 'flex', justifyContent: 'center', gap: '24px', fontSize: '0.9em', color: 'var(--text-dim)'}}>
                <span style={{display: 'flex', alignItems: 'center', gap: '6px'}}>
                  <Clock size={16} />
                  Last reviewed: March 2026
                </span>
                <span style={{display: 'flex', alignItems: 'center', gap: '6px'}}>
                  <Shield size={16} />
                  XForcesolutions LLC, England & Wales
                </span>
              </div>
            </div>
            <div className="card-content" style={{display: 'flex', gap: '16px', alignItems: 'flex-start', padding: '20px', background: 'linear-gradient(135deg, rgba(245, 158, 11, 0.1), rgba(230, 57, 70, 0.1))', borderRadius: '12px', margin: '16px 0', border: '1px solid rgba(245, 158, 11, 0.3)'}}>
              <AlertTriangle size={24} color="#f59e0b" style={{flexShrink: 0, marginTop: '2px'}} />
              <div>
                <p style={{margin: 0, fontSize: '1.05em', lineHeight: '1.6'}}><strong style={{color: '#e63946'}}>IMPORTANT:</strong> XFChess is a pre-launch platform. Exact fee amounts marked <span style={{background: 'rgba(245, 158, 11, 0.2)', padding: '2px 6px', borderRadius: '4px', fontFamily: 'monospace'}}>[TBC]</span> will be confirmed prior to launch and this page will be updated. All prices shown in GBP (£).</p>
              </div>
            </div>
          </div>
        </div>

        <div className="page-section">
          <div className="section-header" style={{textAlign: 'center', marginBottom: '32px'}}>
            <div className="section-number" style={{display: 'inline-flex', alignItems: 'center', justifyContent: 'center', width: '48px', height: '48px', background: 'linear-gradient(135deg, #e63946, #f59e0b)', color: 'white', borderRadius: '50%', fontSize: '1.5em', fontWeight: '700', marginBottom: '16px'}}>
              1
            </div>
            <h4 style={{fontSize: '1.8em', fontWeight: '600', margin: '0 0 16px 0', background: 'linear-gradient(135deg, var(--text), var(--text-dim))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent'}}>WHAT IS XFCHESS MEMBERSHIP?</h4>
          </div>
          
          <div className="card" style={{background: 'linear-gradient(135deg, rgba(255, 255, 255, 0.1), rgba(230, 57, 70, 0.05))', border: '1px solid rgba(230, 57, 70, 0.2)', borderRadius: '16px', padding: '24px', marginBottom: '24px'}}>
            <div className="card-content" style={{fontSize: '1.1em', lineHeight: '1.7', textAlign: 'center'}}>
              XFChess operates a <strong style={{color: '#e63946'}}>tiered competition structure</strong>. All players start at Bronze level and progress as their activity and verification status develops.
            </div>
          </div>

          <div className="grid-2" style={{gap: '24px', marginBottom: '24px'}}>
            <div className="card" style={{background: 'linear-gradient(135deg, rgba(230, 57, 70, 0.08), rgba(245, 158, 11, 0.08))', border: '1px solid rgba(230, 57, 70, 0.2)', borderRadius: '12px', padding: '20px'}}>
              <div className="card-icon" style={{background: 'rgba(230, 57, 70, 0.15)', borderColor: 'rgba(230, 57, 70, 0.3)', width: '56px', height: '56px'}}>
                <Users size={28} color="#e63946" />
              </div>
              <div className="card-header">
                <h5 className="card-title" style={{color: '#e63946'}}>Free Membership</h5>
              </div>
              <p className="card-content" style={{fontSize: '1.05em', lineHeight: '1.6'}}>
                Start playing immediately with no subscription required. Access Bronze and Silver tiers with per-game platform fees.
              </p>
            </div>

            <div className="card" style={{background: 'linear-gradient(135deg, rgba(245, 158, 11, 0.08), rgba(230, 57, 70, 0.08))', border: '1px solid rgba(245, 158, 11, 0.3)', borderRadius: '12px', padding: '20px'}}>
              <div className="card-icon" style={{background: 'rgba(245, 158, 11, 0.15)', borderColor: 'rgba(245, 158, 11, 0.3)', width: '56px', height: '56px'}}>
                <Crown size={28} color="#f59e0b" />
              </div>
              <div className="card-header">
                <h5 className="card-title" style={{color: '#f59e0b'}}>Power Player</h5>
                <p className="card-subtitle">£6.99/month</p>
              </div>
              <p className="card-content" style={{fontSize: '1.05em', lineHeight: '1.6'}}>
                Optional upgrade that removes per-game platform fees. For regular players, this is typically more cost-effective than paying per game.
              </p>
            </div>
          </div>

          <div className="card card-centered" style={{background: 'linear-gradient(135deg, rgba(230, 57, 70, 0.12), rgba(245, 158, 11, 0.12))', border: '2px solid rgba(230, 57, 70, 0.3)', borderRadius: '16px', padding: '24px', maxWidth: '600px', margin: '0 auto'}}>
            <div className="card-icon" style={{background: 'rgba(230, 57, 70, 0.2)', borderColor: 'rgba(230, 57, 70, 0.4)', width: '64px', height: '64px'}}>
              <Calculator size={32} color="#e63946" />
            </div>
            <div className="card-header">
              <h5 className="card-title" style={{fontSize: '1.3em', color: '#e63946'}}>Smart Pricing Philosophy</h5>
            </div>
            <p className="card-content" style={{fontSize: '1.1em', lineHeight: '1.6', textAlign: 'center', fontStyle: 'italic'}}>
              <strong>We are not in the business of monetising inactivity.</strong> If you are not playing regularly, the free tier with per-game fees will cost you less. Use the break-even calculator below to assess which option suits your usage pattern.
            </p>
          </div>
        </div>

        <div className="page-section">
          <div className="section-header" style={{textAlign: 'center', marginBottom: '40px'}}>
            <div className="section-number" style={{display: 'inline-flex', alignItems: 'center', justifyContent: 'center', width: '48px', height: '48px', background: 'linear-gradient(135deg, #e63946, #f59e0b)', color: 'white', borderRadius: '50%', fontSize: '1.5em', fontWeight: '700', marginBottom: '16px'}}>
              2
            </div>
            <h4 style={{fontSize: '1.8em', fontWeight: '600', margin: '0 0 16px 0', background: 'linear-gradient(135deg, var(--text), var(--text-dim))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent'}}>COMPETITION TIERS</h4>
            <p className="card-content" style={{fontSize: '1.1em', maxWidth: '600px', margin: '0 auto'}}>XFChess has three competition levels. Entry to each tier depends on your <strong style={{color: '#e63946'}}>wager amount</strong>, not your subscription status.</p>
          </div>

          <div className="grid-3" style={{gap: '24px'}}>
            <div className="card card-centered" style={{background: 'linear-gradient(135deg, rgba(205, 127, 50, 0.08), rgba(139, 69, 19, 0.05))', border: '2px solid rgba(205, 127, 50, 0.3)', borderRadius: '16px', padding: '24px', position: 'relative', overflow: 'hidden', boxShadow: '0 8px 24px rgba(205, 127, 50, 0.15)'}}>
              <div style={{position: 'absolute', top: '0', left: '0', right: '0', height: '4px', background: 'linear-gradient(90deg, #cd7f32, #b8860b)'}}></div>
              <div className="card-icon" style={{background: 'linear-gradient(135deg, rgba(205, 127, 50, 0.2), rgba(139, 69, 19, 0.15))', borderColor: 'rgba(205, 127, 50, 0.4)', width: '64px', height: '64px', marginBottom: '16px'}}>
                <Crown size={32} color="#cd7f32" />
              </div>
              <div className="card-header">
                <h5 className="card-title" style={{fontSize: '1.4em', color: '#cd7f32', marginBottom: '8px'}}>BRONZE</h5>
                <p className="card-subtitle" style={{fontSize: '1em', fontWeight: '600'}}>Entry wager: £2.00 per side</p>
              </div>
              <div className="card-content" style={{textAlign: 'left'}}>
                <div className="pricing-option" style={{background: 'rgba(205, 127, 50, 0.1)', padding: '12px', borderRadius: '8px', marginBottom: '12px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px'}}>
                    <span style={{fontWeight: '600'}}>Standard</span>
                    <span style={{color: '#cd7f32', fontWeight: '700'}}>£2.30</span>
                  </div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>£2.00 wager + £0.30 platform fee</div>
                </div>
                <div className="pricing-option" style={{background: 'rgba(245, 158, 11, 0.1)', padding: '12px', borderRadius: '8px', marginBottom: '12px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px'}}>
                    <span style={{fontWeight: '600'}}>Power Player</span>
                    <span style={{color: '#f59e0b', fontWeight: '700'}}>£2.00</span>
                  </div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Platform fee waived</div>
                </div>
                <div style={{textAlign: 'center', padding: '12px', background: 'rgba(205, 127, 50, 0.05)', borderRadius: '8px', marginTop: '12px'}}>
                  <span style={{color: '#cd7f32', fontWeight: '600'}}>🎯 Casual & Beginner Players</span>
                </div>
              </div>
            </div>

            <div className="card card-centered" style={{background: 'linear-gradient(135deg, rgba(192, 192, 192, 0.08), rgba(128, 128, 128, 0.05))', border: '2px solid rgba(192, 192, 192, 0.3)', borderRadius: '16px', padding: '24px', position: 'relative', overflow: 'hidden', boxShadow: '0 8px 24px rgba(192, 192, 192, 0.15)'}}>
              <div style={{position: 'absolute', top: '0', left: '0', right: '0', height: '4px', background: 'linear-gradient(90deg, #c0c0c0, #808080)'}}></div>
              <div className="card-icon" style={{background: 'linear-gradient(135deg, rgba(192, 192, 192, 0.2), rgba(128, 128, 128, 0.15))', borderColor: 'rgba(192, 192, 192, 0.4)', width: '64px', height: '64px', marginBottom: '16px'}}>
                <Star size={32} color="#c0c0c0" />
              </div>
              <div className="card-header">
                <h5 className="card-title" style={{fontSize: '1.4em', color: '#c0c0c0', marginBottom: '8px'}}>SILVER</h5>
                <p className="card-subtitle" style={{fontSize: '1em', fontWeight: '600'}}>Entry wager: £5.00 per side</p>
              </div>
              <div className="card-content" style={{textAlign: 'left'}}>
                <div className="pricing-option" style={{background: 'rgba(192, 192, 192, 0.1)', padding: '12px', borderRadius: '8px', marginBottom: '12px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px'}}>
                    <span style={{fontWeight: '600'}}>Standard</span>
                    <span style={{color: '#c0c0c0', fontWeight: '700'}}>£5.75</span>
                  </div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>£5.00 wager + £0.75 platform fee</div>
                </div>
                <div className="pricing-option" style={{background: 'rgba(245, 158, 11, 0.1)', padding: '12px', borderRadius: '8px', marginBottom: '12px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px'}}>
                    <span style={{fontWeight: '600'}}>Power Player</span>
                    <span style={{color: '#f59e0b', fontWeight: '700'}}>£5.00</span>
                  </div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Platform fee waived</div>
                </div>
                <div style={{textAlign: 'center', padding: '12px', background: 'rgba(192, 192, 192, 0.05)', borderRadius: '8px', marginTop: '12px'}}>
                  <span style={{color: '#c0c0c0', fontWeight: '600'}}>⚔️ Competitive & Intermediate</span>
                </div>
              </div>
            </div>

            <div className="card card-centered" style={{background: 'linear-gradient(135deg, rgba(255, 215, 0, 0.08), rgba(255, 193, 7, 0.05))', border: '2px solid rgba(255, 215, 0, 0.4)', borderRadius: '16px', padding: '24px', position: 'relative', overflow: 'hidden', boxShadow: '0 8px 32px rgba(255, 215, 0, 0.2)'}}>
              <div style={{position: 'absolute', top: '0', left: '0', right: '0', height: '4px', background: 'linear-gradient(90deg, #ffd700, #ffc107)'}}></div>
              <div className="card-icon" style={{background: 'linear-gradient(135deg, rgba(255, 215, 0, 0.2), rgba(255, 193, 7, 0.15))', borderColor: 'rgba(255, 215, 0, 0.4)', width: '64px', height: '64px', marginBottom: '16px'}}>
                <Crown size={32} color="#ffd700" />
              </div>
              <div className="card-header">
                <h5 className="card-title" style={{fontSize: '1.4em', color: '#ffd700', marginBottom: '8px'}}>GOLD</h5>
                <p className="card-subtitle" style={{fontSize: '1em', fontWeight: '600'}}>Entry wager: £10.00 per side</p>
              </div>
              <div className="card-content" style={{textAlign: 'left'}}>
                <div className="pricing-option" style={{background: 'rgba(255, 215, 0, 0.1)', padding: '12px', borderRadius: '8px', marginBottom: '12px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px'}}>
                    <span style={{fontWeight: '600'}}>Standard</span>
                    <span style={{color: '#ffd700', fontWeight: '700'}}>£11.50</span>
                  </div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>£10.00 wager + £1.50 platform fee</div>
                </div>
                <div className="pricing-option" style={{background: 'rgba(245, 158, 11, 0.1)', padding: '12px', borderRadius: '8px', marginBottom: '12px'}}>
                  <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '4px'}}>
                    <span style={{fontWeight: '600'}}>Power Player</span>
                    <span style={{color: '#f59e0b', fontWeight: '700'}}>£10.00</span>
                  </div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Platform fee waived</div>
                </div>
                <div style={{textAlign: 'center', padding: '12px', background: 'rgba(255, 215, 0, 0.05)', borderRadius: '8px', marginTop: '12px', border: '1px solid rgba(255, 215, 0, 0.2)'}}>
                  <div style={{color: '#ffd700', fontWeight: '600', marginBottom: '4px'}}>🔒 Power Player Only</div>
                  <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Enhanced verification required</div>
                  <div style={{color: '#ffd700', fontWeight: '600', marginTop: '4px'}}>👑 Experienced & High-Frequency</div>
                </div>
              </div>
            </div>
          </div>

          <div className="card" style={{background: 'linear-gradient(135deg, rgba(230, 57, 70, 0.05), rgba(245, 158, 11, 0.05))', border: '1px solid rgba(230, 57, 70, 0.2)', borderRadius: '16px', padding: '24px', marginTop: '32px'}}>
            <div className="card-header" style={{textAlign: 'center', marginBottom: '20px'}}>
              <div className="card-icon" style={{background: 'rgba(230, 57, 70, 0.15)', borderColor: 'rgba(230, 57, 70, 0.3)', width: '56px', height: '56px', margin: '0 auto 16px'}}>
                <Shield size={28} color="#e63946" />
              </div>
              <h5 className="card-title" style={{fontSize: '1.3em', color: '#e63946'}}>Secure Wager System</h5>
            </div>
            <p className="card-content" style={{fontSize: '1.1em', lineHeight: '1.7', textAlign: 'center', marginBottom: '16px'}}>
              In all tiers, your wager is held directly in a <strong style={{color: '#e63946'}}>Solana smart contract</strong> and is paid to your opponent if you lose, or returned to you plus your opponent's wager if you win.
            </p>
            <div style={{display: 'flex', justifyItems: 'center', gap: '24px', flexWrap: 'wrap'}}>
              <div style={{textAlign: 'center', flex: '1', minWidth: '150px'}}>
                <div style={{fontSize: '2em', fontWeight: '700', color: '#e63946', marginBottom: '4px'}}>0%</div>
                <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Wager Pool Commission</div>
              </div>
              <div style={{textAlign: 'center', flex: '1', minWidth: '150px'}}>
                <div style={{fontSize: '2em', fontWeight: '700', color: '#f59e0b', marginBottom: '4px'}}>100%</div>
                <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Player Winnings</div>
              </div>
              <div style={{textAlign: 'center', flex: '1', minWidth: '150px'}}>
                <div style={{fontSize: '2em', fontWeight: '700', color: '#27c93f', marginBottom: '4px'}}>✓</div>
                <div style={{fontSize: '0.9em', color: 'var(--text-dim)'}}>Blockchain Security</div>
              </div>
            </div>
            <p className="card-content" style={{fontSize: '1em', fontStyle: 'italic', textAlign: 'center', marginTop: '16px', color: 'var(--text-dim)'}}>
              XFChess revenue comes only from platform fees - we never take a percentage of your winnings.
            </p>
          </div>

          <div className="card">
            <div className="card-header">
              <h5 className="card-title"><AlertTriangle size={20} color="#e63946" /> TOTAL COST DISCLOSURE (DRIP PRICING COMPLIANCE)</h5>
            </div>
            <p className="card-content">Before you confirm entry to any match, a single screen will show:</p>
            <div className="table-container">
              <table className="table">
                <tbody>
                  <tr>
                    <td>Wager:</td>
                    <td>£[X.XX]</td>
                  </tr>
                  <tr>
                    <td>Platform fee:</td>
                    <td>£[X.XX] (£0.00 if subscribed)</td>
                  </tr>
                  <tr>
                    <td>Solana network fee:</td>
                    <td>~£[X.XX] (real-time estimate; actual may vary slightly)</td>
                  </tr>
                  <tr style={{borderTop: '2px solid var(--border)'}}>
                    <td><strong>Total leaving your wallet:</strong></td>
                    <td><strong>£[X.XX]</strong></td>
                  </tr>
                </tbody>
              </table>
            </div>
            <p className="card-content" style={{marginTop: '16px'}}><strong>You will never be shown a lower number early in the flow and a higher number at confirmation.</strong></p>
          </div>
        </div>

        <div className="page-section">
          <h4>3. THE POWER PLAYER SUBSCRIPTION — BREAK-EVEN GUIDE</h4>
          <p className="card-content">The Power Player subscription costs £6.99 per month. The platform fee is waived on all matches while your subscription is active.</p>

          <p className="card-content">At what point does the subscription pay for itself?</p>

          <div className="grid-3">
            <div className="card card-centered">
              <div className="card-header">
                <h5 className="card-title">Bronze</h5>
              </div>
              <div className="card-content">
                <p><strong>£0.30 fee per game</strong></p>
                <p><strong>break-even at 24 games/month</strong></p>
              </div>
            </div>

            <div className="card card-centered">
              <div className="card-header">
                <h5 className="card-title">Silver</h5>
              </div>
              <div className="card-content">
                <p><strong>£0.75 fee per game</strong></p>
                <p><strong>break-even at 10 games/month</strong></p>
              </div>
            </div>

            <div className="card card-centered">
              <div className="card-header">
                <h5 className="card-title">Gold</h5>
              </div>
              <div className="card-content">
                <p><strong>£1.50 fee per game</strong></p>
                <p><strong>break-even at 5 games/month</strong></p>
              </div>
            </div>
          </div>

          <div className="card">
            <div className="card-header">
              <h5 className="card-title">Calculator Information</h5>
            </div>
            <p className="card-content">If you play across multiple tiers, your personal break-even will fall somewhere in between. A calculator on your account dashboard will show your projected monthly saving based on your recent match history before you commit to subscribing.</p>
            <p className="card-content"><strong>There is no pressure to subscribe.</strong> The calculator is there to help you make the right call for your usage pattern.</p>
          </div>
        </div>

        <div className="page-section">
          <h4>4. POWER PLAYER VERIFICATION REQUIREMENT</h4>
          <p className="card-content">To access Gold tier and to maintain an active subscription, players must complete enhanced identity verification. This is separate from the standard KYC check required to enter any wagered match.</p>

          <div className="card" style={{background: 'rgba(230, 57, 70, 0.1)', border: '1px solid rgba(230, 57, 70, 0.3)'}}>
            <div className="card-content" style={{display: 'flex', gap: '12px', alignItems: 'flex-start'}}>
              <Shield size={24} color="#e63946" />
              <div>
                <p>At the point a player's cumulative wager total approaches £100, they will be prompted to complete enhanced verification before proceeding further. This involves an additional AML source-of-funds check conducted via our verification partner, Didit. The purpose is to meet our obligations under UK anti-money laundering regulations for higher-activity accounts.</p>
                <p>Players who do not wish to complete enhanced verification may continue playing at Bronze and Silver tiers within their pre-verified limits.</p>
              </div>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h4>5. SUBSCRIPTION BILLING</h4>
          
          <div className="grid-2">
            <div className="card">
              <div className="card-icon"><CreditCard size={20} /></div>
              <div className="card-header">
                <h5 className="card-title">Monthly Billing</h5>
              </div>
              <p className="card-content">The Power Player subscription is billed monthly, on the same date each month (the "billing date"), starting from the date you first subscribe.</p>
            </div>

            <div className="card">
              <div className="card-icon"><TrendingUp size={20} /></div>
              <div className="card-header">
                <h5 className="card-title">Payment Method</h5>
              </div>
              <p className="card-content">Payment is taken from your XFChess wallet balance or from the payment method on file.</p>
            </div>

            <div className="card">
              <div className="card-icon"><Calculator size={20} /></div>
              <div className="card-header">
                <h5 className="card-title">Pricing</h5>
              </div>
              <p className="card-content">The price is £6.99/month inclusive of any applicable VAT. We will always show you the VAT-inclusive price. We will never add VAT on top of an advertised price at checkout.</p>
            </div>

            <div className="card">
              <div className="card-icon"><AlertTriangle size={20} /></div>
              <div className="card-header">
                <h5 className="card-title">Price Changes</h5>
              </div>
              <p className="card-content">If your subscription price changes, we will notify you with no less than 30 days' notice before the new price takes effect and remind you of your right to cancel before the change applies.</p>
            </div>
          </div>

          <div className="card">
            <div className="card-header">
              <h5 className="card-title">VAT NOTE:</h5>
            </div>
            <p className="card-content">XFChess subscription fees are subject to UK VAT at the standard rate (20%) once XFChess is VAT-registered (triggered at £90,000 annual taxable turnover). During the period prior to VAT registration, the £6.99 figure is the total you pay. Once VAT-registered, the displayed price will remain £6.99 inclusive — the VAT component (currently £1.16) is accounted for within that figure, not added on top.</p>
          </div>
        </div>

        <div className="page-section">
          <h4>6. YOUR CANCELLATION RIGHTS</h4>
          <p className="card-content">You can cancel your Power Player subscription at any time. There is no minimum term and no cancellation fee.</p>

          <div className="grid-2">
            <div className="card card-centered">
              <div className="card-icon"><Users size={20} color="#e63946" /></div>
              <div className="card-header">
                <h5 className="card-title">HOW TO CANCEL:</h5>
              </div>
              <p className="card-content">Cancel directly from your account dashboard via the "Manage Subscription" page. Cancellation is a single action — there is no phone call required, no email approval needed, and no retention flow that you must navigate through.</p>
            </div>

            <div className="card card-centered">
              <div className="card-icon"><Clock size={20} color="#e63946" /></div>
              <div className="card-header">
                <h5 className="card-title">WHAT HAPPENS WHEN YOU CANCEL:</h5>
              </div>
              <p className="card-content">Your subscription remains active until the end of the billing period you have already paid for. You will not receive a refund for the unused portion of the current month unless you cancel within the cooling-off period described below, or unless we have materially changed the subscription terms.</p>
            </div>

            <div className="card card-centered">
              <div className="card-icon"><CheckCircle size={20} color="#27c93f" /></div>
              <div className="card-header">
                <h5 className="card-title">COOLING-OFF PERIOD (NEW SUBSCRIBERS):</h5>
              </div>
              <p className="card-content">When you first subscribe, you have 14 days from the date of subscription to cancel and receive a full refund of your first monthly payment, provided you have not played any wagered matches using the subscription benefit during that period. If you have used the subscription to play matches within the 14-day window, a proportionate deduction reflecting the value of matches played may apply.</p>
            </div>

            <div className="card card-centered">
              <div className="card-icon"><AlertTriangle size={20} color="#f59e0b" /></div>
              <div className="card-header">
                <h5 className="card-title">RENEWAL REMINDER:</h5>
              <p>XFChess is a pre-launch platform and specific pricing, thresholds, and verification requirements will be confirmed prior to launch. We will update this page accordingly. If you subscribe and we change the terms materially, we will notify you before the change takes effect and you will have the right to cancel without penalty.</p>
            </div>
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

            <div className="card" style={{background: 'linear-gradient(135deg, rgba(245, 158, 11, 0.15), rgba(230, 57, 70, 0.15))', border: '2px solid rgba(245, 158, 11, 0.4)', boxShadow: '0 4px 12px rgba(245, 158, 11, 0.2)'}}>
              <div className="card-content" style={{textAlign: 'center', padding: '20px'}}>
                <AlertTriangle size={24} color="#f59e0b" style={{marginBottom: '12px'}} />
                <p style={{fontSize: '1.1em', fontWeight: '600', color: '#f59e0b', margin: '0'}}>
                  <strong>[ALL CONTACT DETAILS TO BE CONFIRMED PRIOR TO LAUNCH]</strong>
                </p>
              </div>
            </div>
          </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default MembershipPage;
