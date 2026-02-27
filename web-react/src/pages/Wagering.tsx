import { motion } from 'framer-motion';
import { ArrowLeft, Users, MapPin, Trophy, Wallet } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './XFBeyond.css';

const Wagering = () => {
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
                <h1>Wagering & <span className="accent">Tournaments.</span></h1>
                <p>From local club meets to international championships—trustless wagering for every competitive format.</p>
            </header>

            <section className="architecture-overview">
                <h2>In-Person & Hybrid Competition</h2>
                <p>
                    XFChess bridges the gap between physical and digital chess. Players meet at coffee shops, chess clubs,
                    or tournament halls, verify their presence on-chain, and wager securely via smart contracts.
                    The result: tamper-proof settlement for over-the-board (OTB) matches with the speed of blockchain.
                </p>

                <div className="contract-modules">
                    <div className="module-card">
                        <MapPin size={28} color="#e63946" />
                        <h3>Local Club Matches</h3>
                        <p>Players meet at verified locations, scan QR codes to confirm presence, and wager via mobile wallets.</p>
                    </div>
                    <div className="module-card">
                        <Users size={28} color="#a855f7" />
                        <h3>Regional Tournaments</h3>
                        <p>Organized events with multiple rounds, automated prize distribution, and on-chain bracket management.</p>
                    </div>
                    <div className="module-card">
                        <Trophy size={28} color="#22c55e" />
                        <h3>International Championships</h3>
                        <p>Grandmaster events with substantial prize pools, live-streaming integration, and global rankings.</p>
                    </div>
                    <div className="module-card">
                        <Wallet size={28} color="#3b82f6" />
                        <h3>Instant Settlement</h3>
                        <p>Winners receive funds immediately after game conclusion—no delays, no disputes, no intermediaries.</p>
                    </div>
                </div>
            </section>

            <section className="competitive-features">
                <h2>Wagering Mechanics</h2>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">01</div>
                        <div>
                            <h3>Game Creation & Escrow</h3>
                            <p className="feature-subtitle">Creating a wager match locks funds in a Program Derived Address</p>
                        </div>
                    </div>
                    <p>
                        When a player creates a game with a wager, SOL is transferred from their wallet to an escrow PDA.
                        This PDA is derived from the game ID and program seeds, ensuring only the program can release funds.
                    </p>
                    <CodeViewer
                        title="xfchess-game/src/instructions/create_game.rs"
                        language="Rust"
                        code={`#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, game_type: GameType)]
pub struct CreateGame<'info> {
    #[account(
        init, 
        payer = player, 
        space = 8 + Game::INIT_SPACE, 
        seeds = [GAME_SEED, &game_id.to_le_bytes()], 
        bump
    )]
    pub game: Account<'info, Game>,
    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateGame>,
    game_id: u64,
    wager_amount: u64,
    game_type: GameType,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    game.wager_amount = wager_amount;
    game.status = GameStatus::WaitingForOpponent;

    // Transfer SOL to escrow PDA
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
    }
    Ok(())
}`}
                    />
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">02</div>
                        <div>
                            <h3>Joining & Matching</h3>
                            <p className="feature-subtitle">Opponents join by matching the wager amount</p>
                        </div>
                    </div>
                    <p>
                        When a second player joins the game, they must match the wager amount. Both players' funds
                        are now locked in escrow until the game concludes or expires.
                    </p>
                    <CodeViewer
                        title="xfchess-game/src/instructions/join_game.rs"
                        language="Rust"
                        code={`pub fn handler(
    ctx: Context<JoinGame>,
    _game_id: u64,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    
    // Verify game is waiting for opponent
    require!(game.status == GameStatus::WaitingForOpponent, 
        GameErrorCode::GameNotJoinable);
    
    // Black player joins and matches wager
    game.black = ctx.accounts.player.key();
    
    // Transfer matching wager to escrow
    if game.wager_amount > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            game.wager_amount,
        )?;
    }
    
    // Game is now active
    game.status = GameStatus::Active;
    msg!("Game {} is now active with {} SOL wager", _game_id, game.wager_amount);
    Ok(())
}`}
                    />
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">03</div>
                        <div>
                            <h3>Settlement & Payout</h3>
                            <p className="feature-subtitle">Winner takes all via automated smart contract distribution</p>
                        </div>
                    </div>
                    <p>
                        When a game concludes (checkmate, resignation, or draw), the smart contract automatically
                        distributes the escrowed funds. In a decisive result, the winner receives both wagers.
                        In a draw, funds are returned to both players.
                    </p>
                    <CodeViewer
                        title="xfchess-game/src/instructions/finalize_game.rs"
                        language="Rust"
                        code={`pub fn handler(
    ctx: Context<FinalizeGame>,
    _game_id: u64,
    result: GameResult,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    require!(game.status == GameStatus::Active, 
        GameErrorCode::GameNotActive);

    let escrow_bump = ctx.bumps.escrow_pda;
    let game_id_bytes = _game_id.to_le_bytes();
    let seeds = &[WAGER_ESCROW_SEED, game_id_bytes.as_ref(), &[escrow_bump]];
    let signer_seeds = &[&seeds[..]];

    match result {
        GameResult::Winner(winner) => {
            // Winner takes all - transfer total pot
            let pot = game.wager_amount * 2;
            **ctx.accounts.escrow_pda.try_borrow_mut_lamports()? -= pot;
            **ctx.accounts.winner.try_borrow_mut_lamports()? += pot;
            
            game.result = GameResult::Winner(winner);
            msg!("Game {} finalized. Winner: {}. Payout: {} SOL", 
                _game_id, winner, pot);
        }
        GameResult::Draw => {
            // Return wagers to both players
            **ctx.accounts.escrow_pda.try_borrow_mut_lamports()? -= game.wager_amount;
            **ctx.accounts.white.try_borrow_mut_lamports()? += game.wager_amount;
            **ctx.accounts.escrow_pda.try_borrow_mut_lamports()? -= game.wager_amount;
            **ctx.accounts.black.try_borrow_mut_lamports()? += game.wager_amount;
            
            game.result = GameResult::Draw;
            msg!("Game {} finalized as draw. Wagers returned.", _game_id);
        }
        _ => return Err(GameErrorCode::InvalidGameResult.into()),
    }

    game.status = GameStatus::Finished;
    Ok(())
}`}
                    />
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">04</div>
                        <div>
                            <h3>Tournament Formats</h3>
                            <p className="feature-subtitle">Supporting multiple competitive structures</p>
                        </div>
                    </div>
                    <p>
                        XFChess supports various tournament formats, each with on-chain bracket management
                        and automated prize distribution based on final standings.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Single Elimination</h4>
                            <p>Bracket-style tournaments where losers are eliminated. Winners advance until a champion is crowned.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Round Robin</h4>
                            <p>Each player faces every other player. Points are tallied on-chain for transparent standings.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Swiss System</h4>
                            <p>Popular in chess tournaments—players face opponents with similar records over multiple rounds.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Entry Fees & Prize Pools</h4>
                            <p>Tournament entry fees aggregate into prize pools, distributed automatically to top finishers.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">05</div>
                        <div>
                            <h3>Security & Anti-Cheating</h3>
                            <p className="feature-subtitle">Multi-layered protection for competitive integrity</p>
                        </div>
                    </div>
                    <p>
                        XFChess implements comprehensive anti-cheating measures to ensure fair play across all
                        match types. From move validation to behavioral analysis, the platform protects
                        competitive integrity while preserving user privacy.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>On-Chain Move Validation</h4>
                            <p>Every move is validated against the current FEN state using the Shakmaty chess engine. Illegal moves are rejected before being recorded.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Session Key Authentication</h4>
                            <p>Players authorize ephemeral session keys that expire after 2 hours, preventing unauthorized access even if credentials are compromised.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Behavioral Analysis</h4>
                            <p>Statistical models detect anomalous play patterns that may indicate engine assistance, flagging accounts for review.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Dispute Resolution</h4>
                            <p>On-chain move logs provide immutable evidence for dispute resolution. Third-party arbiters can verify game integrity from permanent records.</p>
                        </div>
                    </div>
                </div>
            </section>
        </motion.div>
    );
};

export default Wagering;
