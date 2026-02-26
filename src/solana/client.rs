use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

use crate::solana::instructions::*;
use xfchess_game::state::{Game, MoveLog, PlayerProfile};

pub struct SolanaChessClient {
    rpc_client: RpcClient,
    payer: Keypair,
}

impl SolanaChessClient {
    pub fn new(rpc_url: &str, payer: Keypair) -> Result<Self> {
        let rpc_client =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

        Ok(Self { rpc_client, payer })
    }

    pub async fn create_player_profile(&self, _username: String) -> Result<Pubkey> {
        let profile_pda = self.get_player_profile_pda(self.payer.pubkey())?;

        // Re-using the init_profile handler from our unified instructions
        let instruction = crate::solana::instructions::init_profile_ix(
            self.payer.pubkey(), // The program ID is handled in the wrapper now or fetched via crate::PROGRAM_ID
        );

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        println!("Player profile created with signature: {}", signature);

        Ok(profile_pda)
    }

    pub async fn create_new_game(&self, _opponent_pubkey: Option<Pubkey>) -> Result<Pubkey> {
        // In our new Anchor contract, we use u64 game_id.
        // We'll generate one from timestamp for now.
        let game_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Get Program ID - for this wrapper we'll assume it's the one in instructions
        let _pid_str = crate::solana::errors::XfChessError::GameFull; // Use errors module directly
        let program_id = solana_chess_client::XFCHESS_PROGRAM_ID.parse().unwrap();

        let instruction = create_game_ix(program_id, self.payer.pubkey(), game_id);

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        let game_pda =
            Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

        println!("New game (ID: {}) created: {}", game_id, signature);
        Ok(game_pda)
    }

    pub async fn make_move(&self, game_pda: Pubkey, from: u8, to: u8) -> Result<()> {
        let instruction = make_move_instruction(self.payer.pubkey(), game_pda, from, to)?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        println!("Move made with signature: {}", signature);

        Ok(())
    }

    pub async fn finish_game(&self, game_pda: Pubkey, winner: Pubkey) -> Result<()> {
        let instruction = finish_game_instruction(self.payer.pubkey(), game_pda, winner)?;

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        println!("Game finished with signature: {}", signature);

        Ok(())
    }

    fn get_player_profile_pda(&self, player_pubkey: Pubkey) -> Result<Pubkey> {
        // Derive PDA for player profile based on player's public key
        let seeds = &[b"profile", player_pubkey.as_ref()];
        let (pda, _bump_seed) =
            Pubkey::find_program_address(seeds, &crate::PROGRAM_ID.parse().unwrap());
        Ok(pda)
    }

    fn generate_unique_game_pda(&self) -> Result<Pubkey> {
        // Generate a unique game PDA using timestamp and payer pubkey
        use std::time::{SystemTime, UNIX_EPOCH};
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let seeds = &[b"game", &time.to_le_bytes(), self.payer.pubkey().as_ref()];
        let (pda, _bump_seed) =
            Pubkey::find_program_address(seeds, &crate::PROGRAM_ID.parse().unwrap());
        Ok(pda)
    }
}
