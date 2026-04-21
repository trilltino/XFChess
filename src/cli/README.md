# CLI Tools

Command-line interface tools for XFChess tournament administration and game management.

## Overview

The CLI tools provide a command-line interface for interacting with XFChess functionality, including tournament administration, game management, and system operations. These tools are designed for:
- Tournament organizers to manage tournaments
- Developers to test and debug functionality
- System administrators to perform maintenance

## CLI Architecture

The CLI is built using the `clap` crate for argument parsing and follows a command-subcommand structure:

```
xfchess-cli <command> <subcommand> [options]
```

## Available Commands

- `tournament` - Tournament management operations
- `game` - Game management operations
- `player` - Player information and statistics
- `system` - System maintenance and configuration

## Components

- Tournament administration commands
- Game management utilities
- Player statistics and rankings
- System configuration and maintenance

## Example: Tournament Management

This example shows the tournament CLI commands for creating, starting, and managing tournaments.

```bash
# Create a new tournament
xfchess-cli tournament create \
  --name "Weekly Championship" \
  --entry-fee 1.0 \
  --authority <wallet-keypair>

# List all tournaments
xfchess-cli tournament list

# Get tournament details
xfchess-cli tournament show \
  --tournament-id <tournament-id>

# Start a tournament
xfchess-cli tournament start \
  --tournament-id <tournament-id> \
  --authority <wallet-keypair>

# Cancel a tournament
xfchess-cli tournament cancel \
  --tournament-id <tournament-id> \
  --authority <wallet-keypair>
```

## Example: Game Management

This example shows the game CLI commands for managing chess games.

```bash
# Create a new game
xfchess-cli game create \
  --player-white <wallet-keypair> \
  --time-control "10+5"

# List active games
xfchess-cli game list --status active

# Get game details
xfchess-cli game show --game-id <game-id>

# End a game
xfchess-cli game end \
  --game-id <game-id> \
  --winner <wallet-keypair> \
  --reason checkmate
```

## Example: Player Statistics

This example shows the player CLI commands for viewing player information.

```bash
# Get player profile
xfchess-cli player profile <wallet-pubkey>

# Get player ELO rating
xfchess-cli player rating <wallet-pubkey>

# Get player game history
xfchess-cli player history <wallet-pubkey> --limit 10

# Get player tournament results
xfchess-cli player tournaments <wallet-pubkey>
```

## Example: System Commands

This example shows the system CLI commands for maintenance and configuration.

```bash
# Check system status
xfchess-cli system status

# View configuration
xfchess-cli system config

# Reset system (development only)
xfchess-cli system reset --force

# Run database migrations
xfchess-cli system migrate
```

## CLI Implementation Example

This example shows how CLI commands are implemented using `clap`.

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// XFChess CLI tool for tournament and game management
#[derive(Parser, Debug)]
#[command(name = "xfchess-cli")]
#[command(about = "Command-line interface for XFChess", long_about = None)]
struct Cli {
    /// Path to wallet keypair file
    #[arg(short, long)]
    keypair: PathBuf,
    
    /// RPC endpoint URL
    #[arg(short, long, default_value = "https://api.devnet.solana.com")]
    rpc_url: String,
    
    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    
    #[command(subcommand)]
    command: Commands,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Tournament management operations
    Tournament {
        #[command(subcommand)]
        tournament_command: TournamentCommands,
    },
    
    /// Game management operations
    Game {
        #[command(subcommand)]
        game_command: GameCommands,
    },
    
    /// Player information and statistics
    Player {
        #[command(subcommand)]
        player_command: PlayerCommands,
    },
    
    /// System maintenance and configuration
    System {
        #[command(subcommand)]
        system_command: SystemCommands,
    },
}

/// Tournament commands
#[derive(Subcommand, Debug)]
enum TournamentCommands {
    /// Create a new tournament
    Create {
        /// Tournament name
        #[arg(short, long)]
        name: String,
        
        /// Entry fee in SOL
        #[arg(short, long)]
        entry_fee: f64,
    },
    
    /// List all tournaments
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },
    
    /// Show tournament details
    Show {
        /// Tournament ID
        #[arg(short, long)]
        tournament_id: u64,
    },
    
    /// Start a tournament
    Start {
        /// Tournament ID
        #[arg(short, long)]
        tournament_id: u64,
    },
    
    /// Cancel a tournament
    Cancel {
        /// Tournament ID
        #[arg(short, long)]
        tournament_id: u64,
    },
}

