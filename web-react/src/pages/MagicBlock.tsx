import { motion } from 'framer-motion';
import { ArrowLeft, Zap, Shield, Network } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './XFBeyond.css'; // Using the same styling as other premium pages

const MagicBlockPage = () => {
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
        <div className="section-label" style={{ color: '#e63946', fontSize: '0.75rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', marginBottom: '0.75rem' }}>Infrastructure</div>
        <h1>Accelerated by <span className="accent">MagicBlock.</span></h1>
        <p>XFChess leverages MagicBlock Ephemeral Rollups to achieve sub-second on-chain move results for competitive PvP.</p>
      </header>

      <section className="architecture-overview">
        <h2>The Ephemeral Rollup (ER) Stack</h2>
        <p>
          Ephemeral Rollups act as a decentralized second layer for high-speed state transitions.
          By delegating game accounts to high-performance validators, XFChess removes the 400ms "Solana speed limit"
          without sacrificing the security of the Base Layer.
        </p>

        <div className="contract-modules">
          <div className="module-card">
            <Zap size={28} color="#e63946" />
            <h3>PvP Advantage</h3>
            <p>Sub-second move confirmation creates a "fluid" feel identical to centralized platforms, but fully trustless.</p>
          </div>
          <div className="module-card">
            <Shield size={28} color="#a855f7" />
            <h3>Native Delegation</h3>
            <p>Smart contracts authorize temporary control to the ER layer via secure CPI calls.</p>
          </div>
          <div className="module-card">
            <Network size={28} color="#22c55e" />
            <h3>Instant Settlement</h3>
            <p>Game outcomes are committed back to Solana L1 only once the session is complete.</p>
          </div>
        </div>
      </section>

      <section className="competitive-features">
        <h2>Technical Implementation</h2>

        <div className="feature-section">
          <h3>1. Program Account Delegation</h3>
          <p>
            When a PvP game begins, the <code>xfchess-game</code> program authorises the
            delegation of the Game PDA to the MagicBlock network.
          </p>
          <CodeViewer
            title="xfchess-game/src/instructions/delegate_game.rs"
            language="Rust"
            code={`// Authorize account delegation to the Ephemeral Rollup validator
delegate_account(
    &ctx.accounts.payer.to_account_info(),
    &ctx.accounts.game.to_account_info(),
    &ctx.accounts.owner_program.to_account_info(),
    &ctx.accounts.buffer.to_account_info(),
    &ctx.accounts.delegation_record.to_account_info(),
    &seeds,
    valid_until,
    300_000, // Priority compute units for high-speed execution
)?;`}
          />
        </div>

        <div className="feature-section">
          <h3>2. High-Speed Batch Commits</h3>
          <p>
            Moves are processed in the ER and eventually batched and verified back to the
            main chain using cryptographic session signatures.
          </p>
          <CodeViewer
            title="xfchess-game/src/instructions/commit_move_batch.rs"
            language="Rust"
            code={`// Validating and Applying a high-speed move batch on the Rollup layer
pub fn handler_commit_move_batch(
    ctx: Context<CommitMoveBatchCtx>,
    moves: Vec<String>,
    next_fens: Vec<String>,
) -> Result<()> {
    // Verify move legality against the current FEN state
    for (move_str, next_fen_str) in moves.iter().zip(next_fens.iter()) {
        require!(current_pos.is_legal(chess_move), XfchessGameError::InvalidMove);
        current_pos = new_pos;
    }
    game.fen = current_pos.to_string();
    Ok(())
}`}
          />
        </div>

        <div className="feature-section">
          <h3>Trustless Asset Safety & Deep Liquidity</h3>
          <p>
            MagicBlock Ephemeral Rollups don't just speed up moves—they fundamentally harden the safety of every wager.
            By decoupling <em>execution</em> from <em>settlement</em>, XFChess protects your high-value assets across every competitive layer.
          </p>
          <div className="infrastructure-list">
            <div className="infrastructure-item">
              <h4>Non-Custodial Wagers (SOL & Tokens)</h4>
              <p>Wagered capital remains locked in Solana L1 PDAs (Program Derived Addresses). Only the state representing the "game board" is delegated to the ER layer—never the player's funds. This means tokens are never at risk from external network congestion.</p>
            </div>
            <div className="infrastructure-item">
              <h4>NFT & Asset Integrity</h4>
              <p>When playing with premium piece sets or themed boards (Metaplex Core), only metadata pointers are verified on the ER layer. Ownership remains natively on the Solana Base Layer, ensuring your digital assets are never truly "bridged" or exposed to L2 vulnerabilities.</p>
            </div>
            <div className="infrastructure-item">
              <h4>Verifiable Settlement</h4>
              <p>The transition from "high-speed play" to "final settlement" is cryptographically governed. The Solana L1 verifies the validity of the final ER state before a single lamport is released from escrow, guaranteeing total financial finality.</p>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default MagicBlockPage;