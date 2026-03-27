import { motion } from 'framer-motion';
import { ArrowLeft, Users, Trophy, DollarSign, Palette, Star, Mail, Calendar, Target, Handshake, Award, TrendingUp } from 'lucide-react';
import { Link } from 'react-router-dom';

const ClubOfferPage = () => {
  // Partnership tiers and benefits
  const partnershipTiers = [
    {
      name: "Bronze Partner",
      fee: "£2.50/tournament",
      features: [
        "Basic tournament hosting platform",
        "Standard XFChess branding",
        "Email support",
        "Monthly analytics report",
        "Access to 100+ player base"
      ],
      idealFor: "Local clubs starting with online tournaments"
    },
    {
      name: "Silver Partner", 
      fee: "£5.00/tournament",
      features: [
        "Advanced tournament features",
        "Co-branded tournaments",
        "Priority support",
        "Weekly analytics & insights",
        "Access to 500+ player base",
        "Custom tournament rules",
        "Player recruitment tools"
      ],
      idealFor: "Regional clubs with regular tournaments"
    },
    {
      name: "Gold Partner",
      fee: "£10.00/tournament",
      features: [
        "White-label tournament platform",
        "Full club branding",
        "Dedicated account manager",
        "Real-time analytics dashboard",
        "Access to 2000+ player base",
        "Custom board & piece designs",
        "Revenue sharing (70/30 split)",
        "Annual charity event support"
      ],
      idealFor: "National clubs and chess federations"
    }
  ];

  // Creator onboarding steps
  const creatorSteps = [
    {
      step: 1,
      title: "Design Your Assets",
      description: "Create high-quality 3D models of chess boards and pieces following our technical specifications.",
      requirements: [
        "Board: 2048x2048 texture, PBR materials",
        "Pieces: Individual 3D models, 1024x1024 textures",
        "File formats: .glb, .fbx, or .obj",
        "Poly count: Boards <10K, Pieces <2K each"
      ],
      tools: ["Blender", "Maya", "3ds Max", "Cinema 4D"]
    },
    {
      step: 2,
      title: "Submit for Review",
      description: "Upload your designs through our creator portal for quality and performance testing.",
      reviewCriteria: [
        "Visual quality and detail",
        "Performance optimization",
        "Originality and creativity",
        "Technical compliance",
        "Licensing verification"
      ],
      timeline: "2-3 business days"
    },
    {
      step: 3,
      title: "Set Pricing & Royalties",
      description: "Choose your pricing model and royalty structure. We handle payments and distribution.",
      pricingOptions: [
        "Fixed price sales (70% creator, 30% platform)",
        "Subscription access (60% creator, 40% platform)",
        "Tournament exclusives (50% creator, 50% platform)",
        "Charity collaborations (80% creator, 20% platform)"
      ]
    },
    {
      step: 4,
      title: "Launch & Promote",
      description: "Go live on the XFChess marketplace and access our player community.",
      marketingSupport: [
        "Featured placement in new releases",
        "Social media promotion",
        "Tournament spotlight opportunities",
        "Creator profile and portfolio",
        "Analytics and sales dashboard"
      ]
    }
  ];

  // Sample outreach script
  const outreachScript = {
    subject: "Partnership Opportunity: XFChess × [Club Name] - Revolutionary Chess Platform",
    introduction: `Dear [Club President/Tournament Director],

I'm reaching out from XFChess, the world's first blockchain chess platform offering sub-second gameplay with true digital ownership. We're seeking strategic partnerships with prestigious European chess clubs to launch our charity tournament initiative.

Our platform combines the speed of traditional chess apps with the security and ownership of blockchain technology, creating a unique competitive experience for your members.`,
    valueProposition: `Why Partner with XFChess?

• **Zero Platform Fees**: Unlike other platforms, we operate on a simple subscription model - your club keeps 100% of tournament entry fees
• **Sub-Second Gameplay**: MagicBlock Ephemeral Rollups enable real-time chess with ~200ms move latency
• **True Digital Ownership**: Players own their game history, ratings, and collectible chess sets
• **Charity Impact**: Host charity tournaments with transparent fund tracking on Solana blockchain
• **Global Player Base**: Access our growing community of 2000+ active players across Europe`,
    callToAction: `I'd love to schedule a 15-minute call to discuss how we can create a mutually beneficial partnership. Our Gold Partnership tier includes:
- White-label tournament platform with your club branding
- Revenue sharing on marketplace sales (70/30 split in your favor)
- Dedicated support for organizing charity events
- Access to our advanced tournament management tools

Are you available for a brief conversation next week? I'm flexible with timing and can demonstrate the platform live.

Best regards,
[Your Name]
Partnership Manager
XFChess`,
    followUp: `P.S. We're launching our European charity tournament series next month and have 3 partnership slots remaining. This is an excellent opportunity to position your club at the forefront of chess innovation while supporting meaningful causes.`
  };

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Partnership</div>
        <h2>Chess Club <span className="accent">Partnership Program</span></h2>

        <div className="page-hero">
          <h3>Revolutionize Your Chess Tournaments</h3>
          <p className="page-hero-subtitle">
            Partner with XFChess to host cutting-edge tournaments, earn revenue, and lead the future of competitive chess. 
            Zero platform fees, sub-second gameplay, and true digital ownership.
          </p>
          <div className="btn-group">
            <Link to="/early-access" className="btn btn-primary">
              <Users size={16} />
              Apply for Partnership
            </Link>
            <a href="mailto:partnerships@xfchess.com" className="btn btn-secondary">
              <Mail size={16} />
              Contact Us
            </a>
          </div>
        </div>

        <div className="page-section">
          <h3><Trophy size={24} /> Partnership Tiers</h3>
          <p className="page-hero-subtitle">
            Choose the partnership level that best fits your club's ambitions and tournament schedule.
          </p>
          <div className="grid-3">
            {partnershipTiers.map((tier, index) => (
              <div key={index} className={`card ${tier.name === 'Gold Partner' ? 'card-centered' : ''}`}>
                <div className="card-header">
                  <h4 className="card-title">{tier.name}</h4>
                  <p className="card-subtitle">{tier.fee}</p>
                </div>
                <ul className="list-check">
                  {tier.features.map((feature, idx) => (
                    <li key={idx}>{feature}</li>
                  ))}
                </ul>
                <p className="card-content" style={{marginTop: '16px', fontStyle: 'italic'}}>
                  {tier.idealFor}
                </p>
              </div>
            ))}
          </div>
        </div>

        <div className="page-section">
          <h3><Handshake size={24} /> Partnership Benefits</h3>
          <div className="grid-2">
            <div className="card">
              <div className="card-icon"><DollarSign size={24} /></div>
              <div className="card-header">
                <h4 className="card-title">Revenue Opportunities</h4>
              </div>
              <ul className="list-check">
                <li>Keep 100% of tournament entry fees</li>
                <li>70% revenue share on marketplace sales</li>
                <li>Charity tournament hosting capabilities</li>
                <li>Custom tournament entry pricing</li>
              </ul>
            </div>
            <div className="card">
              <div className="card-icon"><Users size={24} /></div>
              <div className="card-header">
                <h4 className="card-title">Community Growth</h4>
              </div>
              <ul className="list-check">
                <li>Access to 2000+ active players</li>
                <li>Global tournament visibility</li>
                <li>Player recruitment tools</li>
                <li>Club branding opportunities</li>
              </ul>
            </div>
            <div className="card">
              <div className="card-icon"><Target size={24} /></div>
              <div className="card-header">
                <h4 className="card-title">Advanced Features</h4>
              </div>
              <ul className="list-check">
                <li>White-label tournament platform</li>
                <li>Real-time analytics dashboard</li>
                <li>Custom tournament rules</li>
                <li>Dedicated account management</li>
              </ul>
            </div>
            <div className="card">
              <div className="card-icon"><Award size={24} /></div>
              <div className="card-header">
                <h4 className="card-title">Prestige & Innovation</h4>
              </div>
              <ul className="list-check">
                <li>First-mover advantage in blockchain chess</li>
                <li>Association with cutting-edge technology</li>
                <li>Charity impact and social responsibility</li>
                <li>Media and press opportunities</li>
              </ul>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3><Mail size={24} /> Outreach Script & Pitch</h3>
          <div className="card">
            <div className="card-header">
              <h4 className="card-title">Email Template for Chess Clubs</h4>
            </div>
            <div className="card-content">
              <div className="table-container">
                <div className="table-header">
                  <h4>Email Components</h4>
                </div>
                <table className="table">
                  <thead>
                    <tr>
                      <th>Section</th>
                      <th>Content</th>
                      <th>Purpose</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr>
                      <td className="table-highlight">Subject</td>
                      <td>{outreachScript.subject}</td>
                      <td>Grab attention, establish credibility</td>
                    </tr>
                    <tr>
                      <td className="table-highlight">Introduction</td>
                      <td>{outreachScript.introduction}</td>
                      <td>Introduce XFChess and partnership opportunity</td>
                    </tr>
                    <tr>
                      <td className="table-highlight">Value Prop</td>
                      <td>{outreachScript.valueProposition}</td>
                      <td>Highlight key benefits and differentiators</td>
                    </tr>
                    <tr>
                      <td className="table-highlight">Call to Action</td>
                      <td>{outreachScript.callToAction}</td>
                      <td>Request meeting and provide specific next steps</td>
                    </tr>
                    <tr>
                      <td className="table-highlight">P.S.</td>
                      <td>{outreachScript.followUp}</td>
                      <td>Create urgency and reinforce value</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3><Palette size={24} /> Creator Onboarding Guide</h3>
          <p className="page-hero-subtitle">
            Empower 3D artists to create and monetize chess assets on the XFChess marketplace.
          </p>
          <div className="steps-list">
            {creatorSteps.map((step, index) => (
              <div key={index} className="step-item">
                <div className="step-number">{step.step}</div>
                <div className="step-content">
                  <h4>{step.title}</h4>
                  <p>{step.description}</p>
                  {step.requirements && (
                    <div className="card" style={{marginTop: '16px'}}>
                      <div className="card-header">
                        <h5 className="card-title">Requirements</h5>
                      </div>
                      <ul className="list-check">
                        {step.requirements.map((req, idx) => (
                          <li key={idx}>{req}</li>
                        ))}
                      </ul>
                    </div>
                  )}
                  {step.tools && (
                    <div className="card" style={{marginTop: '16px'}}>
                      <div className="card-header">
                        <h5 className="card-title">Recommended Tools</h5>
                      </div>
                      <div className="grid-auto">
                        {step.tools.map((tool, idx) => (
                          <div key={idx} className="tech-item">
                            <strong>{tool}</strong>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                  {step.reviewCriteria && (
                    <div className="card" style={{marginTop: '16px'}}>
                      <div className="card-header">
                        <h5 className="card-title">Review Criteria</h5>
                      </div>
                      <ul className="list-check">
                        {step.reviewCriteria.map((criteria, idx) => (
                          <li key={idx}>{criteria}</li>
                        ))}
                      </ul>
                      <p className="card-subtitle" style={{marginTop: '8px'}}>
                        <strong>Timeline:</strong> {step.timeline}
                      </p>
                    </div>
                  )}
                  {step.pricingOptions && (
                    <div className="card" style={{marginTop: '16px'}}>
                      <div className="card-header">
                        <h5 className="card-title">Pricing Options</h5>
                      </div>
                      <ul className="list-check">
                        {step.pricingOptions.map((option, idx) => (
                          <li key={idx}>{option}</li>
                        ))}
                      </ul>
                    </div>
                  )}
                  {step.marketingSupport && (
                    <div className="card" style={{marginTop: '16px'}}>
                      <div className="card-header">
                        <h5 className="card-title">Marketing Support</h5>
                      </div>
                      <ul className="list-check">
                        {step.marketingSupport.map((support, idx) => (
                          <li key={idx}>{support}</li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="page-section">
          <h3><Star size={24} /> Quality Standards</h3>
          <div className="grid-2">
            <div className="card">
              <div className="card-header">
                <h4 className="card-title">Technical Requirements</h4>
              </div>
              <ul className="list-check">
                <li>Board textures: 2048x2048 minimum resolution</li>
                <li>Piece models: Optimized under 2K polygons each</li>
                <li>PBR materials with metallic/roughness maps</li>
                <li>Consistent UV unwrapping for texture mapping</li>
                <li>LOD models for performance optimization</li>
                <li>File formats: .glb, .fbx, .obj with textures</li>
              </ul>
            </div>
            <div className="card">
              <div className="card-header">
                <h4 className="card-title">Artistic Standards</h4>
              </div>
              <ul className="list-check">
                <li>Original designs - no copyrighted material</li>
                <li>Consistent art style within themed sets</li>
                <li>High-quality textures with proper mipmapping</li>
                <li>Realistic or stylized with clear visual hierarchy</li>
                <li>Proper contrast for visibility during gameplay</li>
                <li>Cultural sensitivity and historical accuracy</li>
              </ul>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3><TrendingUp size={24} /> Success Metrics</h3>
          <div className="grid-3">
            <div className="card card-centered">
              <div className="card-header">
                <h4 className="card-title">Club KPIs</h4>
              </div>
              <ul className="list-check">
                <li>Tournament participation rate</li>
                <li>Member retention improvement</li>
                <li>Revenue from tournament fees</li>
                <li>Charity funds raised</li>
              </ul>
            </div>
            <div className="card card-centered">
              <div className="card-header">
                <h4 className="card-title">Creator KPIs</h4>
              </div>
              <ul className="list-check">
                <li>Monthly sales volume</li>
                <li>Customer satisfaction ratings</li>
                <li>Asset download numbers</li>
                <li>Royalty earnings</li>
              </ul>
            </div>
            <div className="card card-centered">
              <div className="card-header">
                <h4 className="card-title">Platform KPIs</h4>
              </div>
              <ul className="list-check">
                <li>Active tournament count</li>
                <li>Creator onboarding rate</li>
                <li>Marketplace transaction volume</li>
                <li>User engagement metrics</li>
              </ul>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3><Calendar size={24} /> Next Steps</h3>
          <div className="btn-group">
            <Link to="/early-access" className="btn btn-primary">
              <Handshake size={16} />
              Apply for Partnership
            </Link>
            <a href="mailto:creators@xfchess.com" className="btn btn-secondary">
              <Palette size={16} />
              Creator Portal Access
            </a>
            <a href="https://github.com/trilltino/XFChess" target="_blank" rel="noopener noreferrer" className="btn btn-secondary">
              <Star size={16} />
              Technical Documentation
            </a>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default ClubOfferPage;
