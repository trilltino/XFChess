import { motion } from 'framer-motion';
import { ArrowLeft, Heart, Globe, Trophy, Users } from 'lucide-react';
import { Link } from 'react-router-dom';
import './XFBeyond.css';

const Charity = () => {
    return (
        <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="contracts-page page-overlay"
        >
            <Link
                to="/"
                className="back-btn"
                style={{
                    position: 'absolute',
                    top: '2rem',
                    left: '2rem',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '0.5rem',
                    color: '#e63946',
                    textDecoration: 'none',
                    fontWeight: 'bold'
                }}
            >
                <ArrowLeft size={18} /> Back
            </Link>

            <header className="contracts-header">
                <div className="section-label" style={{ color: '#e63946', fontSize: '0.75rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', marginBottom: '0.75rem' }}>Financialised Layer</div>
                <h1>Charity <span className="accent">Events.</span></h1>
                <p>Competitive gaming with social impact—where every match contributes to meaningful causes worldwide.</p>
            </header>

            <section className="architecture-overview">
                <h2>Gaming for Good</h2>
                <p>
                    XFChess Charity Events transform competitive chess into a force for positive change.
                    Players participate in organized tournaments where a portion of entry fees and wagers
                    goes directly to verified charitable organizations. From local community initiatives
                    to global humanitarian efforts, every game played contributes to making the world better.
                </p>

                <div className="contract-modules">
                    <div className="module-card" style={{ borderTop: '3px solid #e63946' }}>
                        <Heart size={32} color="#e63946" />
                        <h3>Direct Donations</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            A percentage of every wager and entry fee is automatically routed to charity
                            via smart contracts. Transparent on-chain records show exactly how much
                            each event raised.
                        </p>
                    </div>
                    <div className="module-card" style={{ borderTop: '3px solid #22c55e' }}>
                        <Globe size={32} color="#22c55e" />
                        <h3>Global Reach</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Support causes worldwide—from education initiatives in developing nations
                            to disaster relief, medical research, and environmental conservation.
                        </p>
                    </div>
                    <div className="module-card" style={{ borderTop: '3px solid #a855f7' }}>
                        <Trophy size={32} color="#a855f7" />
                        <h3>Charity Tournaments</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Special tournament formats where winners receive recognition and prizes,
                            while the majority of proceeds support the designated charitable cause.
                        </p>
                    </div>
                    <div className="module-card" style={{ borderTop: '3px solid #3b82f6' }}>
                        <Users size={32} color="#3b82f6" />
                        <h3>Community Events</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Local chess clubs and online communities can organize their own charity
                            events, choosing causes that matter to their members.
                        </p>
                    </div>
                </div>
            </section>

            <section className="competitive-features">
                <h2>How Charity Events Work</h2>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">01</div>
                        <div>
                            <h3>Event Creation</h3>
                            <p className="feature-subtitle">Organizers define the charitable cause and fee structure</p>
                        </div>
                    </div>
                    <p>
                        Tournament organizers partner with verified charitable organizations and configure
                        the event parameters. Smart contracts ensure funds are automatically distributed
                        according to the predetermined split—typically 70-90% to charity, remainder to prizes.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Verified Partners</h4>
                            <p>All charitable organizations are vetted and verified before being approved for events.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Transparent Splits</h4>
                            <p>Donation percentages are encoded in the smart contract and visible to all participants.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Automatic Distribution</h4>
                            <p>Funds transfer directly to charity wallets upon tournament conclusion—no intermediaries.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">02</div>
                        <div>
                            <h3>Event Types</h3>
                            <p className="feature-subtitle">Multiple formats for different charitable goals</p>
                        </div>
                    </div>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>High-Stakes Charity Classic</h4>
                            <p>Premium entry fees ($50+) with 80%+ going to charity. Top players win NFT trophies and recognition.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Community Fundraiser</h4>
                            <p>Low entry barriers ($1-5) designed for mass participation. Every player contributes, everyone wins.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Streamer Showdown</h4>
                            <p>Content creators host charity streams where viewers can donate and wager alongside matches.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Corporate Sponsorship</h4>
                            <p>Businesses sponsor tournaments, matching player donations up to specified limits.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">03</div>
                        <div>
                            <h3>Impact Tracking</h3>
                            <p className="feature-subtitle">On-chain transparency for charitable giving</p>
                        </div>
                    </div>
                    <p>
                        Every donation is recorded on-chain, creating a permanent, auditable record of charitable
                        contributions. Players can see exactly how much they've raised across their chess career,
                        and charities receive funds immediately without delays or administrative overhead.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Personal Impact Dashboard</h4>
                            <p>Players track their lifetime charitable contributions and see which causes they've supported.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Charity Leaderboards</h4>
                            <p>Recognition for top contributors, creating friendly competition around giving.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Receipt NFTs</h4>
                            <p>Participants receive commemorative NFTs documenting their contribution to each event.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">04</div>
                        <div>
                            <h3>Featured Causes</h3>
                            <p className="feature-subtitle">Examples of charities supported through XFChess events</p>
                        </div>
                    </div>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Education Initiatives</h4>
                            <p>Chess-in-schools programs, scholarships for underprivileged youth, and educational technology in developing regions.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Disaster Relief</h4>
                            <p>Rapid response funding for natural disasters, humanitarian crises, and emergency medical care.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Medical Research</h4>
                            <p>Funding for disease research, mental health initiatives, and healthcare access programs.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Environmental Conservation</h4>
                            <p>Climate action, wildlife preservation, and sustainable development projects worldwide.</p>
                        </div>
                    </div>
                </div>
            </section>
        </motion.div>
    );
};

export default Charity;
