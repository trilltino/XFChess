import { motion } from 'framer-motion';
import { ArrowLeft, Gem, Crown, Swords, TrendingUp } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './XFBeyond.css';

const NFTWagers = () => {
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
                <h1>NFT <span className="accent">Wagers.</span></h1>
                <p>Wager unique chess pieces, boards, and collectibles in high-stakes matches with trustless settlement.</p>
            </header>

            <section className="architecture-overview">
                <h2>Tradable Game Assets as Collateral</h2>
                <p>
                    XFChess enables players to wager NFTs directly—premium chess piece sets, exclusive board skins,
                    and tournament badges can all be staked in competitive matches. When a player loses, the NFT
                    automatically transfers to the winner via on-chain settlement. This creates a thriving secondary
                    market where rare assets gain value through competitive provenance.
                </p>

                <div className="contract-modules">
                    <div className="module-card">
                        <Crown size={28} color="#e63946" />
                        <h3>Premium Piece Sets</h3>
                        <p>Rare 3D chess pieces (Haitian Revolution, Napoleonic Era, Cyberpunk) can be wagered and traded based on competitive history.</p>
                    </div>
                    <div className="module-card">
                        <Gem size={28} color="#a855f7" />
                        <h3>Exclusive Board Skins</h3>
                        <p>Limited edition board themes—from marble tournament halls to neon cyber-arenas—add prestige and value to high-stakes matches.</p>
                    </div>
                    <div className="module-card">
                        <Swords size={28} color="#22c55e" />
                        <h3>Tournament Badges</h3>
                        <p>Victory badges from major tournaments become tradable collectibles, with winners earning exclusive NFT trophies.</p>
                    </div>
                    <div className="module-card">
                        <TrendingUp size={28} color="#3b82f6" />
                        <h3>Secondary Market Trading</h3>
                        <p>All wagered NFTs are tradable on marketplaces like Tensor and Magic Eden, with value tied to competitive history.</p>
                    </div>
                </div>
            </section>

            <section className="competitive-features">
                <h2>NFT Wager Implementation</h2>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">01</div>
                        <div>
                            <h3>Game State with Token Support</h3>
                            <p className="feature-subtitle">The Game account includes optional NFT mint address for token wagers</p>
                        </div>
                    </div>
                    <p>
                        Each game can escrow both SOL and NFTs. The <code>wager_token</code> field stores the
                        mint address of the NFT being wagered, while <code>wager_amount</code> handles the quantity
                        (useful for SPL tokens or 1 for NFTs).
                    </p>
                    <CodeViewer
                        title="xfchess-game/src/state/game.rs"
                        language="Rust"
                        code={`#[account]
#[derive(InitSpace)]
pub struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub status: GameStatus,
    pub result: GameResult,
    #[max_len(100)]
    pub fen: String,
    pub move_count: u16,
    pub turn: u8,
    pub created_at: i64,
    pub updated_at: i64,
    pub wager_amount: u64,
    pub wager_token: Option<Pubkey>,  // NFT/SPL Token mint address
    pub game_type: GameType,
    pub bump: u8,
}`}
                    />
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">02</div>
                        <div>
                            <h3>NFT Escrow Transfer</h3>
                            <p className="feature-subtitle">Secure token transfer via Associated Token Accounts</p>
                        </div>
                    </div>
                    <p>
                        When a game expires or concludes, the NFT wager is transferred via the Token Program.
                        The escrow PDA acts as the authority, ensuring trustless settlement without intermediaries.
                    </p>
                    <CodeViewer
                        title="xfchess-game/src/instructions/withdraw_expired_wager.rs"
                        language="Rust"
                        code={`pub fn handler(ctx: Context<WithdrawExpiredWager>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();

    // Verify game is expired and caller is game creator
    require!(game.status == GameStatus::WaitingForOpponent, 
        GameErrorCode::GameNotExpired);
    require!(game.white == player, GameErrorCode::NotGameCreator);

    if let Some(_token_mint) = game.wager_token {
        // NFT/SPL Token transfer via escrow PDA
        let vault_ata = ctx.accounts.vault_nft_ata.as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;
        let player_ata = ctx.accounts.player_nft_ata.as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;
        
        // Derive escrow PDA signer seeds
        let game_id_bytes = _game_id.to_le_bytes();
        let escrow_bump = ctx.bumps.escrow_pda;
        let seeds = &[WAGER_ESCROW_SEED, game_id_bytes.as_ref(), &[escrow_bump]];
        
        // Transfer NFT from vault back to player
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: vault_ata.to_account_info(),
                    to: player_ata.to_account_info(),
                    authority: ctx.accounts.escrow_pda.to_account_info(),
                },
                &[&seeds[..]],
            ),
            game.wager_amount, // 1 for NFTs
        )?;
    }
    Ok(())
}`}
                    />
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">03</div>
                        <div>
                            <h3>High-Speed NFT Wagers with Ephemeral Rollups</h3>
                            <p className="feature-subtitle">ER delegation enables sub-second NFT transfers during gameplay</p>
                        </div>
                    </div>
                    <p>
                        NFT wagers benefit from MagicBlock Ephemeral Rollups just like SOL wagers. By delegating
                        the game PDA to the ER layer, NFT transfers can be processed with sub-second latency during
                        the match, with final settlement committing back to Solana L1 when the game concludes.
                    </p>
                    <CodeViewer
                        title="ER-Enabled NFT Settlement Flow"
                        language="rust"
                        code={`// During gameplay: High-speed NFT state updates on ER
pub fn handler_commit_move_batch(ctx: Context<CommitMoveBatchCtx>, ...) -> Result<()> {
    // Game state (including NFT wager status) processed on ER
    // Sub-second confirmation for move validation
    game.fen = new_fen;
    game.move_count += moves.len() as u16;
    Ok(())
}

// Game conclusion: Commit final state + NFT transfer to L1
pub fn finalize_game_with_nft(ctx: Context<FinalizeGameCtx>) -> Result<()> {
    // Undelegate from ER - commits all ER state to Solana base layer
    magicblock_resolver.undelegate_game(&session_keypair)?;
    
    // Execute NFT transfer from escrow to winner
    let vault_ata = ctx.accounts.vault_nft_ata.as_ref()?;
    let winner_ata = ctx.accounts.winner_nft_ata.as_ref()?;
    
    token::transfer(
        CpiContext::new_with_signer(
            token_program,
            Transfer { from: vault_ata, to: winner_ata, authority: escrow },
            signer_seeds,
        ),
        1, // NFT transfer
    )?;
    
    msg!("Game finalized. NFT transferred to winner on L1.");
    Ok(())
}`}
                    />
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>ER Delegation for NFT Games</h4>
                            <p>Game PDAs with NFT wagers can be delegated to ER for high-speed processing, just like SOL wager games.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Metaplex Core Standard</h4>
                            <p>NFTs use Metaplex Core or Token Metadata standards for maximum compatibility with wallets and marketplaces like Tensor and Magic Eden.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Provenance Tracking</h4>
                            <p>Each NFT's match history is recorded on-chain, creating a verifiable competitive record that adds value in secondary markets.</p>
                        </div>
                    </div>
                </div>
            </section>
        </motion.div>
    );
};

export default NFTWagers;
