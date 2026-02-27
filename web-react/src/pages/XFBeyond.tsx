import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';
import './XFBeyond.css';

const REBELS = [
    { role: 'King', name: 'Toussaint Louverture', desc: 'High-ranking revolutionary officer in a captured French General\'s frock coat with a West African headwrap beneath a bicorne hat. Holds a ceremonial saber with a dignified, strategic posture.' },
    { role: 'Queen', name: 'Santé Bélair', desc: 'Female revolutionary Sergeant in a military jacket and traditional Madras wrap skirt. Armed with a musket and machete — fierce grassroots leadership.' },
    { role: 'Bishop', name: 'Man Houngan', desc: 'Vodou spiritual leader in white ceremonial linen with gris-gris charms, holding a wooden cross or ritual staff. The spiritual catalyst of the 1791 uprising.' },
    { role: 'Knight', name: 'Maroon Guerrilla', desc: 'Riding a lean, unarmored workhorse in rugged torn trousers and bare feet. Armed with a long machete — agile and unpredictable against French cavalry.' },
    { role: 'Rook', name: 'Caimite / Fortified Morne', desc: 'A rebel lookout or stone fortification echoing Citadelle Laferrière. Heavy wooden palisade or a captured bronze cannon on a crude stone base.' },
    { role: 'Pawn', name: 'Field Insurgent', desc: 'Common laborer turned soldier in straw hat and plantation rags, wielding a machete, sharpened tool, or musket. Slight headwear variations mark each pawn\'s irregular nature.' },
];

const FRENCH = [
    { role: 'King', name: 'General Charles Leclerc', desc: 'Napoleonic General in a deep blue wool frock coat with gold epaulettes and a large bicorne hat. Holds a telescope or ceremonial rapier — upright, commanding, imperial.' },
    { role: 'Queen', name: 'Aristocratic Figure', desc: 'High-status figure in a silk Empire-waist dress or Marianne allegory of the French State. Holds a handheld fan or scepter bearing the "RF" (République Française) crest.' },
    { role: 'Bishop', name: 'Colonial Catholic Chaplain', desc: 'A priest in a black cassock with white clerical collar, holding a leather-bound Bible or silver crucifix. Represents the institutional Church backing the colonial order.' },
    { role: 'Knight', name: 'French Dragoon', desc: 'Mounted on a groomed European warhorse with a brass helmet and long horsehair plume. Armed with a straight heavy cavalry saber — power and discipline made visible.' },
    { role: 'Rook', name: 'Coastal Bastion', desc: 'A square stone tower with crenellations, typical of French military architecture in Le Cap. Heavy iron cannon protruding from a stone embrasure with clean-cut masonry.' },
    { role: 'Pawn', name: 'Line Infantry (Grenadier)', desc: 'Standard "Bluecoat" in a tall shako hat, white cross-belts, and a blue coat with red facings. Armed with a Charleville musket and fixed bayonet. All pawns identical — military uniformity.' },
];

const XFBeyond = () => {
    return (
        <motion.div
            className="xfbeyond-page"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -20 }}
            transition={{ duration: 0.5 }}
        >
            <Link to="/" className="back-btn" style={{ display: 'inline-flex', alignItems: 'center', gap: '0.5rem', color: '#e63946', textDecoration: 'none', fontWeight: 'bold', marginBottom: '2rem' }}>
                <ArrowLeft size={18} /> Back
            </Link>

            <header className="xfbeyond-header">
                <h1>Next Steps</h1>
                <p>Historically-themed 3D chess sets for competitive wagering of money and unique board assets.</p>
            </header>

            {/* VISION */}
            <section className="concept-introduction">
                <h2>The Vision</h2>
                <p>
                    XFChess Beyond expands the traditional chess experience by introducing historically-themed 3D chess sets where each piece represents significant figures and concepts from pivotal moments in history. These aren't just chess games — they're immersive experiences where players engage in competitive wagering while exploring historical narratives through gameplay. Each season introduces a new conflict, new factions, and new on-chain assets to own and trade.
                </p>
            </section>

            {/* SEASON 1 BOX */}
            <section className="beyond-season-box">
                <div className="beyond-season-label">Season 1</div>
                <h2 className="beyond-season-title">Haitian Revolt</h2>
                <p className="beyond-season-intro">
                    The first season brings to life the dramatic conflict between French colonial forces and Haitian slave rebels — one of history's most remarkable revolutions. Each piece is a detailed, animated 3D model drawn from the real figures and symbols of the 1791 uprising and the Haitian War of Independence.
                </p>

                <div className="beyond-factions">
                    {/* REBELS */}
                    <div className="beyond-faction">
                        <div className="beyond-faction-header beyond-faction-header--rebels">
                            Haitian Slave Rebels
                        </div>
                        <div className="beyond-piece-list">
                            {REBELS.map(p => (
                                <div className="beyond-piece-row" key={p.role}>
                                    <div className="beyond-piece-role">{p.role}</div>
                                    <div className="beyond-piece-detail">
                                        <span className="beyond-piece-name">{p.name}</span>
                                        <span className="beyond-piece-desc">{p.desc}</span>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>

                    {/* FRENCH */}
                    <div className="beyond-faction">
                        <div className="beyond-faction-header beyond-faction-header--french">
                            French Forces
                        </div>
                        <div className="beyond-piece-list">
                            {FRENCH.map(p => (
                                <div className="beyond-piece-row" key={p.role}>
                                    <div className="beyond-piece-role">{p.role}</div>
                                    <div className="beyond-piece-detail">
                                        <span className="beyond-piece-name">{p.name}</span>
                                        <span className="beyond-piece-desc">{p.desc}</span>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>
                </div>
            </section>

            {/* TECHNOLOGY */}
            <section className="technology-section">
                <h2>Technology</h2>
                <p>Built with the <a href="https://bevy.org/" target="_blank" rel="noopener noreferrer">Bevy game engine</a>. Each piece features:</p>
                <ul>
                    <li>Subtle loopable idle animations (4-second cycles)</li>
                    <li>Smooth movement and transition animations (1-second cycles)</li>
                    <li>Attack and capture animations</li>
                    <li>Full PBR textures at 2K resolution</li>
                    <li>OBJ/GLTF export for compatibility with major game engines</li>
                </ul>
            </section>

            {/* FUTURE */}
            <section className="future-vision">
                <h2>Expanding Horizons</h2>
                <p>The Haitian Revolt season is just the beginning. Future seasons will explore other pivotal moments in history:</p>
                <ul>
                    <li>Medieval warfare: Knights vs. Saracens</li>
                    <li>Ancient civilizations: Romans vs. Carthaginians</li>
                    <li>Revolutionary periods: American Revolution, French Revolution</li>
                    <li>Fantasy adaptations: Mythological creatures and legendary heroes</li>
                </ul>
            </section>
        </motion.div>
    );
};

export default XFBeyond;
