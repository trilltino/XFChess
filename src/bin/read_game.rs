use borsh::BorshDeserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX";
const RPC_URL: &str = "https://api.devnet.solana.com";
const ER_RPC_URL: &str = "https://devnet-eu.magicblock.app/";
/// MagicBlock Delegation Program — owns delegated accounts on L1
const DELEGATION_PROGRAM: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh";

#[derive(BorshDeserialize, Debug)]
struct MoveLog {
    pub game_id: u64,
    pub moves: Vec<String>,
    pub timestamps: Vec<i64>,
    pub player_signatures: Vec<Vec<u8>>,
    pub nonce: u64,
}

#[derive(BorshDeserialize, Debug)]
struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub status: u8,          // GameStatus enum variant index
    pub result: GameResult,
    pub fen: String,
    pub move_count: u16,
    pub turn: u8,
    pub created_at: i64,
    pub updated_at: i64,
    pub wager_amount: u64,
    pub wager_token: Option<Pubkey>,
    pub game_type: u8,       // GameType enum variant index
    pub bump: u8,
}

#[derive(BorshDeserialize, Debug)]
enum GameResult {
    None,
    Winner(Pubkey),
    Draw,
}

fn main() {
    let game_id: u64 = std::env::args()
        .nth(1)
        .expect("Usage: read_game <game_id>")
        .parse()
        .expect("game_id must be a u64");

    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let id_bytes = game_id.to_le_bytes();

    let game_pda = Pubkey::find_program_address(&[b"game", &id_bytes], &program_id).0;
    let move_log_pda = Pubkey::find_program_address(&[b"move_log", &id_bytes], &program_id).0;

    let rpc = RpcClient::new(RPC_URL.to_string());
    let er_rpc = RpcClient::new(ER_RPC_URL.to_string());
    let delegation_prog = Pubkey::from_str(DELEGATION_PROGRAM).unwrap();

    println!("Game ID  : {}", game_id);
    println!("game PDA : {}", game_pda);
    println!("log PDA  : {}", move_log_pda);
    println!();

    // --- Game account ---
    match rpc.get_account(&game_pda) {
        Ok(acc) => {
            println!("game account: {} bytes, owner: {}", acc.data.len(), acc.owner);
            if acc.data.len() > 8 {
                // skip 8-byte Anchor discriminator
                match Game::deserialize(&mut &acc.data[8..]) {
                    Ok(game) => {
                        let status = ["WaitingForOpponent","Active","Inactive","Disputed","Cancelled","Finished","Expired"].get(game.status as usize).unwrap_or(&"Unknown");
                        let result = match &game.result {
                            GameResult::None => "None".to_string(),
                            GameResult::Winner(pk) => format!("Winner({})", pk),
                            GameResult::Draw => "Draw".to_string(),
                        };
                        println!("  status     : {}", status);
                        println!("  result     : {}", result);
                        println!("  fen        : {}", game.fen);
                        println!("  move_count : {}", game.move_count);
                        println!("  turn       : {}", game.turn);
                        println!("  white      : {}", game.white);
                        println!("  black      : {}", game.black);
                        println!("  wager      : {} lamports", game.wager_amount);
                    }
                    Err(e) => println!("  [decode error] {}", e),
                }
            } else {
                println!("  [account has no data — not initialized or empty]");
            }
        }
        Err(_) => {
            print!("game account NOT FOUND on devnet");
            // Check if it's still delegated (owned by delegation program on ER)
            match er_rpc.get_account(&game_pda) {
                Ok(acc) if acc.owner == delegation_prog => println!(" — still delegated on ER (state not yet committed to L1)"),
                Ok(acc) => println!(" — found on ER, owner: {}", acc.owner),
                Err(_) => println!(" — not found on ER either (undelegation may have failed)"),
            }
        }
    }

    println!();

    // --- MoveLog account (devnet) ---
    match rpc.get_account(&move_log_pda) {
        Ok(acc) => {
            println!("move_log (devnet): {} bytes, owner: {}", acc.data.len(), acc.owner);
            if acc.data.len() > 8 {
                match MoveLog::deserialize(&mut &acc.data[8..]) {
                    Ok(log) => {
                        println!("  nonce : {}", log.nonce);
                        println!("  moves : {}", log.moves.len());
                        for (i, mv) in log.moves.iter().enumerate() {
                            let ts = log.timestamps.get(i).copied().unwrap_or(0);
                            println!("    [{}] {} (ts: {})", i + 1, mv, ts);
                        }
                    }
                    Err(e) => println!("  [decode error] {}", e),
                }
            } else {
                println!("  [account has no data]");
            }
        }
        Err(_) => {
            print!("move_log NOT FOUND on devnet");
            match er_rpc.get_account(&move_log_pda) {
                Ok(acc) => {
                    println!(" — found on ER ({} bytes, owner: {})", acc.data.len(), acc.owner);
                    if acc.data.len() > 8 {
                        match MoveLog::deserialize(&mut &acc.data[8..]) {
                            Ok(log) => {
                                println!("  nonce : {}", log.nonce);
                                println!("  moves : {}", log.moves.len());
                                for (i, mv) in log.moves.iter().enumerate() {
                                    let ts = log.timestamps.get(i).copied().unwrap_or(0);
                                    println!("    [{}] {} (ts: {})", i + 1, mv, ts);
                                }
                            }
                            Err(e) => println!("  [decode error] {}", e),
                        }
                    }
                }
                Err(_) => println!(" — not found on ER either"),
            }
        }
    }
}