/// Game commands
#[derive(Subcommand, Debug)]
enum GameCommands {
    /// Create a new game
    Create {
        /// Time control (e.g., "10+5" for 10 minutes + 5 second increment)
        #[arg(short, long)]
        time_control: String,
    },
    
    /// List games
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },
    
    /// Show game details
    Show {
        /// Game ID
        #[arg(short, long)]
        game_id: u64,
    },
}

/// Player commands
#[derive(Subcommand, Debug)]
enum PlayerCommands {
    /// Show player profile
    Profile {
        /// Player public key
        #[arg(short, long)]
        pubkey: String,
    },
    
    /// Show player rating
    Rating {
        /// Player public key
        #[arg(short, long)]
        pubkey: String,
    },
}

/// System commands
#[derive(Subcommand, Debug)]
enum SystemCommands {
    /// Check system status
    Status,
    
    /// Show configuration
    Config,
    
    /// Run database migrations
    Migrate,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize RPC client
    let rpc_client = RpcClient::new(&cli.rpc_url)?;
    
    // Load wallet keypair
    let keypair = solana_sdk::keypair::read_keypair_file(&cli.keypair)?;
    
    // Execute command
    match cli.command {
        Commands::Tournament { tournament_command } => {
            handle_tournament_command(tournament_command, &rpc_client, &keypair).await
        }
        Commands::Game { game_command } => {
            handle_game_command(game_command, &rpc_client, &keypair).await
        }
        Commands::Player { player_command } => {
            handle_player_command(player_command, &rpc_client).await
        }
        Commands::System { system_command } => {
            handle_system_command(system_command).await
        }
    }
}

async fn handle_tournament_command(
    command: TournamentCommands,
    rpc_client: &RpcClient,
    keypair: &Keypair,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        TournamentCommands::Create { name, entry_fee } => {
            println!("Creating tournament: {}", name);
            println!("Entry fee: {} SOL", entry_fee);
            // Implementation: Call tournament creation instruction
            Ok(())
        }
        TournamentCommands::List { status } => {
            println!("Listing tournaments");
            if let Some(s) = status {
                println!("Filtering by status: {}", s);
            }
            // Implementation: Query tournaments from backend
            Ok(())
        }
        TournamentCommands::Show { tournament_id } => {
            println!("Showing tournament: {}", tournament_id);
            // Implementation: Query tournament details
            Ok(())
        }
        TournamentCommands::Start { tournament_id } => {
            println!("Starting tournament: {}", tournament_id);
            // Implementation: Call tournament start instruction
            Ok(())
        }
        TournamentCommands::Cancel { tournament_id } => {
            println!("Cancelling tournament: {}", tournament_id);
            // Implementation: Call tournament cancel instruction
            Ok(())
        }
    }
}

async fn handle_game_command(
    command: GameCommands,
    rpc_client: &RpcClient,
    keypair: &Keypair,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        GameCommands::Create { time_control } => {
            println!("Creating game with time control: {}", time_control);
            // Implementation: Call game creation instruction
            Ok(())
        }
        GameCommands::List { status } => {
            println!("Listing games");
            if let Some(s) = status {
                println!("Filtering by status: {}", s);
            }
            // Implementation: Query games from backend
            Ok(())
        }
        GameCommands::Show { game_id } => {
            println!("Showing game: {}", game_id);
            // Implementation: Query game details
            Ok(())
        }
    }
}

async fn handle_player_command(
    command: PlayerCommands,
    rpc_client: &RpcClient,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        PlayerCommands::Profile { pubkey } => {
            println!("Player profile: {}", pubkey);
            // Implementation: Query player profile
            Ok(())
        }
        PlayerCommands::Rating { pubkey } => {
            println!("Player rating: {}", pubkey);
            // Implementation: Query player ELO rating
            Ok(())
        }
    }
}

async fn handle_system_command(
    command: SystemCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SystemCommands::Status => {
            println!("System status: OK");
            // Implementation: Check system health
            Ok(())
        }
        SystemCommands::Config => {
            println!("System configuration:");
            // Implementation: Display configuration
            Ok(())
        }
        SystemCommands::Migrate => {
            println!("Running database migrations...");
            // Implementation: Run migrations
            Ok(())
        }
    }
}
```
