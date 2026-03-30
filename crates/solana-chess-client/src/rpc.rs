use crate::XFCHESS_PROGRAM_ID;
use anchor_lang::{AccountDeserialize, InstructionData};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use xfchess_game::constants::{
    GAME_SEED, MOVE_LOG_SEED, PROFILE_SEED, SESSION_DELEGATION_SEED, WAGER_ESCROW_SEED,
};
use xfchess_game::state::{Game, GameType, PlayerProfile};

/// Get the system program Pubkey (11111...1).
fn system_program_id() -> Pubkey {
    "11111111111111111111111111111111".parse().unwrap()
}

pub type Error = anyhow::Error;

pub struct ChessRpcClient {
    pub rpc: RpcClient,
    pub program_id: Pubkey,
}

impl ChessRpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            rpc: RpcClient::new(url.to_string()),
            program_id: XFCHESS_PROGRAM_ID.parse().expect("Invalid program ID"),
        }
    }

    pub fn derive_pda(&self, seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &self.program_id).0
    }

    pub fn get_game_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[GAME_SEED, &game_id.to_le_bytes()])
    }

    pub fn get_escrow_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()])
    }

    pub fn get_profile_pda(&self, wallet: &Pubkey) -> Pubkey {
        self.derive_pda(&[PROFILE_SEED, wallet.as_ref()])
    }

    pub fn get_move_log_pda(&self, game_id: u64) -> Pubkey {
        self.derive_pda(&[MOVE_LOG_SEED, &game_id.to_le_bytes()])
    }

    pub fn get_session_delegation_pda(&self, game_id: u64, player: &Pubkey) -> Pubkey {
        self.derive_pda(&[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            player.as_ref(),
        ])
    }

    pub fn fetch_game(&self, game_id: u64) -> Result<Option<Game>> {
        let pda = self.get_game_pda(game_id);
        let data = self.rpc.get_account_data(&pda)?;
        if data.len() < 8 {
            return Ok(None);
        }
        // Anchor discriminator is first 8 bytes
        let mut reader = &data[8..];
        let game = Game::try_deserialize(&mut reader)?;
        Ok(Some(game))
    }

    pub fn fetch_all_games(&self) -> Result<Vec<Game>> {
        let accounts = self.rpc.get_program_accounts(&self.program_id)?;
        let mut games = Vec::new();

        for (_, account) in accounts {
            if account.data.len() >= 8 {
                let mut reader = &account.data[8..];
                if let Ok(game) = Game::try_deserialize(&mut reader) {
                    games.push(game);
                }
            }
        }
        Ok(games)
    }

    pub fn fetch_profile(&self, wallet: &Pubkey) -> Result<Option<PlayerProfile>> {
        let pda = self.get_profile_pda(wallet);
        match self.rpc.get_account_data(&pda) {
            Ok(data) => {
                if data.len() >= 8 {
                    let mut reader = &data[8..];
                    let profile = PlayerProfile::try_deserialize(&mut reader)?;
                    Ok(Some(profile))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Creates an instruction to initialize a player profile.
    pub fn create_init_profile_ix(&self, player: Pubkey) -> Instruction {
        let profile_pda = self.get_profile_pda(&player);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(profile_pda, false),
                AccountMeta::new(player, true),
                AccountMeta::new_readonly(system_program_id(), false),
            ],
            data: xfchess_game::instruction::InitProfile {}.data(),
        }
    }

    /// Creates an instruction to create a new game.
    pub fn create_create_game_ix(
        &self,
        player: Pubkey,
        game_id: u64,
        wager_amount: u64,
        game_type: GameType,
    ) -> Instruction {
        let game_pda = self.get_game_pda(game_id);
        let move_log_pda = self.get_move_log_pda(game_id);
        let escrow_pda = self.get_escrow_pda(game_id);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(game_pda, false),
                AccountMeta::new(move_log_pda, false),
                AccountMeta::new(escrow_pda, false),
                AccountMeta::new(player, true),
                AccountMeta::new_readonly(system_program_id(), false),
            ],
            data: xfchess_game::instruction::CreateGame {
                game_id,
                wager_amount,
                game_type,
            }
            .data(),
        }
    }

    /// Creates an instruction to join an existing game.
    pub fn create_join_game_ix(&self, player: Pubkey, game_id: u64) -> Instruction {
        let game_pda = self.get_game_pda(game_id);
        let escrow_pda = self.get_escrow_pda(game_id);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(game_pda, false),
                AccountMeta::new(escrow_pda, false),
                AccountMeta::new(player, true),
                AccountMeta::new_readonly(system_program_id(), false),
            ],
            data: xfchess_game::instruction::JoinGame { game_id }.data(),
        }
    }

    /// Creates an instruction to record a move on-chain.
    pub fn create_record_move_ix(
        &self,
        player: Pubkey,
        game_id: u64,
        move_str: String,
        next_fen: String,
        nonce: u64,
        signature: Option<Vec<u8>>,
    ) -> Instruction {
        let game_pda = self.get_game_pda(game_id);
        let move_log_pda = self.get_move_log_pda(game_id);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(game_pda, false),
                AccountMeta::new(move_log_pda, false),
                AccountMeta::new(player, true),
            ],
            data: xfchess_game::instruction::RecordMove {
                game_id,
                move_str,
                next_fen,
                nonce,
                signature,
            }
            .data(),
        }
    }

    /// Creates an instruction to finalize a game.
    pub fn create_finalize_game_ix(
        &self,
        _player: Pubkey,
        game_id: u64,
        white: Pubkey,
        black: Pubkey,
        result: xfchess_game::state::GameResult,
    ) -> Instruction {
        let game_pda = self.get_game_pda(game_id);
        let white_profile = self.get_profile_pda(&white);
        let black_profile = self.get_profile_pda(&black);
        let escrow_pda = self.get_escrow_pda(game_id);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(game_pda, false),
                AccountMeta::new(white_profile, false),
                AccountMeta::new(black_profile, false),
                AccountMeta::new(white, true),
                AccountMeta::new(black, false),
                AccountMeta::new(escrow_pda, false),
                AccountMeta::new_readonly(system_program_id(), false),
            ],
            data: xfchess_game::instruction::FinalizeGame { game_id, result }.data(),
        }
    }

    /// Creates an instruction to withdraw an expired wager.
    pub fn create_withdraw_expired_wager_ix(&self, player: Pubkey, game_id: u64) -> Instruction {
        let game_pda = self.get_game_pda(game_id);
        let escrow_pda = self.get_escrow_pda(game_id);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(game_pda, false),
                AccountMeta::new(escrow_pda, false),
                AccountMeta::new(player, true),
                AccountMeta::new_readonly(system_program_id(), false),
            ],
            data: xfchess_game::instruction::WithdrawExpiredWager { game_id }.data(),
        }
    }

    pub fn create_authorize_session_key_ix(
        &self,
        player: Pubkey,
        game_id: u64,
        session_pubkey: Pubkey,
    ) -> Instruction {
        let delegation_pda = self.get_session_delegation_pda(game_id, &player);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(delegation_pda, false),
                AccountMeta::new(player, true),
                AccountMeta::new_readonly(system_program_id(), false),
            ],
            data: xfchess_game::instruction::AuthorizeSessionKey {
                game_id,
                session_pubkey,
            }
            .data(),
        }
    }

    pub fn create_revoke_session_key_ix(&self, player: Pubkey, game_id: u64) -> Instruction {
        let delegation_pda = self.get_session_delegation_pda(game_id, &player);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(delegation_pda, false),
                AccountMeta::new(player, true),
            ],
            data: xfchess_game::instruction::RevokeSessionKey { game_id }.data(),
        }
    }

    pub fn create_commit_move_batch_ix(
        &self,
        game_id: u64,
        moves: Vec<String>,
        next_fens: Vec<String>,
        white: Pubkey,
        black: Pubkey,
        white_session: Pubkey,
        black_session: Pubkey,
    ) -> Instruction {
        let game_pda = self.get_game_pda(game_id);
        let move_log_pda = self.get_move_log_pda(game_id);
        let white_delegation = self.get_session_delegation_pda(game_id, &white);
        let black_delegation = self.get_session_delegation_pda(game_id, &black);

        Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(game_pda, false),
                AccountMeta::new(move_log_pda, false),
                AccountMeta::new(white_session, true),
                AccountMeta::new(black_session, true),
                AccountMeta::new_readonly(white_delegation, false),
                AccountMeta::new_readonly(black_delegation, false),
            ],
            data: xfchess_game::instruction::CommitMoveBatch {
                game_id,
                moves,
                next_fens,
            }
            .data(),
        }
    }
}
