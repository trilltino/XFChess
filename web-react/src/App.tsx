import { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowLeft, Shield, Award, Trophy, Brain, Layout, Maximize, Github } from 'lucide-react';
import { HashRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import CodeViewer from './components/CodeViewer';
import ContractsPage from './pages/Contracts';
import Multiplayer from './pages/Multiplayer';
import Demo from './pages/Demo';
import XFBeyond from './pages/XFBeyond';
import Android from './pages/Android';
import MagicBlockPage from './pages/MagicBlock';
import tinoPhoto from './assets/tino.webp';

// ═══════════════════════════════════════
// UI COMPONENTS
// ═══════════════════════════════════════

const Navbar = () => {
  return (
    <nav className="navbar">
      <Link to="/" className="nav-logo">
        <span className="xf">XF</span>Chess.
      </Link>
      <div className="nav-links">
        <Link to="/demo" className="nav-link">Demo</Link>

        {/* 3 W's Dropdown */}
        <div className="dropdown">
          <span className="nav-link dropdown-toggle">3 W's</span>
          <div className="dropdown-menu">
            <Link to="/who" className="dropdown-item">Who</Link>
            <Link to="/what" className="dropdown-item">What</Link>
            <Link to="/why" className="dropdown-item">Why</Link>
          </div>
        </div>

        {/* Roadmap Dropdown */}
        <div className="dropdown">
          <span className="nav-link dropdown-toggle">Roadmap</span>
          <div className="dropdown-menu">
            <Link to="/android" className="dropdown-item">Android</Link>
            <Link to="/beyond" className="dropdown-item">Beyond</Link>
          </div>
        </div>

        {/* Networking Dropdown */}
        <div className="dropdown">
          <span className="nav-link dropdown-toggle">Networking</span>
          <div className="dropdown-menu">
            <Link to="/multiplayer" className="dropdown-item">Multiplayer</Link>
            <Link to="/solana" className="dropdown-item">Solana</Link>
            <Link to="/magicblock" className="dropdown-item">MagicBlock</Link>
          </div>
        </div>

        <a href="https://github.com/trilltino/XFChess/releases" className="nav-link" target="_blank" rel="noreferrer">Download</a>
      </div>
    </nav>
  );
};

const CyclingHero = () => {
  const words = ['Chess.', 'PlayFriends.', 'PlayFamily.', 'PlayLocal.', 'PlayGlobal.'];
  const [index, setIndex] = useState(0);
  const [settled, setSettled] = useState(false);

  useEffect(() => {
    if (settled) return;
    const interval = setInterval(() => {
      setIndex((prev) => {
        if (prev === words.length - 1) {
          setSettled(true);
          return 0; // Back to Chess.
        }
        return prev + 1;
      });
    }, 2500); // Slowed down from 1200ms to 2500ms for better readability
    return () => clearInterval(interval);
  }, [settled, words.length]);

  return (
    <section className="landing">
      <div className="landing-title">
        <span className="xf">XF</span>
        <span className="cycling">
          <AnimatePresence mode="wait">
            <motion.span
              key={settled ? 'final' : index}
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -20 }}
              transition={{ duration: 0.4 }}
              className="cycling-word"
            >
              {settled ? 'Chess.' : words[index]}
            </motion.span>
          </AnimatePresence>
        </span>
        {!settled && <span className="cursor" />}
      </div>
      <div className="landing-subtitle">Competitive Chess</div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: settled ? 1 : 0, y: settled ? 0 : 10 }}
        className="landing-tagline"
      >
        <span>Play Anywhere. Own your History.</span>
      </motion.div>
    </section>
  );
};


const LandingPage = () => {
  return (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
      <CyclingHero />
    </motion.div>
  );
};



const WhatPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <p><strong>XFChess</strong> is a revolutionary 3D chess experience built on <strong>Solana</strong>, combining stunning visuals with blockchain technology. The game is completely <strong>free and open source</strong>, allowing anyone to verify the fairness of every move while enjoying a premium chess experience.</p>

        <div className="divider" />

        <div className="section-label">3D Chess Experience</div>
        <h2>Immersive <span className="accent">Visual Gameplay.</span></h2>

        <p>XFChess brings chess to life with <strong>stunning 3D visualization</strong> powered by the Bevy Engine. Experience classic chess with dynamic lighting, realistic shadows, and smooth piece animations that make every move feel tangible and satisfying. The 3D environment creates an immersive atmosphere that traditional 2D boards simply cannot match.</p>

        <div className="features-grid">
          <div className="feature-card">
            <div className="f-icon"><Layout size={32} color="#e63946" /></div>
            <h3>3D Visualization</h3>
            <p>Stunning 3D environments built with the Bevy Engine. Experience chess with dynamic lighting, shadows, and smooth piece animations.</p>
          </div>

          <div className="feature-card">
            <div className="f-icon"><Brain size={32} color="#e63946" /></div>
            <h3>Stockfish Engine</h3>
            <p>Integrated Stockfish AI for training and analysis. Challenge an engine that adapts to your skill level, up to Grandmaster strength.</p>
          </div>

          <div className="feature-card">
            <div className="f-icon"><Maximize size={32} color="#e63946" /></div>
            <h3>Move Validation</h3>
            <p>Comprehensive move validation logic ensuring 100% adherence to FIDE rules, including en passant, castling, and stalemate detection.</p>
          </div>
        </div>

        <div className="divider" />

        <div className="section-label">Blockchain Integration</div>
        <h2>Truly <span className="accent">Transparent.</span></h2>

        <p>Every game played in XFChess is permanently recorded on the <strong>Solana blockchain</strong>, providing complete transparency and verifiable fairness. Your ELO rating, match history, and achievements are securely stored in Program Derived Addresses (PDAs), ensuring they remain accessible and tamper-proof forever. This blockchain integration ensures that all games are provably fair and your progress is permanently secured.</p>

        <p>As an open-source project, the entire codebase is available for inspection, modification, and contribution. We believe that transparency builds trust, and our open approach allows the community to verify every aspect of the game's functionality.</p>

        <div className="features-grid">
          <div className="feature-card">
            <div className="f-icon"><Shield size={32} color="#e63946" /></div>
            <h3>Fully Transparent</h3>
            <p>All moves are permanently recorded on-chain via the XFChess Game Program. Complete visibility into game history and player records.</p>
          </div>

          <div className="feature-card">
            <div className="f-icon"><Award size={32} color="#e63946" /></div>
            <h3>Secure Player Profiles</h3>
            <p>Your ELO, match history, and achievements are stored in secure Program Derived Addresses (PDAs), accessible globally.</p>
          </div>

          <div className="feature-card">
            <div className="f-icon"><Github size={32} color="#e63946" /></div>
            <h3>Open Source</h3>
            <p>Completely free and open source. Anyone can inspect, modify, and contribute to the codebase.</p>
          </div>
        </div>

        <div className="divider" />

        <div className="section-label">Technical Architecture</div>
        <h2>Built for <span className="accent">reliability.</span></h2>

        <p>XFChess leverages the power of Solana to create a unique chess experience that balances performance with security. Built with the <strong>Anchor framework</strong>, our on-chain programs ensure every move is verified and permanently recorded. The game combines Rust-based backend systems with cutting-edge blockchain technology to deliver an unparalleled chess experience.</p>

        <p>The architecture enables seamless synchronization between players, whether in local multiplayer or on-chain competitive modes. Each game state is carefully validated before being committed to the blockchain, ensuring the highest level of integrity for competitive play.</p>

        <div className="technical-snippets">
          <div className="code-caption">
            <h3>On-Chain Move Recording</h3>
            <p>Each move is validated and recorded on-chain via Anchor. The handler checks turn order, verifies the signer matches the expected player, updates the FEN string representing board state, and appends the algebraic notation to an immutable move log PDA.</p>
          </div>
          <CodeViewer
            title="xfchess_game/instructions/record_move.rs"
            language="Rust (Anchor)"
            code={`#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecordMove<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()], bump)]
    pub move_log: Account<'info, MoveLog>,
    pub player: Signer<'info>,
}

pub fn handler(ctx: Context<RecordMove>, _game_id: u64, move_str: String, next_fen: String) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;

    require!(game.status == GameStatus::Active, GameErrorCode::GameNotActive);

    // Turn and Identity Validation
    if game.turn % 2 != 0 {
        require!(ctx.accounts.player.key() == game.white, GameErrorCode::NotPlayerTurn);
    } else {
        require!(ctx.accounts.player.key() == game.black, GameErrorCode::NotPlayerTurn);
    }

    game.fen = next_fen;
    game.move_count += 1;
    game.turn += 1;
    move_log.moves.push(move_str);

    Ok(())
}`}
          />
        </div>

        <p>The combination of <strong>3D visualization</strong>, <strong>blockchain verification</strong>, and <strong>open-source transparency</strong> makes XFChess the premier choice for players who value both an engaging experience and complete fairness in their chess games.</p>
      </section>
    </motion.div>
  );
};

const WhoPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>
        <div className="section-label">The Builder</div>
        <h2>Hi, I'm <span className="accent">Tino.</span></h2>

        <div className="who-profile">
          <div className="who-photo-wrap">
            <img src={tinoPhoto} alt="Tino — Open Source Software Engineer" className="who-photo" />
          </div>
          <div className="who-bio">
            <p>I'm an <strong>Open Source Software Engineer</strong> specializing in <strong>Rust</strong> for distributed systems and blockchain infrastructure. I build full-stack applications using <strong>Leptos</strong>, <strong>Axum</strong>, and <strong>Tauri</strong>, with focus on P2P protocols and decentralized architecture.</p>

            <p>After graduating in <strong>Law from the University of Warwick</strong>, I pivoted to software engineering and taught myself Rust through building real systems. I've contributed to <strong>IETF protocol implementations</strong>, built P2P networking layers, and developed <strong>Solana smart contracts</strong>.</p>

            <p>Currently active in <strong>Solana Superteam UK</strong>, where I contribute to ecosystem growth through developer education and open-source tooling. My work spans protocol-level engineering, DeFi infrastructure, and making decentralized technologies more accessible to developers worldwide.</p>

            <div className="who-links">
              <a
                href="https://github.com/trilltino"
                className="who-link-btn"
                target="_blank"
                rel="noreferrer"
                aria-label="GitHub"
              >
                <Github size={18} />
                GitHub
              </a>
              <a
                href="https://www.linkedin.com/in/valentine-i-b0619b2b6/"
                className="who-link-btn who-link-btn--linkedin"
                target="_blank"
                rel="noreferrer"
                aria-label="LinkedIn"
              >
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433a2.062 2.062 0 0 1-2.063-2.065 2.064 2.064 0 1 1 2.063 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z" />
                </svg>
                LinkedIn
              </a>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

const WhyPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>
        <div className="section-label">Purpose</div>
        <h2>Why <span className="accent">XFChess?</span></h2>

        {/* REASON 1 — EDUCATION */}
        <div className="why-reason">
          <div className="why-reason-header">
            <div className="why-reason-icon">
              <Brain size={32} color="#e63946" />
            </div>
            <div>
              <div className="section-label" style={{ marginBottom: '0.25rem' }}>Reason One</div>
              <h3 className="why-reason-title">Chess Education & Building Blockchain Games</h3>
            </div>
          </div>
          <div className="why-prose">
            <p>Chess is one of the oldest and most studied strategy games in human history — a discipline that sharpens pattern recognition, long-term planning, and decision-making under pressure. Yet for most people, learning chess remains a solitary, opaque experience: you play, you lose, and you rarely understand why. XFChess changes that by embedding education directly into the game loop. Every match is permanently recorded on-chain, giving players a tamper-proof archive of their own games to study, share, and learn from. Integrated Stockfish analysis means post-game review is always available, surfacing the critical moments where a game was won or lost.</p>
            <p>Beyond the player experience, XFChess is itself a blueprint for how to build a serious blockchain game. The codebase is fully open-source — the Anchor smart contracts, the Bevy game client, the Braid networking layer, and the Iroh P2P transport are all available for developers to read, fork, and build upon. There is a severe shortage of well-documented, production-quality examples of blockchain game development. XFChess is designed to fill that gap: a real game, built with real infrastructure, that other developers can learn from and extend. Every architectural decision — from the ECS pattern used on-chain to the way advanced networking and on-chain processing are integrated — is an opportunity to demonstrate what thoughtful blockchain game engineering looks like in practice.</p>
          </div>
        </div>

        <div className="divider" />

        {/* REASON 2 — FINANCIALISED GAMING */}
        <div className="why-reason">
          <div className="why-reason-header">
            <div className="why-reason-icon">
              <Trophy size={32} color="#e63946" />
            </div>
            <div>
              <div className="section-label" style={{ marginBottom: '0.25rem' }}>Reason Two</div>
              <h3 className="why-reason-title">Improving Financialised Gaming on Solana</h3>
            </div>
          </div>
          <div className="why-prose">
            <p>Financialised gaming on Solana has a credibility problem. The space is littered with projects that bolt a token onto a shallow game loop, extract value from players, and disappear. The result is a justified scepticism among both gamers and developers: the idea that on-chain economies and genuine gameplay are fundamentally incompatible. XFChess exists to challenge that assumption directly. Chess is a game of pure skill — there is no randomness, no pay-to-win mechanic, no artificial scarcity. When you wager SOL on a match in XFChess, the outcome is determined entirely by the quality of your play, enforced by smart contracts that neither player can manipulate.</p>
            <p>The financial layer in XFChess is designed to be minimal, transparent, and fair. Wager escrows are governed by Anchor programs that release funds automatically on game conclusion — no intermediary, no withdrawal delays, no hidden fees. ELO ratings are stored in Program Derived Addresses and updated on-chain after every match, creating a reputation system that is globally verifiable and impossible to fake. The goal is not to make chess a vehicle for speculation, but to demonstrate that skill-based, financially-settled games can be built on Solana in a way that respects players and strengthens the ecosystem's reputation. If XFChess succeeds, it becomes a reference point for what responsible financialised gaming looks like — and a foundation that other developers can build on.</p>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

// ═══════════════════════════════════════
// MAIN APP COMPONENT
// ═══════════════════════════════════════

const App = () => {
  const location = useLocation();

  return (
    <div className="app-container">
      <div className="grid-bg" />
      <Navbar />
      <main>
        <AnimatePresence mode="wait">
          <Routes location={location} key={location.pathname}>
            <Route path="/" element={<LandingPage />} />
            <Route path="/what" element={<WhatPage />} />
            <Route path="/who" element={<WhoPage />} />
            <Route path="/why" element={<WhyPage />} />
            <Route path="/solana" element={<ContractsPage />} />
            <Route path="/contracts" element={<ContractsPage />} />
            <Route path="/multiplayer" element={<Multiplayer />} />
            <Route path="/magicblock" element={<MagicBlockPage />} />
            <Route path="/demo" element={<Demo />} />
            <Route path="/android" element={<Android />} />
            <Route path="/beyond" element={<XFBeyond />} />
          </Routes>
        </AnimatePresence>
      </main>

      <footer className="footer">

      </footer>
    </div>
  );
};

const Root = () => (
  <Router>
    <App />
  </Router>
);

export default Root;
