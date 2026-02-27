import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
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

      </section>

      <section className="competitive-features">
        <h2>Technical Implementation</h2>

        <div className="feature-section">
          <div className="feature-header">
            <div className="feature-number">01</div>
            <div>
              <h3>Session Key Authorization</h3>
              <p className="feature-subtitle">Before delegation, players authorize ephemeral session keys for high-speed signing</p>
            </div>
          </div>
          <p>
            Players generate temporary session keypairs that are authorized to sign moves on their behalf
            for the duration of the game. These keys expire after 2 hours and can be revoked at any time.
          </p>
          <CodeViewer
            title="programs/xfchess-game/src/instructions/session_delegation.rs"
            language="Rust"
            code={`pub fn handler_authorize_session_key(
    ctx: Context<AuthorizeSessionCtx>,
    game_id: u64,
    session_pubkey: Pubkey,
) -> Result<()> {
    let session = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    // Verify player is part of this game
    require!(
        player.key() == game.white || player.key() == game.black,
        XfchessGameError::UnauthorizedAccess
    );

    // Configure session delegation
    session.game_id = game_id;
    session.player = player.key();
    session.session_key = session_pubkey;
    session.expires_at = Clock::get()?.unix_timestamp + (2 * 60 * 60); // 2 hours
    session.max_batch_len = 10;
    session.enabled = true;

    Ok(())
}`}
          />
        </div>

        <div className="feature-section">
          <div className="feature-header">
            <div className="feature-number">02</div>
            <div>
              <h3>Program Account Delegation</h3>
              <p className="feature-subtitle">The Game PDA is delegated to MagicBlock ER for sub-second processing</p>
            </div>
          </div>
          <p>
            Once both players authorize their session keys, the game PDA is delegated to the
            Ephemeral Rollup. This moves game state processing off the base layer while
            maintaining Solana's security guarantees.
          </p>
          <CodeViewer
            title="programs/xfchess-game/src/instructions/delegate_game.rs"
            language="Rust"
            code={`pub fn handler_delegate_game(
    ctx: Context<DelegateGameCtx>,
    _game_id: u64,
    valid_until: i64,
) -> Result<()> {
    let game = &ctx.accounts.game;

    // Only game participants can delegate
    require!(
        ctx.accounts.payer.key() == game.white ||
        ctx.accounts.payer.key() == game.black,
        XfchessGameError::UnauthorizedAccess
    );

    // Calculate PDA seeds for the game account
    let game_id_bytes = _game_id.to_le_bytes();
    let seeds: &[&[u8]] = &[b"game", &game_id_bytes, &[game.bump]];

    // Configure delegation with commit frequency
    let config = DelegateConfig {
        commit_frequency_ms: (valid_until as u32).saturating_mul(1000),
        validator: None, // Any available ER validator
    };

    // Execute delegation CPI to MagicBlock
    delegate_account(delegate_accounts, seeds, config)?;
    Ok(())
}`}
          />
        </div>

        <div className="feature-section">
          <div className="feature-header">
            <div className="feature-number">03</div>
            <div>
              <h3>High-Speed Batch Commits</h3>
              <p className="feature-subtitle">Moves are validated and committed in batches using session signatures</p>
            </div>
          </div>
          <p>
            On the ER layer, moves are processed in batches up to 10 moves at a time. Each batch
            is validated against the chess rules using the Shakmaty engine, then committed back
            to the game state.
          </p>
          <CodeViewer
            title="programs/xfchess-game/src/instructions/commit_move_batch.rs"
            language="Rust"
            code={`pub fn handler_commit_move_batch(
    ctx: Context<CommitMoveBatchCtx>,
    _game_id: u64,
    moves: Vec<String>,
    next_fens: Vec<String>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let white_del = &ctx.accounts.white_delegation;
    let black_del = &ctx.accounts.black_delegation;

    // Verify session keys are valid and not expired
    require!(
        ctx.accounts.white_session.key() == white_del.session_key,
        XfchessGameError::InvalidSessionKey
    );
    require!(
        white_del.enabled && clock.unix_timestamp <= white_del.expires_at,
        XfchessGameError::SessionExpired
    );

    // Validate each move using Shakmaty chess engine
    for (move_str, next_fen) in moves.iter().zip(next_fens.iter()) {
        let uci: UciMove = move_str.parse()?;
        let chess_move = uci.to_move(&current_pos)?;
        let new_pos = current_pos.play(chess_move)?;
        
        // Verify provided FEN matches computed state
        let computed_fen = Fen::from_position(&new_pos, EnPassantMode::Legal);
        require!(computed_fen.to_string() == *next_fen,
            XfchessGameError::InvalidNextFen);
        
        current_pos = new_pos;
    }

    game.fen = current_pos.to_string();
    game.move_count += moves.len() as u16;
    Ok(())
}`}
          />
        </div>

        <div className="feature-section">
          <div className="feature-header">
            <div className="feature-number">04</div>
            <div>
              <h3>Ephemeral Rollups SDK Integration</h3>
              <p className="feature-subtitle">Direct usage of ephemeral-rollups-sdk crate for ER operations</p>
            </div>
          </div>
          <p>
            XFChess integrates the <code>ephemeral-rollups-sdk</code> crate directly, using its CPI helpers and
            delegation primitives. The SDK provides the <code>delegate_account</code> and <code>commit_and_undelegate_accounts</code>
            functions that power the ER lifecycle.
          </p>
          <CodeViewer
            title="Cargo.toml - SDK Dependency"
            language="toml"
            code={`[features]
magicblock = ["dep:ephemeral-rollups-sdk", "ephemeral-rollups-sdk/anchor"]

[dependencies]
ephemeral-rollups-sdk = { path = "../ephemeral-rollups-sdk/rust/sdk", optional = true }`}
          />
          <CodeViewer
            title="src/instructions/delegate_game.rs - SDK Imports"
            language="rust"
            code={`use ephemeral_rollups_sdk::consts::DELEGATION_PROGRAM_ID;
use ephemeral_rollups_sdk::cpi::{delegate_account, DelegateAccounts, DelegateConfig};
use ephemeral_rollups_sdk::ephem::deprecated::v0::commit_and_undelegate_accounts;

/// Delegate the Game PDA to the MagicBlock ER
pub fn handler_delegate_game(...) -> Result<()> {
    let config = DelegateConfig {
        commit_frequency_ms: (valid_until as u32).saturating_mul(1000),
        validator: None, // Any available ER validator
    };

    let delegate_accounts = DelegateAccounts {
        payer: &ctx.accounts.payer.to_account_info(),
        pda: &ctx.accounts.game.to_account_info(),
        owner_program: &ctx.accounts.owner_program.to_account_info(),
        buffer: &ctx.accounts.buffer.to_account_info(),
        delegation_record: &ctx.accounts.delegation_record.to_account_info(),
        delegation_metadata: &ctx.accounts.delegation_metadata.to_account_info(),
        delegation_program: &ctx.accounts.delegation_program.to_account_info(),
        system_program: &ctx.accounts.system_program.to_account_info(),
    };

    // Execute delegation CPI via SDK
    delegate_account(delegate_accounts, seeds, config)?;
    Ok(())
}`}
          />
        </div>

        <div className="feature-section">
          <div className="feature-header">
            <div className="feature-number">05</div>
            <div>
              <h3>Bevy Rollup Network Bridge</h3>
              <p className="feature-subtitle">Rust game engine integration with ER systems</p>
            </div>
          </div>
          <p>
            The game client uses Bevy's ECS architecture to manage ER delegation lifecycle. Systems handle
            game start/end events and route transactions through the MagicBlock resolver when delegated.
          </p>
          <CodeViewer
            title="src/multiplayer/rollup_network_bridge.rs - Bevy Plugin"
            language="rust"
            code={`pub struct RollupNetworkBridgePlugin;

impl Plugin for RollupNetworkBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RollupNetworkBridge>();
        app.add_event::<MagicBlockEvent>();

        // Core bridge systems
        app.add_systems(Update, handle_rollup_to_network_events);
        app.add_systems(Update, handle_network_to_rollup_events);
        app.add_systems(Update, process_batch_commit_requests);

        // Magic Block ER delegation systems
        app.add_systems(Update, handle_game_start_delegation);
        app.add_systems(Update, handle_game_end_undelegation);
        app.add_systems(Update, handle_magic_block_events);

        info!("RollupNetworkBridgePlugin initialized with Magic Block ER support");
    }
}`}
          />
          <CodeViewer
            title="src/multiplayer/rollup_network_bridge.rs - Delegation Handler"
            language="rust"
            code={`/// Handles game start events to delegate the game PDA to the ER
fn handle_game_start_delegation(
    mut game_started_events: EventReader<GameStartedEvent>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    session_key_manager: Res<SessionKeyManager>,
    mut magicblock_events: EventWriter<MagicBlockEvent>,
) {
    for event in game_started_events.read() {
        let game_id = event.game_id;
        info!("Game {} started - initiating ER delegation", game_id);

        // Get session keypair for signing delegation tx
        let session_keypair = match session_key_manager.get_active_keypair() {
            Some(kp) => kp,
            None => {
                error!("No session keypair available for delegation");
                continue;
            }
        };

        // Delegate game to ER
        match magicblock_resolver.delegate_game(game_pda, &session_keypair) {
            Ok(_) => {
                info!("Successfully delegated game {} to ER", game_id);
                magicblock_events.send(MagicBlockEvent::GameDelegated { game_pda });
            }
            Err(e) => {
                error!("Failed to delegate game {} to ER: {}", game_id, e);
                magicblock_events.send(MagicBlockEvent::DelegationFailed { game_pda, error: e.to_string() });
            }
        }
    }
}`}
          />
        </div>
      </section>
    </motion.div>
  );
};

export default MagicBlockPage;