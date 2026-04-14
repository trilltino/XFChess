#![allow(dead_code)]
//! CLI argument parsing for XFChess
//!
//! This module handles command-line arguments for both the game client
//! and the transaction debugger.

use clap;
#[cfg(feature = "solana")]
pub mod tournament_admin;
use std::path::PathBuf;

/// Player color option
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum PlayerColor {
    White,
    Black,
}

/// XFChess CLI arguments
#[derive(clap::Parser, Debug)]
#[command(name = "xfchess")]
#[command(about = "XFChess - Decentralized Chess with Ephemeral Rollups")]
#[command(version = "0.1.0")]
pub struct Cli {
    /// Game ID
    #[arg(long)]
    pub game_id: Option<u64>,

    /// Player color (white or black)
    #[arg(long, value_enum)]
    pub player_color: Option<PlayerColor>,

    /// Solana RPC URL
    #[arg(long, default_value = "https://api.devnet.solana.com")]
    pub rpc_url: String,

    /// Session key (base58 encoded) for signing rollups
    #[arg(long)]
    pub session_key: Option<String>,

    /// Session public key
    #[arg(long)]
    pub session_pubkey: Option<String>,

    /// P2P network port
    #[arg(long, default_value = "5001")]
    pub p2p_port: u16,

    /// Bootstrap node ID (for Player 2 to connect to Player 1)
    #[arg(long)]
    pub bootstrap_node: Option<String>,

    /// Game PDA address
    #[arg(long)]
    pub game_pda: Option<String>,

    /// Wager amount in SOL
    #[arg(long)]
    pub wager_amount: Option<f64>,

    /// Opponent's P2P node ID
    #[arg(long)]
    pub opponent_node_id: Option<String>,

    /// Enable transaction debugger
    #[arg(long)]
    pub debug: bool,

    /// Log file for transaction debugger
    #[arg(long, default_value = "rollup_debug.log")]
    pub log_file: PathBuf,

    /// Disable pretty printing in debugger
    #[arg(long)]
    pub no_pretty_print: bool,

    /// Session config JSON file path
    #[arg(long)]
    pub session_config: Option<PathBuf>,

    /// Subcommand
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Subcommands
#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Run the transaction debugger
    Debug {
        /// Game ID to monitor
        #[arg(long)]
        game_id: u64,

        /// Log file path
        #[arg(long, default_value = "rollup_debug.log")]
        log_file: PathBuf,

        /// WebSocket port for remote monitoring
        #[arg(long)]
        websocket_port: Option<u16>,
    },
    /// Launch game directly with parameters
    Play {
        /// Game ID
        #[arg(long)]
        game_id: u64,

        /// Player color
        #[arg(long, value_enum)]
        player_color: PlayerColor,

        /// Session key
        #[arg(long)]
        session_key: String,

        /// Wager amount in SOL
        #[arg(long)]
        wager_amount: f64,
    },
    /// Tournament administrator controls
    Tournament {
        #[command(subcommand)]
        action: TournamentCommand,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum TournamentCommand {
    /// Create a new tournament on-chain and register it with the VPS
    Create {
        #[arg(long, default_value = "XFChess Cup")]
        name: String,
        /// Entry fee in SOL (e.g. 0.05)
        #[arg(long, default_value = "0.05")]
        entry_fee: f64,
    },
    /// List all active tournaments on the VPS
    List,
    /// Get tournament info
    Info {
        #[arg(long)]
        id: u64,
    },
    /// Start the bracket (call after players are registered)
    Start {
        #[arg(long)]
        id: u64,
    },
    /// Record a match result
    Record {
        #[arg(long)]
        id: u64,
        #[arg(long, value_name = "0|1|2")]
        match_index: u8,
        #[arg(long)]
        winner: String,
    },
    /// Advance semi-final winners into the final match slot
    Advance {
        #[arg(long)]
        id: u64,
    },
    /// Print current on-chain tournament state
    Status {
        #[arg(long)]
        id: u64,
    },
    /// Register 4 generated test wallets so you can call start immediately
    TestFill {
        #[arg(long)]
        id: u64,
    },
    /// Cancel a tournament
    Cancel {
        #[arg(long)]
        id: u64,
    },
}

/// Debugger-specific CLI arguments
#[derive(clap::Parser, Debug)]
#[command(name = "xfchess-debugger")]
#[command(about = "XFChess Transaction Debugger")]
pub struct DebuggerCli {
    /// Game ID to monitor
    #[arg(long)]
    pub game_id: u64,

    /// Log file path
    #[arg(long, default_value = "rollup_debug.log")]
    pub log_file: PathBuf,

    /// Enable pretty colored output
    #[arg(long, default_value = "true")]
    pub pretty_print: bool,

    /// WebSocket port for remote monitoring
    #[arg(long)]
    pub websocket_port: Option<u16>,

    /// Read from stdin
    #[arg(long)]
    pub stdin: bool,

    /// Follow mode
    #[arg(short, long)]
    pub follow: bool,
}

/// Session configuration loaded from JSON file
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionConfig {
    pub game_id: String,
    pub player_color: String,
    pub session_key: String,
    pub session_pubkey: String,
    pub node_id: String,
    pub rpc_url: String,
    pub game_pda: String,
    pub wager_amount: f64,
    pub opponent_pubkey: Option<String>,
}

impl Cli {
    /// Parse CLI arguments from environment
    pub fn parse_args() -> Self {
        <Self as clap::Parser>::parse()
    }

    /// Load session config from JSON file if specified
    pub fn load_session_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref path) = self.session_config {
            println!("📄 Loading session config from: {}", path.display());

            let contents = std::fs::read_to_string(path)?;
            let session: SessionConfig = serde_json::from_str(&contents)?;

            // Populate CLI args from session config
            self.game_id = Some(session.game_id.parse()?);
            self.player_color = Some(match session.player_color.as_str() {
                "white" => PlayerColor::White,
                "black" => PlayerColor::Black,
                _ => PlayerColor::White,
            });
            self.session_key = Some(session.session_key);
            self.session_pubkey = Some(session.session_pubkey);
            self.rpc_url = session.rpc_url;
            self.game_pda = Some(session.game_pda);
            self.wager_amount = Some(session.wager_amount);

            println!("✅ Session config loaded successfully");
            println!("   Game ID: {}", self.game_id.unwrap());
            println!("   Player: {:?}", self.player_color.unwrap());
            println!("   RPC: {}", self.rpc_url);
        }
        Ok(())
    }

    /// Check if running in debug mode
    pub fn is_debug_mode(&self) -> bool {
        self.debug || matches!(self.command, Some(Commands::Debug { .. }))
    }

    /// Get the game ID if specified
    pub fn get_game_id(&self) -> Option<u64> {
        if let Some(Commands::Debug { game_id, .. }) = self.command {
            return Some(game_id);
        }
        self.game_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_defaults() {
        let cli = Cli::parse_from(["xfchess"]);
        assert_eq!(cli.rpc_url, "https://api.devnet.solana.com");
        assert_eq!(cli.p2p_port, 5001);
        assert!(!cli.debug);
    }

    #[test]
    fn test_cli_with_args() {
        let cli = Cli::parse_from([
            "xfchess",
            "--game-id",
            "12345",
            "--player-color",
            "white",
            "--debug",
        ]);
        assert_eq!(cli.game_id, Some(12345));
        assert!(cli.debug);
    }
}
