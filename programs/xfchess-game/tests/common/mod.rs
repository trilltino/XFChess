#![allow(dead_code)]

use anchor_lang::{AccountDeserialize, AccountSerialize, InstructionData, Space, ToAccountMetas};
use chess_logic_on_chain::CompactBoard;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use xfchess_game::state::{
    Game, GameResult, GameStatus, GameType, MatchType, PlayerProfile, SessionDelegation,
};

pub const PROGRAM: &str = "xfchess_game";

pub const MAGIC_PROGRAM: &str = "Magic11111111111111111111111111111111111111";
pub const MAGIC_CONTEXT: &str = "MagicContext1111111111111111111111111111111";

pub fn game_pda(game_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &xfchess_game::ID)
}

pub fn profile_pda(player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], &xfchess_game::ID)
}

pub fn sd_pda(game_id: u64, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"session_delegation",
            &game_id.to_le_bytes(),
            player.as_ref(),
        ],
        &xfchess_game::ID,
    )
}

pub fn uci(s: &str) -> [u8; 5] {
    let mut a = [0u8; 5];
    for (i, b) in s.bytes().enumerate() {
        a[i] = b;
    }
    a
}

pub fn start_board() -> [u8; 68] {
    CompactBoard::starting_position().to_bytes()
}

pub fn board_from_fen(fen: &str) -> [u8; 68] {
    CompactBoard::from_fen(fen).to_bytes()
}

pub fn apply(board: &[u8; 68], mv: &[u8; 5]) -> [u8; 68] {
    let mut g = CompactBoard::from_bytes(board).to_on_chain_game();
    chess_logic_on_chain::validate_and_apply(&mut g, mv).expect("test move must be legal");
    g.to_compact_board().to_bytes()
}

pub fn ec(e: xfchess_game::errors::GameErrorCode) -> u32 {
    6000 + e as u32
}

pub fn program_account<T: AccountSerialize>(value: &T, space: usize) -> Account {
    let mut data = Vec::with_capacity(space);
    value.try_serialize(&mut data).unwrap();
    if data.len() < space {
        data.resize(space, 0);
    }
    Account {
        lamports: 1_000_000_000,
        data,
        owner: xfchess_game::ID,
        executable: false,
        rent_epoch: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn game_account(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    board_state: [u8; 68],
    turn: u16,
    nonce: u64,
    status: GameStatus,
) -> (Pubkey, Account) {
    game_account_with_delegation(
        game_id,
        white,
        black,
        board_state,
        turn,
        nonce,
        status,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn game_account_with_delegation(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    board_state: [u8; 68],
    turn: u16,
    nonce: u64,
    status: GameStatus,
    is_delegated: bool,
) -> (Pubkey, Account) {
    let (pda, bump) = game_pda(game_id);
    let g = Game {
        game_id,
        white,
        black,
        status,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: white,
        result: GameResult::None,
        board_state,
        move_count: turn.saturating_sub(1),
        halfmove_clock: 0,
        turn,
        created_at: 0,
        updated_at: 0,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 0,
        increment_seconds: 0,
        bump,
        is_delegated,
        tournament_id: None,
        nonce,
        draw_offered_by: None,
    };
    (pda, program_account(&g, 8 + Game::INIT_SPACE))
}

pub fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: solana_system_interface::program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn session_account(
    game_id: u64,
    player: Pubkey,
    session_key: Pubkey,
    expires_at: i64,
    enabled: bool,
) -> (Pubkey, Account) {
    let (pda, bump) = sd_pda(game_id, &player);
    let sd = SessionDelegation {
        game_id,
        player,
        session_key,
        expires_at,
        max_batch_len: 0,
        enabled,
        bump,
    };
    (pda, program_account(&sd, 8 + SessionDelegation::INIT_SPACE))
}

pub async fn start(accounts: Vec<(Pubkey, Account)>) -> ProgramTestContext {
    std::env::set_var(
        "SBF_OUT_DIR",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy"),
    );
    let mut pt = ProgramTest::new(PROGRAM, xfchess_game::ID, None);
    for (key, account) in accounts {
        pt.add_account(key, account);
    }
    pt.start_with_context().await
}

pub fn record_move_ix(
    game_id: u64,
    player_wallet: &Pubkey,
    session_signer: &Pubkey,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    parent_nonce: Option<u64>,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_record_move::RecordMove {
        game: game_pda(game_id).0,
        player: *session_signer,
        session_delegation: sd_pda(game_id, player_wallet).0,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::RecordMove {
        game_id,
        move_uci,
        next_board,
        nonce,
        signature: None,
        parent_nonce,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

pub fn undelegate_ix(
    game_id: u64,
    payer: Pubkey,
    magic_context: Pubkey,
    magic_program: Pubkey,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_undelegate_game_ctx::UndelegateGameCtx {
        game: game_pda(game_id).0,
        payer,
        magic_context,
        magic_program,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::UndelegateGame { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

pub fn process_undelegation_ix(
    game_id: u64,
    payer: Pubkey,
    buffer: Pubkey,
    account_seeds: Vec<Vec<u8>>,
) -> Instruction {
    let accounts =
        xfchess_game::__client_accounts_initialize_after_undelegation::InitializeAfterUndelegation {
            base_account: game_pda(game_id).0,
            buffer,
            payer,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None);
    let data = xfchess_game::instruction::ProcessUndelegation { account_seeds }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

pub fn resign_ix(game_id: u64, player: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_resign_game::ResignGame {
        game: game_pda(game_id).0,
        player,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::Resign { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

pub fn cancel_time_check_ix(
    game_id: u64,
    payer: Pubkey,
    task_id: u64,
    magic_program: Pubkey,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_cancel_time_check::CancelTimeCheck {
        payer,
        game: game_pda(game_id).0,
        magic_program,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::CancelTimeCheck {
        args: xfchess_game::CancelTimeCheckArgs { task_id },
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

pub fn claim_timeout_ix(game_id: u64, caller: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_claim_timeout::ClaimTimeout {
        game: game_pda(game_id).0,
        caller,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::ClaimTimeout { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

pub async fn send(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = ctx.last_blockhash;
    let payer_pk = ctx.payer.pubkey();
    let mut signers: Vec<&Keypair> = vec![&ctx.payer];
    signers.extend_from_slice(extra);
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
    tx.sign(&signers, blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .map_err(transaction_error)
}

fn transaction_error(e: BanksClientError) -> TransactionError {
    match e {
        BanksClientError::TransactionError(te) => te,
        BanksClientError::SimulationError { err, .. } => err,
        other => panic!("unexpected banks error: {other:?}"),
    }
}

pub fn custom_code(err: &TransactionError) -> Option<u32> {
    match err {
        TransactionError::InstructionError(_, InstructionError::Custom(c)) => Some(*c),
        _ => None,
    }
}

pub async fn fetch_game(ctx: &mut ProgramTestContext, game_id: u64) -> Game {
    let acc = ctx
        .banks_client
        .get_account(game_pda(game_id).0)
        .await
        .unwrap()
        .expect("game account missing");
    Game::try_deserialize(&mut &acc.data[..]).unwrap()
}

pub async fn fetch_profile(ctx: &mut ProgramTestContext, player: &Pubkey) -> PlayerProfile {
    let acc = ctx
        .banks_client
        .get_account(profile_pda(player).0)
        .await
        .unwrap()
        .expect("profile account missing");
    PlayerProfile::try_deserialize(&mut &acc.data[..]).unwrap()
}
