use anyhow::{anyhow, Result};
use bs58;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;

use braid_stockfish_ai::BraidStockfishSidecar;
use shakmaty::fen::Fen;
use shakmaty::uci::UciMove as Uci;
use shakmaty::CastlingMode;
use shakmaty::{Chess, Position};
use solana_chess_client::ChessRpcClient;

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::mpsc;
use xfchess_game::state::{GameStatus, GameType};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Starting XFChess AI Service...");

    // 1. Load AI Authority Keypair
    let keypair_path = "crates/xfchess-ai-service/ai-authority.json";
    let ai_authority = solana_sdk::signature::read_keypair_file(keypair_path)
        .map_err(|e| anyhow!("Failed to read keypair from {}: {}", keypair_path, e))?;
    let ai_pubkey = ai_authority.pubkey();
    log::info!("AI Authority Pubkey: {}", ai_pubkey);

    // 2. Initialize RPC Client
    let rpc_url =
        std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string());
    let client = ChessRpcClient::new(&rpc_url);
    log::info!("Connected to RPC: {}", rpc_url);

    // 3. Initialize Stockfish Sidecar
    let (fen_tx, fen_rx) = std::sync::mpsc::channel::<String>();
    let (move_tx, mut move_rx) = tokio::sync::mpsc::channel::<String>(10);

    let mut sidecar =
        BraidStockfishSidecar::with_channels(fen_rx, move_tx).map_err(|e| anyhow!("{:?}", e))?;
    std::thread::spawn(move || {
        sidecar.run();
    });
    log::info!("Stockfish Sidecar thread started.");

    // 4. Processing Loop
    let mut processed_moves = HashSet::new();

    loop {
        match client.fetch_all_games() {
            Ok(games) => {
                for game in games {
                    // Filter: Active, PvAI, and it's Black's turn (even)
                    if game.status == GameStatus::Active
                        && game.game_type == GameType::PvAI
                        && game.turn % 2 == 0
                    {
                        let game_key = format!("{}_{}", game.game_id, game.turn);
                        if processed_moves.contains(&game_key) {
                            continue;
                        }

                        log::info!(
                            "[AI Service] Processing turn {} for Game {}",
                            game.turn,
                            game.game_id
                        );

                        // Send FEN to Stockfish
                        if let Err(e) = fen_tx.send(game.fen.clone()) {
                            log::error!("Failed to send FEN to sidecar: {}", e);
                            continue;
                        }

                        // Wait for move from Stockfish
                        if let Some(uci_move) = move_rx.recv().await {
                            log::info!("[AI Service] Stockfish suggested move: {}", uci_move);

                            // Calculate next FEN using shakmaty
                            let next_fen = match calculate_next_fen(&game.fen, &uci_move) {
                                Ok(fen) => fen,
                                Err(e) => {
                                    log::error!("Failed to calculate next FEN: {}", e);
                                    continue;
                                }
                            };

                            // Submit RecordMove transaction
                            match submit_move(
                                &client,
                                &ai_authority,
                                game.game_id,
                                uci_move.to_string(),
                                next_fen.to_string(),
                            )
                            .await
                            {
                                Ok(sig) => {
                                    log::info!("[AI Service] Move submitted! Signature: {}", sig);
                                    processed_moves.insert(game_key);
                                }
                                Err(e) => {
                                    log::error!("Failed to submit move: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to fetch games: {}", e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}

fn calculate_next_fen(current_fen: &str, uci_move: &str) -> Result<String> {
    use shakmaty::CastlingMode;
    let mut pos: shakmaty::Chess = Fen::from_str(current_fen)
        .map_err(|e| anyhow!("Invalid FEN: {:?}", e))?
        .into_position(CastlingMode::Standard)
        .map_err(|e| anyhow!("Invalid position: {:?}", e))?;
    let uci: Uci = uci_move
        .parse()
        .map_err(|e| anyhow!("Invalid UCI move: {:?}", e))?;
    let mv = uci
        .to_move(&pos)
        .map_err(|e| anyhow!("Move not legal: {}", e))?;

    pos.play_unchecked(mv);

    Ok(Fen::from_position(&pos, shakmaty::EnPassantMode::Legal).to_string())
}

async fn submit_move(
    client: &ChessRpcClient,
    signer: &Keypair,
    game_id: u64,
    move_str: String,
    next_fen: String,
) -> Result<solana_sdk::signature::Signature> {
    let ix = client.create_record_move_ix(signer.pubkey(), game_id, move_str, next_fen);

    let recent_blockhash = client.rpc.get_latest_blockhash()?;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    );

    client
        .rpc
        .send_and_confirm_transaction(&tx)
        .map_err(|e| anyhow!("Transaction failed: {:?}", e))
}
