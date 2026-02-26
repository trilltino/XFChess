use solana_chess_client::ChessRpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Solana Move Simulation (XFChess) ---");

    // 1. Setup Client and Program
    // We'll use a dummy RPC URL since we're just simulating the TX structure
    let client = ChessRpcClient::new("https://api.devnet.solana.com");
    let program_id = client.program_id;
    println!("Program ID: {}", program_id);

    // 2. Generate a Mock Player Wallet (Session Key Simulation)
    let player = Keypair::new();
    println!("Simulated Patient Wallet: {}", player.pubkey());

    // 3. Define Game Parameters
    let game_id: u64 = 1337;
    let move_str = "e2e4".to_string();
    let next_fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string();

    println!("\nGenerating RecordMove Instruction...");
    println!("Game ID: {}", game_id);
    println!("Move: {}", move_str);

    // 4. Craft the Instruction
    let record_move_ix = client.create_record_move_ix(player.pubkey(), game_id, move_str, next_fen);

    // 5. Wrap in a Transaction
    let mut tx = Transaction::new_with_payer(&[record_move_ix], Some(&player.pubkey()));

    // In a real scenario, we'd fetch a recent blockhash
    // For simulation, we'll just show the structure
    println!("\n--- Transaction Details ---");
    println!("Instruction Type: RecordMove");
    println!("Accounts Involved:");
    for (i, meta) in tx.message.instructions[0].accounts.iter().enumerate() {
        let pubkey = tx.message.account_keys[*meta as usize];
        println!(
            "  {}. Account: {} (Writable: {}, Signer: {})",
            i + 1,
            pubkey,
            tx.message.is_maybe_writable(*meta as usize),
            tx.message.is_signer(*meta as usize)
        );
    }

    println!("\nInstruction Data (Hex):");
    println!("{:02x?}", tx.message.instructions[0].data);

    println!("\n--- Simulation Complete ---");
    println!("Summary: This transaction was crafted locally by the Bevy client.");
    println!("In a real game, this would be sent to the Ephemeral Rollup instantly.");

    Ok(())
}
