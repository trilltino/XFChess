
import { motion } from 'framer-motion';
import { ArrowLeft, Bot, Users, Cpu, Target } from 'lucide-react';
import { Link } from 'react-router-dom';
import CodeViewer from '../components/CodeViewer';
import './XFBeyond.css';

const Ecosystem = () => {
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
                <h1>Players & <span className="accent">Bots.</span></h1>
                <p>A competitive landscape where human skill meets algorithmic precision in trustless combat. From casual players to AI engines—every participant competes for real stakes on-chain.</p>
            </header>

            <section className="architecture-overview">
                <h2>The Competitive Ecosystem</h2>
                <p>
                    XFChess creates a unique environment where human players, AI engines, and hybrid competitors
                    coexist in a vibrant wagering economy. Players earn ELO ratings stored on-chain, bots compete
                    autonomously for profit, and developers monetize their algorithms. Every match is a smart contract
                    interaction—transparent, immutable, and instantly settled.
                </p>

                <div className="contract-modules">
                    <div className="module-card" style={{ borderTop: '3px solid #e63946' }}>
                        <Users size={32} color="#e63946" />
                        <h3>Casual Players</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Entry-level to intermediate players find skill-matched opponents via the on-chain ELO system.
                            Every game contributes to a permanent, verifiable ranking. Friendly wagers from 0.01 SOL
                            make every match meaningful while the escrow system ensures fair play.
                        </p>
                        <div style={{ marginTop: '1rem', padding: '0.75rem', background: 'rgba(230, 57, 70, 0.1)', borderRadius: '8px' }}>
                            <small style={{ color: '#e63946', fontWeight: 600 }}>Avg. Wager: 0.1-1 SOL</small>
                        </div>
                    </div>
                    <div className="module-card" style={{ borderTop: '3px solid #a855f7' }}>
                        <Target size={32} color="#a855f7" />
                        <h3>Grandmasters & Pros</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Elite competitors (2200+ ELO) access exclusive high-stakes tournaments with substantial
                            prize pools. On-chain reputation becomes a resume—every victory, every title, every
                            championship permanently recorded on Solana for global verification.
                        </p>
                        <div style={{ marginTop: '1rem', padding: '0.75rem', background: 'rgba(168, 85, 247, 0.1)', borderRadius: '8px' }}>
                            <small style={{ color: '#a855f7', fontWeight: 600 }}>Avg. Wager: 10-100+ SOL</small>
                        </div>
                    </div>
                    <div className="module-card" style={{ borderTop: '3px solid #22c55e' }}>
                        <Cpu size={32} color="#22c55e" />
                        <h3>Chess Engine Developers</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Developers deploy custom algorithms—Stockfish forks, Leela Chess Zero configurations,
                            or novel neural networks. The Braid-HTTP API enables real-time engine integration,
                            allowing bots to compete autonomously for wagering profits 24/7.
                        </p>
                        <div style={{ marginTop: '1rem', padding: '0.75rem', background: 'rgba(34, 197, 94, 0.1)', borderRadius: '8px' }}>
                            <small style={{ color: '#22c55e', fontWeight: 600 }}>Tech: Rust, Python, UCI Protocol</small>
                        </div>
                    </div>
                    <div className="module-card" style={{ borderTop: '3px solid #3b82f6' }}>
                        <Bot size={32} color="#3b82f6" />
                        <h3>Bot Operators</h3>
                        <p style={{ fontSize: '0.95rem', lineHeight: '1.6' }}>
                            Businesses and enthusiasts run automated players as a revenue stream. With session key
                            delegation, bots operate autonomously on the Ephemeral Rollup—processing moves in
                            milliseconds while generating passive income from every victory.
                        </p>
                        <div style={{ marginTop: '1rem', padding: '0.75rem', background: 'rgba(59, 130, 246, 0.1)', borderRadius: '8px' }}>
                            <small style={{ color: '#3b82f6', fontWeight: 600 }}>Uptime: 24/7 Automated</small>
                        </div>
                    </div>
                </div>
            </section>

            <section className="competitive-features">
                <h2>Human vs Bot Dynamics</h2>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">01</div>
                        <div>
                            <h3>Skill-Matched Competition</h3>
                            <p className="feature-subtitle">ELO system ensures fair matchups across all competitor types</p>
                        </div>
                    </div>
                    <p>
                        Whether facing a human opponent or an AI engine, the ELO rating system ensures competitive
                        balance. Bots are ranked alongside humans, creating a unified leaderboard where the best
                        players—organic or synthetic—rise to the top.
                    </p>
                    <CodeViewer
                        title="xfchess-game/src/state/player_profile.rs - On-Chain Player Stats"
                        language="rust"
                        code={`#[account]
#[derive(InitSpace)]
pub struct PlayerProfile {
    pub authority: Pubkey,      // Player's wallet address
    pub wins: u32,              // Total games won
    pub losses: u32,            // Total games lost
    pub draws: u32,             // Total games drawn
    pub games_played: u32,      // Total games completed
    pub elo: u16,               // Current ELO rating (starting: 1200)
    pub highest_elo: u16,       // Peak rating achieved
    pub tournament_wins: u16,   // Championships won
    pub bot_matches: u32,       // Games vs AI opponents
}

impl PlayerProfile {
    // Calculate win rate percentage
    pub fn win_rate(&self) -> f64 {
        if self.games_played == 0 { return 0.0; }
        (self.wins as f64 / self.games_played as f64) * 100.0
    }
    
    // Update ELO after a match using standard chess formula
    pub fn update_elo(&mut self, opponent_elo: u16, result: GameResult) {
        let k_factor = if self.games_played < 30 { 40 } else { 20 };
        let expected_score = 1.0 / (1.0 + 10f64.powi(
            (opponent_elo as i32 - self.elo as i32) / 400
        ));
        
        let actual_score = match result {
            GameResult::Win => 1.0,
            GameResult::Draw => 0.5,
            GameResult::Loss => 0.0,
        };
        
        let elo_change = (k_factor as f64 * (actual_score - expected_score)) as i16;
        self.elo = (self.elo as i16 + elo_change).max(100) as u16;
        self.highest_elo = self.highest_elo.max(self.elo);
    }
}`}
                    />
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Unified Rankings</h4>
                            <p>Humans and bots compete on the same ELO ladder. A 2400-rated bot faces the same challenges as a 2400-rated Grandmaster.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Transparent Ratings</h4>
                            <p>All ratings are stored on-chain, preventing rating manipulation and ensuring fair matchmaking.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">02</div>
                        <div>
                            <h3>Bot API & Developer Tools</h3>
                            <p className="feature-subtitle">Open protocols for third-party engine integration</p>
                        </div>
                    </div>
                    <p>
                        XFChess provides comprehensive APIs for developers to integrate chess engines. Using the
                        Braid-HTTP protocol, bots can subscribe to game states and respond with moves in real-time,
                        enabling sub-second response times for competitive play.
                    </p>
                    <CodeViewer
                        title="src/bot/braid_client.rs - Bot Network Integration"
                        language="rust"
                        code={`use braid_http::{Client, Subscription};
use crate::chess::{ChessEngine, Move};

pub struct BotBraidClient {
    client: Client,
    engine: ChessEngine,
    game_topic: String,
}

impl BotBraidClient {
    pub async fn connect(&mut self, game_id: u64) -> Result<(), Box<dyn Error>> {
        // Subscribe to game state updates via Braid-HTTP
        let topic = format!("xfchess/game/{}", game_id);
        let mut subscription = self.client.subscribe(&topic).await?;
        
        info!("Bot connected to game {} via Braid", game_id);
        
        // Listen for game state changes
        while let Some(update) = subscription.next().await {
            let game_state: GameState = serde_json::from_slice(&update)?;
            
            // If it's our turn, calculate best move
            if game_state.current_player == self.bot_keypair.pubkey() {
                let best_move = self.engine.calculate_best_move(
                    &game_state.fen,
                    Duration::from_secs(5)
                ).await?;
                
                // Submit move via network
                self.submit_move(game_id, best_move).await?;
            }
        }
        Ok(())
    }
    
    async fn submit_move(&self, game_id: u64, mv: Move) -> Result<(), Box<dyn Error>> {
        let move_msg = NetworkMessage::Move {
            game_id,
            move_str: mv.to_uci(),
            next_fen: mv.resulting_fen(),
        };
        
        self.client.publish(&self.game_topic, move_msg).await?;
        Ok(())
    }
}`}
                    />
                    <CodeViewer
                        title="src/bot/stockfish_bridge.rs - UCI Engine Integration"
                        language="rust"
                        code={`use tokio::process::{Command, Child};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};

pub struct StockfishBridge {
    process: Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl StockfishBridge {
    pub async fn spawn(engine_path: &Path) -> Result<Self, Box<dyn Error>> {
        let mut child = Command::new(engine_path)
            .arg("uci")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
            
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        
        Ok(Self { process: child, stdin, stdout })
    }
    
    pub async fn calculate_best_move(
        &mut self,
        fen: &str,
        think_time: Duration
    ) -> Result<String, Box<dyn Error>> {
        // Send position to Stockfish
        self.stdin.write_all(
            format!("position fen {}\\n", fen).as_bytes()
        ).await?;
        
        // Request move calculation
        self.stdin.write_all(
            format!("go movetime {}\\n", think_time.as_millis()).as_bytes()
        ).await?;
        
        // Read bestmove response
        let mut line = String::new();
        loop {
            line.clear();
            self.stdout.read_line(&mut line).await?;
            
            if line.starts_with("bestmove") {
                let mv = line.split_whitespace().nth(1).unwrap();
                return Ok(mv.to_string());
            }
        }
    }
}`}
                    />
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Session Key Authorization</h4>
                            <p>Bots use the same session delegation system as humans, allowing high-speed play on the Ephemeral Rollup.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Stockfish Integration</h4>
                            <p>Built-in support for Stockfish via UCI protocol. Deploy your own engine or use the reference implementation.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Neural Network Support</h4>
                            <p>Leela Chess Zero and other NN-based engines can connect via the network bridge for GPU-accelerated play.</p>
                        </div>
                    </div>
                </div>

            </section>

            {/* PLANNED FEATURES SECTION */}
            <section className="competitive-features">
                <h2>Planned Features</h2>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">03</div>
                        <div>
                            <h3>Automated Tournament Economy</h3>
                            <p className="feature-subtitle">Bots enable 24/7 tournament operations</p>
                        </div>
                    </div>
                    <p>
                        Bot operators can run continuous tournaments, offering guaranteed matches at any time of day.
                        This creates a thriving ecosystem where human players always have opponents, and bot operators
                        generate revenue from their algorithmic edge.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Always-Available Matches</h4>
                            <p>No waiting for opponents—bots provide instant matchmaking at any stake level, 24/7.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Arbitrage Opportunities</h4>
                            <p>Sophisticated bot operators can offer odds or handicaps, creating a betting market around match outcomes.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Training & Development</h4>
                            <p>Players can test their skills against various engine strengths, from beginner to super-grandmaster level.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">04</div>
                        <div>
                            <h3>Competitive Scenarios</h3>
                            <p className="feature-subtitle">Diverse match types for every competitive preference</p>
                        </div>
                    </div>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Human vs Human (PvP)</h4>
                            <p>Traditional competitive chess with skill-matched opponents. The foundation of the XFChess ecosystem.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Human vs Bot (PvAI)</h4>
                            <p>Players test their skills against algorithmic opponents, with adjustable difficulty levels and wager amounts.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Bot vs Bot (AIvAI)</h4>
                            <p>Engine tournaments where developers compete to create the strongest chess algorithms. Automated wagering creates real stakes.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Handicap Matches</h4>
                            <p>Stronger players can offer odds (time odds, material odds) to attract weaker opponents while maintaining competitive balance.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">05</div>
                        <div>
                            <h3>Chess Bot Platform</h3>
                            <p className="feature-subtitle">A marketplace for algorithmic competitors</p>
                        </div>
                    </div>
                    <p>
                        XFChess is building the premier platform for chess engine competitions. Developers can
                        deploy their algorithms, tune their parameters, and compete for real stakes. The platform
                        tracks bot performance, maintains leaderboards, and ensures fair matchmaking between
                        engines of similar strength.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Algorithmic Competition</h4>
                            <p>Bots compete in dedicated tournaments, testing different approaches: neural networks, classical engines, hybrid systems.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Performance Analytics</h4>
                            <p>Detailed metrics on bot performance—win rates, average game length, opening preferences, endgame proficiency.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Bot Marketplace</h4>
                            <p>Developers can license their engines to other users, creating a revenue stream from algorithmic innovation.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">06</div>
                        <div>
                            <h3>Economic Incentives</h3>
                            <p className="feature-subtitle">A self-sustaining competitive economy</p>
                        </div>
                    </div>
                    <p>
                        Every participant in the XFChess ecosystem has clear economic incentives. Players seek to
                        improve and win wagers. Bot operators invest in better algorithms to maintain their edge.
                        Tournament organizers earn fees from successful events. This creates a virtuous cycle of
                        competition, improvement, and growth.
                    </p>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Player Rewards</h4>
                            <p>Winning matches earns immediate payouts. Climbing the ELO ladder unlocks exclusive tournaments and higher stakes.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Bot Operator Revenue</h4>
                            <p>Successful bot operators generate passive income from automated matches. Better algorithms = higher win rates = more profit.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Developer Bounties</h4>
                            <p>Open-source contributors can earn rewards for improving the protocol, creating new features, or optimizing engine integrations.</p>
                        </div>
                    </div>
                </div>

                <div className="feature-section">
                    <div className="feature-header">
                        <div className="feature-number">07</div>
                        <div>
                            <h3>Platform Expansion</h3>
                            <p className="feature-subtitle">Mobile and themed content roadmap</p>
                        </div>
                    </div>
                    <div className="infrastructure-list">
                        <div className="infrastructure-item">
                            <h4>Android Mobile Client</h4>
                            <p>Full on-chain chess in your pocket — Bevy engine running natively on Android with Mobile Wallet Adapter (MWA) integration for Phantom and Solflare. The Android build shares the same Rust codebase as desktop, ensuring feature parity with P2P networking, Stockfish AI, and Solana settlement.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Themed Chess Sets — Season 1: Haitian Revolt</h4>
                            <p>Historically-themed 3D chess sets where each piece represents significant figures from pivotal moments. Season 1 features the 1791 Haitian Revolution — rebels led by Toussaint Louverture versus French colonial forces. Each piece is a detailed, animated 3D model with PBR textures at 2K resolution.</p>
                        </div>
                        <div className="infrastructure-item">
                            <h4>Future Seasons</h4>
                            <p>Medieval warfare (Knights vs. Saracens), Ancient civilizations (Romans vs. Carthaginians), Revolutionary periods, and Fantasy adaptations with mythological creatures.</p>
                        </div>
                    </div>
                </div>
            </section>
        </motion.div>
    );
};

export default Ecosystem;
