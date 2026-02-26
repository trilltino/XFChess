import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './XFBeyond.css'; // Utilizing the black and red global Premium theme

const Contracts = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="contracts-page page-overlay">
      <Link to="/" className="back-btn" style={{ position: 'absolute', top: '2rem', left: '2rem', display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#e63946', textDecoration: 'none', fontWeight: 'bold' }}>
        <ArrowLeft size={18} /> Back
      </Link>

      <header className="contracts-header">
        <h1>XFChess Smart Contracts</h1>
        <p>Decentralized chess platform built on Solana with Trustless Wagering and Tournament Protocols.</p>
      </header>

      <section className="architecture-overview">
        <h2>Architecture Overview</h2>
        <p>
          XFChess abandons the standard monolithic approach in favor of a deeply compartmentalized,
          high-performance Solana workspace. State validation happens at various layers
          with final settlements and dispute resolutions executing on-chain for maximum security.
        </p>

        <div className="contract-modules">
          <div className="module-card">
            <h3>Game Engine Core</h3>
            <p>Handles cryptographic FEN parsing, move validation, and high-performance game state transitions.</p>
          </div>
          <div className="module-card">
            <h3>Wagering Protocol</h3>
            <p>Trustless escrowed wagering for PvP matches with automated on-chain settlements.</p>
          </div>
        </div>
      </section>

      <section className="competitive-features">
        <h2>Technical Implementations</h2>

        <div className="feature-section">
          <h3>Escrowed Wagering System</h3>
          <p>
            When a game is created, the wager amount is transferred into a Program Derived Address (PDA) escrow.
            This ensures funds are locked until the game concludes or reaches an expiration state.
          </p>
          <CodeViewer
            title="xfchess-game/src/instructions/create_game.rs"
            language="Rust"
            code={`// Securely route player funds to the Escrow PDA via System Program transfer
if wager_amount > 0 {
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.player.to_account_info(),
                to: ctx.accounts.escrow_pda.to_account_info(),
            },
        ),
        wager_amount,
    )?;
}`}
          />
        </div>

        <div className="feature-section">
          <h3>High-Performance Player Profiles</h3>
          <p>
            XFChess utilizes the high-performance PlayerProfile account to track achievements,
            Elo ratings, and match history directly on-chain.
          </p>
          <CodeViewer
            title="xfchess-game/src/state/player_profile.rs"
            language="Rust"
            code={`#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,
    pub elo: u16,
}`}
          />
        </div>

        <div className="feature-section">
          <h3>Solana Mobile Stack (SMS)</h3>
          <p>
            Native mobile support via the Solana Mobile Stack (SMS). The contracts are optimized for
            low-latency interaction via Mobile Wallet Adapter, ensuring high mobility for tournament players.
          </p>
        </div>
      </section>


    </motion.div >
  );
};

export default Contracts;