import { motion } from 'framer-motion';
import { ArrowLeft, ExternalLink, CheckCircle, Star, Zap, Shield, Clock } from 'lucide-react';
import { Link } from 'react-router-dom';

const EarlyAccessPage = () => {
  // Early access features
  const features = [
    {
      icon: <Zap size={24} />,
      title: "Sub-Second Moves",
      description: "MagicBlock Ephemeral Rollups process chess moves in milliseconds, not seconds."
    },
    {
      icon: <Shield size={24} />,
      title: "Zero House Cut",
      description: "Keep 100% of your winnings. We operate on a simple subscription model."
    },
    {
      icon: <Star size={24} />,
      title: "True Ownership",
      description: "Your game history, ratings, and achievements are permanently on Solana."
    },
    {
      icon: <Clock size={24} />,
      title: "24/7 Availability",
      description: "Play anytime, anywhere with our P2P network - no server downtime."
    }
  ];

  const feeStructure = [
    { service: "Platform Access", type: "Monthly Subscription", amount: "£4.99/month", notes: "Unlimited games, no per-game fees" },
    { service: "Wager Processing", type: "Transaction Fee", amount: "None", notes: "Players keep 100% of winnings" },
    { service: "Board Sales", type: "Marketplace Fee", amount: "5% of sale price", notes: "When selling custom chess boards" },
    { service: "Club Entry", type: "Tournament Fee", amount: "£2.50/tournament", notes: "Premium club tournaments" },
    { service: "Withdrawals", type: "Network Fee", amount: "Solana gas only", notes: "No platform withdrawal fees" }
  ];

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Early Access</div>
        <h2>Get Early Access to <span className="accent">XFChess</span></h2>

        <div className="page-hero">
          <h3>Chess at the Speed of Light</h3>
          <p className="page-hero-subtitle">
            Sub-second moves on Solana with MagicBlock Ephemeral Rollups. 
            Just £4.99/month. Zero wager fees. Keep 100% of winnings.
          </p>
          <div className="btn-group">
            <a 
              href="https://trilltino.github.io/XFChess/#/early-access" 
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-primary"
            >
              <ExternalLink size={16} />
              Join Early Access
            </a>
            <Link to="/test" className="btn btn-secondary">
              View Test Results
            </Link>
          </div>
        </div>

        <div className="page-section">
          <div className="grid-auto">
            {features.map((feature, index) => (
              <motion.div 
                key={index}
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: index * 0.1 }}
                className="card card-centered"
              >
                <div className="card-icon">{feature.icon}</div>
                <div className="card-header">
                  <h4 className="card-title">{feature.title}</h4>
                </div>
                <p className="card-content">{feature.description}</p>
              </motion.div>
            ))}
          </div>
        </div>

        <div className="page-section">
          <h3>Simple, Transparent Pricing</h3>
          <p className="page-hero-subtitle">
            No hidden fees. No percentage cuts. Just predictable pricing that works for both players and the platform.
          </p>

          <div className="table-container">
            <div className="table-header">
              <h4>UK Early Access Pricing</h4>
            </div>
            <table className="table">
              <thead>
                <tr>
                  <th>Service</th>
                  <th>Fee Type</th>
                  <th>Amount</th>
                  <th>Notes</th>
                </tr>
              </thead>
              <tbody>
                {feeStructure.map((fee, index) => (
                  <tr key={index}>
                    <td>{fee.service}</td>
                    <td>{fee.type}</td>
                    <td className={fee.amount === "None" ? "table-success" : "table-highlight"}>
                      {fee.amount}
                    </td>
                    <td>{fee.notes}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        <div className="page-section">
          <h3>Why Choose XFChess?</h3>
          <div className="grid-2">
            <div className="card">
              <CheckCircle size={20} className="card-icon" />
              <div className="card-header">
                <h4 className="card-title">For Players</h4>
              </div>
              <ul className="list-check">
                <li>Keep 100% of winnings (no house rake)</li>
                <li>Sub-second move confirmation</li>
                <li>Transparent fee structure</li>
                <li>On-chain prize guarantees</li>
              </ul>
            </div>
            <div className="card">
              <Star size={20} className="card-icon" />
              <div className="card-header">
                <h4 className="card-title">Platform Sustainability</h4>
              </div>
              <ul className="list-check">
                <li>Predictable revenue via subscriptions</li>
                <li>Zero marginal cost per game (P2P)</li>
                <li>Compliant with EU gaming regulations</li>
                <li>Scales without infrastructure costs</li>
              </ul>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3>Built on Modern Technology</h3>
          <div className="grid-auto">
            <div className="tech-item">
              <strong>Game Engine</strong>
              <span>Bevy 0.18 (Rust)</span>
            </div>
            <div className="tech-item">
              <strong>Blockchain</strong>
              <span>Solana Devnet</span>
            </div>
            <div className="tech-item">
              <strong>Smart Contracts</strong>
              <span>Anchor 0.32.1</span>
            </div>
            <div className="tech-item">
              <strong>Ephemeral Rollups</strong>
              <span>MagicBlock ER SDK</span>
            </div>
            <div className="tech-item">
              <strong>P2P Network</strong>
              <span>Iroh (QUIC) + Braid</span>
            </div>
            <div className="tech-item">
              <strong>Signing Server</strong>
              <span>Axum (Rust HTTP)</span>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3>Join the Revolution</h3>
          <p className="page-hero-subtitle">
            Be among the first to experience sub-second chess on Solana. 
            Help us shape the future of competitive gaming.
          </p>
          <div className="btn-group">
            <a 
              href="https://trilltino.github.io/XFChess/#/early-access" 
              target="_blank"
              rel="noopener noreferrer"
              className="btn btn-cta"
            >
              <ExternalLink size={16} />
              Complete Early Access Form
            </a>
          </div>
          <p className="page-hero-subtitle" style={{marginTop: '16px', fontSize: '0.9rem'}}>
            You'll be redirected to our secure early access form to capture your expected play frequency and wager preferences.
          </p>
        </div>

        <div className="page-section">
          <h3>What Happens Next?</h3>
          <div className="steps-list">
            <div className="step-item">
              <div className="step-number">1</div>
              <div className="step-content">
                <h4>Submit Early Access Form</h4>
                <p>Tell us about your chess background and expected play patterns.</p>
              </div>
            </div>
            <div className="step-item">
              <div className="step-number">2</div>
              <div className="step-content">
                <h4>Receive Invitation</h4>
                <p>Get early access credentials and download instructions.</p>
              </div>
            </div>
            <div className="step-item">
              <div className="step-number">3</div>
              <div className="step-content">
                <h4>Start Playing</h4>
                <p>Experience sub-second chess with real wager stakes.</p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default EarlyAccessPage;
