#![cfg(feature = "solana")]
use anchor_lang::AccountDeserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use xfchess_game::state::Game;
use xfchess_rules::CompactBoard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let game_id: u64 = std::env::args()
        .nth(1)
        .ok_or("Usage: read_game <game_id>")?
        .parse()?;
    let rpc_url =
        std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "https://api.devnet.solana.com".into());
    let er_url =
        std::env::var("ER_RPC_URL").unwrap_or_else(|_| "https://devnet.magicblock.app".into());
    let (address, _) =
        Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &xfchess_game::ID);
    let rpc = RpcClient::new(rpc_url);
    let (network, account) = match rpc.get_account(&address) {
        Ok(account) => ("devnet", account),
        Err(base_error) => {
            let er = RpcClient::new(er_url);
            let account = er.get_account(&address).map_err(|er_error| {
                format!("Cannot fetch {address}: devnet: {base_error}; ER: {er_error}")
            })?;
            ("ER", account)
        }
    };
    let game = Game::try_deserialize(&mut account.data.as_slice())?;
    if game.game_id != game_id {
        return Err("Account game ID does not match the requested game".into());
    }
    let board = CompactBoard {
        squares: std::array::from_fn(|i| game.board_state[i] as i8),
        castling: game.board_state[64],
        ep_target: game.board_state[65] as i8,
        side_to_move: game.board_state[66] as i8,
    };
    println!("Game ID    : {game_id}");
    println!("Address    : {address}");
    println!("Network    : {network}");
    println!("Owner      : {}", account.owner);
    println!("Status     : {:?}", game.status);
    println!("Result     : {:?}", game.result);
    println!("FEN        : {}", board.to_fen()?);
    println!("Moves      : {}", game.move_count);
    println!("Turn       : {}", game.turn);
    println!("White      : {}", game.white);
    println!("Black      : {}", game.black);
    println!("Wager      : {} lamports", game.wager_amount);
    println!("Delegated  : {}", game.is_delegated);
    println!("Nonce      : {}", game.nonce);
    Ok(())
}
