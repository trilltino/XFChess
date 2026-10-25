//! SBF instruction regressions. Public signer roles use Banks metadata execution
//! without cryptographic verification; runtime sysvars and CPIs are executed by the VM.
#[path = "common/agave.rs"]
pub mod agave;
use crate::agave::ProgramTest;
use anchor_lang::{prelude::*, AccountSerialize, InstructionData};
use anyhow::Context as _;
use solana_program::program_error::ProgramError;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    signature::Signer,
    transaction::{Transaction, TransactionError},
};
use std::sync::Mutex;
use xfchess_game::{constants::*, errors::GameErrorCode, state::*};

const NOW: i64 = 1_800_000_000;
const GAME_ID: u64 = 42;
static RUNTIME_LOCK: Mutex<()> = Mutex::new(());

fn with_sbf_runtime(test: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    let _lock = RUNTIME_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::set_var(
        "SBF_OUT_DIR",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy"),
    );
    test()
}

#[derive(Clone, Debug, PartialEq)]
struct Fixture {
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
    signer: bool,
    writable: bool,
    executable: bool,
}

impl Fixture {
    fn system(key: Pubkey) -> Self {
        Self {
            key,
            owner: System::id(),
            lamports: 1_000_000_000,
            data: vec![],
            signer: false,
            writable: true,
            executable: false,
        }
    }

    fn program<T: AccountSerialize>(key: Pubkey, value: &T, space: usize) -> anyhow::Result<Self> {
        let mut account = Self::system(key);
        account.owner = xfchess_game::ID;
        value.try_serialize(&mut account.data)?;
        account.data.resize(space, 0);
        Ok(account)
    }

    fn info(&mut self) -> AccountInfo<'_> {
        AccountInfo::new(
            &self.key,
            self.signer,
            self.writable,
            &mut self.lamports,
            &mut self.data,
            &self.owner,
            self.executable,
        )
    }
}

fn system_program() -> Fixture {
    let mut account = Fixture::system(System::id());
    account.executable = true;
    account.writable = false;
    account
}

fn dispatch(
    accounts: &mut [Fixture],
    data: &[u8],
) -> anyhow::Result<std::result::Result<(), ProgramError>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut test = ProgramTest::new("xfchess_game", xfchess_game::ID, None);
        // Anchor's optional-account sentinel is the current program ID. Keep
        // both program markers backed by the VM's executable accounts.
        let is_builtin = |account: &Fixture| {
            account.executable && (account.key == System::id() || account.key == xfchess_game::ID)
        };
        for account in accounts.iter().filter(|account| !is_builtin(account)) {
            test.add_account(
                account.key,
                Account {
                    lamports: account.lamports,
                    data: account.data.clone(),
                    owner: account.owner,
                    executable: account.executable,
                    rent_epoch: 0,
                },
            );
        }
        let context = test.start_with_context().await;
        context.set_sysvar(&Clock {
            unix_timestamp: NOW,
            ..Clock::default()
        });
        let instruction = Instruction {
            program_id: xfchess_game::ID,
            accounts: accounts
                .iter()
                .map(|account| {
                    if account.writable {
                        AccountMeta::new(account.key, account.signer)
                    } else {
                        AccountMeta::new_readonly(account.key, account.signer)
                    }
                })
                .collect(),
            data: data.to_vec(),
        };
        let mut transaction =
            Transaction::new_with_payer(&[instruction], Some(&context.payer.pubkey()));
        transaction.partial_sign(&[&context.payer], context.last_blockhash);
        let outcome = context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await?
            .result;
        for account in accounts.iter_mut().filter(|account| !is_builtin(account)) {
            if let Some(updated) = context.banks_client.get_account(account.key).await? {
                account.owner = updated.owner;
                account.lamports = updated.lamports;
                account.data = updated.data;
                account.executable = updated.executable;
            } else {
                account.owner = System::id();
                account.lamports = 0;
                account.data.clear();
                account.executable = false;
            }
        }
        match outcome {
            Ok(()) => Ok(Ok(())),
            Err(TransactionError::InstructionError(_, error)) => Ok(Err(ProgramError::try_from(
                error,
            )
            .context("unexpected VM instruction error")?)),
            Err(error) => Err(anyhow::anyhow!(
                "VM transaction failed outside the program: {error}"
            )),
        }
    })
}

fn code(error: GameErrorCode) -> ProgramError {
    ProgramError::Custom(6000 + error as u32)
}

fn seeds_error() -> ProgramError {
    ProgramError::Custom(anchor_lang::error::ErrorCode::ConstraintSeeds as u32)
}

fn game_key() -> Pubkey {
    Pubkey::find_program_address(&[GAME_SEED, &GAME_ID.to_le_bytes()], &xfchess_game::ID).0
}

fn recovery_accounts() -> Vec<Fixture> {
    let mut game = Fixture::system(game_key());
    game.owner = xfchess_game::ID;
    let escrow = Pubkey::find_program_address(
        &[WAGER_ESCROW_SEED, &GAME_ID.to_le_bytes()],
        &xfchess_game::ID,
    )
    .0;
    let treasury = Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &xfchess_game::ID).0;
    let mut authority = Fixture::system(dispute_authority::ID);
    authority.signer = true; // Simulates the *authorized* runtime signer, no secret needed.
    vec![
        game,
        Fixture::system(escrow),
        Fixture::system(treasury),
        Fixture::system(Pubkey::new_unique()),
        Fixture::system(Pubkey::new_unique()),
        authority,
        system_program(),
    ]
}

#[test]
fn authorized_recovery_without_identity_preserves_all_accounts_on_retry() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let data = xfchess_game::instruction::RecoverStuckDelegation { game_id: GAME_ID }.data();
        for pot in [0, 1, 2_000_000_001, u64::MAX / 2] {
            for duplicate in [false, true] {
                let mut accounts = recovery_accounts();
                accounts[1].lamports = pot;
                if duplicate {
                    accounts[4].key = accounts[3].key;
                }
                let before = accounts.clone();
                for _ in 0..2 {
                    assert_eq!(
                        dispatch(&mut accounts, &data)?,
                        Err(code(GameErrorCode::RecoveryIdentityUnavailable))
                    );
                    assert_eq!(
                        accounts, before,
                        "no escrow, rent, recipient or treasury mutation"
                    );
                }
            }
        }
        Ok(())
    })
}

#[test]
fn recovery_rejects_unauthorized_signer_and_populated_game() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let data = xfchess_game::instruction::RecoverStuckDelegation { game_id: GAME_ID }.data();
        let mut accounts = recovery_accounts();
        accounts[5].key = Pubkey::new_unique();
        let before = accounts.clone();
        assert_eq!(
            dispatch(&mut accounts, &data)?,
            Err(code(GameErrorCode::UnauthorizedDisputeResolution))
        );
        assert_eq!(accounts, before);
        accounts = recovery_accounts();
        accounts[0].data = vec![1; 8];
        let before = accounts.clone();
        assert_eq!(
            dispatch(&mut accounts, &data)?,
            Err(code(GameErrorCode::GameNotStuckDelegation))
        );
        assert_eq!(accounts, before);
        Ok(())
    })
}

fn callback_accounts(base: Pubkey) -> Vec<Fixture> {
    let mut buffer = Fixture::system(
        Pubkey::find_program_address(
            &[b"undelegate-buffer", base.as_ref()],
            &ephemeral_rollups_sdk::id(),
        )
        .0,
    );
    buffer.owner = ephemeral_rollups_sdk::id();
    buffer.writable = false;
    vec![
        Fixture::system(base),
        buffer,
        Fixture::system(Pubkey::new_unique()),
        system_program(),
    ]
}

#[test]
fn callback_rejects_malformed_seeds_and_wrong_game_without_mutation() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        for seeds in [
            vec![],
            vec![GAME_SEED.to_vec()],
            vec![GAME_SEED.to_vec(), vec![0; 7]],
            vec![GAME_SEED.to_vec(), vec![0; 9]],
            vec![b"profile".to_vec(), GAME_ID.to_le_bytes().to_vec()],
            vec![
                GAME_SEED.to_vec(),
                GAME_ID.to_le_bytes().to_vec(),
                vec![255],
            ],
            vec![GAME_SEED.to_vec(), (GAME_ID + 1).to_le_bytes().to_vec()],
        ] {
            let mut accounts = callback_accounts(game_key());
            let before = accounts.clone();
            let data = xfchess_game::instruction::ProcessUndelegation {
                account_seeds: seeds,
            }
            .data();
            assert_eq!(dispatch(&mut accounts, &data)?, Err(seeds_error()));
            assert_eq!(accounts, before);
        }
        Ok(())
    })
}

#[test]
fn callback_checks_buffer_system_program_and_sdk_signature_owner() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let data = xfchess_game::instruction::ProcessUndelegation {
            account_seeds: vec![GAME_SEED.to_vec(), GAME_ID.to_le_bytes().to_vec()],
        }
        .data();
        for case in 0..4 {
            let mut accounts = callback_accounts(game_key());
            let expected = match case {
                0 => {
                    accounts[1].key = Pubkey::new_unique();
                    seeds_error()
                }
                1 => {
                    accounts[3].key = Pubkey::new_unique();
                    ProgramError::Custom(anchor_lang::error::ErrorCode::InvalidProgramId as u32)
                }
                2 => ProgramError::MissingRequiredSignature,
                _ => {
                    accounts[1].signer = true;
                    accounts[1].owner = System::id();
                    ProgramError::InvalidAccountOwner
                }
            };
            let before = accounts.clone();
            assert_eq!(dispatch(&mut accounts, &data)?, Err(expected));
            assert_eq!(accounts, before);
        }
        Ok(())
    })
}

fn profile_accounts(player: Pubkey) -> anyhow::Result<Vec<Fixture>> {
    let profile_key =
        Pubkey::find_program_address(&[PROFILE_SEED, player.as_ref()], &xfchess_game::ID).0;
    let username_key =
        Pubkey::find_program_address(&[USERNAME_SEED, b"secureplayer"], &xfchess_game::ID).0;
    let profile = PlayerProfile {
        authority: player,
        wins: 7,
        losses: 3,
        games_played: 10,
        elo_rating: 175_000.0,
        elo_bullet: 160_000.0,
        elo_blitz: 170_000.0,
        elo_rapid: 180_000.0,
        is_verified: true,
        created_at: 1234,
        ..PlayerProfile::default()
    };
    let record = UsernameRecord {
        owner: player,
        created_at: 1234,
    };
    let mut wallet = Fixture::system(player);
    wallet.signer = true;
    Ok(vec![
        Fixture::program(profile_key, &profile, 8 + PlayerProfile::INIT_SPACE)?,
        Fixture::program(username_key, &record, 8 + UsernameRecord::LEN)?,
        wallet,
        system_program(),
    ])
}

fn profile_data() -> Vec<u8> {
    xfchess_game::instruction::InitProfile {
        username: "secureplayer".into(),
        country: "GB".into(),
        date_of_birth: 631_152_000,
    }
    .data()
}

#[test]
fn profile_rejects_game_and_other_players_profile_without_mutation() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let player = Pubkey::new_unique();
        for key in [
            game_key(),
            Pubkey::find_program_address(
                &[PROFILE_SEED, Pubkey::new_unique().as_ref()],
                &xfchess_game::ID,
            )
            .0,
        ] {
            let mut accounts = profile_accounts(player)?;
            accounts[0].key = key;
            if key == game_key() {
                accounts[0] = Fixture::program(
                    key,
                    &waiting_game(player, NOW),
                    8 + Game::INIT_SPACE.max(PlayerProfile::INIT_SPACE),
                )?;
            }
            let before = accounts.clone();
            assert_eq!(
                dispatch(&mut accounts, &profile_data())?,
                Err(seeds_error())
            );
            assert_eq!(
                accounts, before,
                "profile and username bytes/lamports must be unchanged"
            );
        }
        Ok(())
    })
}

#[test]
fn canonical_profile_reinitialization_preserves_accumulated_state() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let player = Pubkey::new_unique();
        let mut accounts = profile_accounts(player)?;
        dispatch(&mut accounts, &profile_data())??;
        let profile = PlayerProfile::try_deserialize(&mut accounts[0].data.as_slice())?;
        assert_eq!(profile.authority, player);
        assert_eq!(
            (profile.wins, profile.losses, profile.games_played),
            (7, 3, 10)
        );
        assert_eq!(
            (
                profile.elo_rating,
                profile.elo_bullet,
                profile.elo_blitz,
                profile.elo_rapid
            ),
            (175_000.0, 160_000.0, 170_000.0, 180_000.0)
        );
        assert!(profile.is_verified);
        assert_eq!(profile.created_at, 1234);
        assert_eq!(profile.username, "secureplayer");
        Ok(())
    })
}

#[test]
fn canonical_nonempty_profile_with_wrong_owner_is_rejected() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let mut accounts = profile_accounts(Pubkey::new_unique())?;
        accounts[0].owner = System::id();
        let before = accounts.clone();
        assert_eq!(
            dispatch(&mut accounts, &profile_data())?,
            Err(code(GameErrorCode::UnauthorizedAccess))
        );
        assert_eq!(accounts, before);
        Ok(())
    })
}

fn waiting_game(white: Pubkey, created_at: i64) -> Game {
    Game {
        game_id: GAME_ID,
        white,
        black: Pubkey::default(),
        status: GameStatus::WaitingForOpponent,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: white,
        result: GameResult::None,
        board_state: [0; 68],
        move_count: 0,
        halfmove_clock: 0,
        turn: 1,
        created_at,
        updated_at: created_at,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 0,
        bump: Pubkey::find_program_address(&[GAME_SEED, &GAME_ID.to_le_bytes()], &xfchess_game::ID)
            .1,
        is_delegated: false,
        tournament_id: None,
        nonce: 0,
        draw_offered_by: None,
    }
}

#[test]
fn wager_expiry_rejects_overflow_and_exact_boundary_without_mutation() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let white = Pubkey::new_unique();
        let data = xfchess_game::instruction::WithdrawExpiredWager { game_id: GAME_ID }.data();
        for (created_at, expected) in [
            (i64::MAX, code(GameErrorCode::ArithmeticOverflow)),
            (i64::MAX - 86_399, code(GameErrorCode::ArithmeticOverflow)),
            (NOW - 86_400, code(GameErrorCode::GameNotExpired)),
        ] {
            let game = waiting_game(white, created_at);
            let mut wallet = Fixture::system(white);
            wallet.signer = true;
            let escrow = Pubkey::find_program_address(
                &[WAGER_ESCROW_SEED, &GAME_ID.to_le_bytes()],
                &xfchess_game::ID,
            )
            .0;
            let mut absent = Fixture::system(xfchess_game::ID);
            absent.executable = true;
            let mut accounts = vec![
                Fixture::program(game_key(), &game, 8 + Game::INIT_SPACE)?,
                Fixture::system(escrow),
                wallet,
                system_program(),
                absent.clone(),
                absent.clone(),
                absent.clone(),
                absent,
            ];
            accounts.push(Fixture::system(
                Pubkey::find_program_address(
                    &[RecoveryIdentity::SEED, &GAME_ID.to_le_bytes()],
                    &xfchess_game::ID,
                )
                .0,
            ));
            let before = accounts.clone();
            assert_eq!(dispatch(&mut accounts, &data)?, Err(expected));
            assert_eq!(accounts, before);
        }
        Ok(())
    })
}

fn session_accounts(
    player: Pubkey,
    key: Pubkey,
    expires_at: i64,
    games: u16,
    enabled: bool,
) -> anyhow::Result<Vec<Fixture>> {
    let (pda, bump) = Pubkey::find_program_address(
        &[GlobalSessionDelegation::SEED, player.as_ref()],
        &xfchess_game::ID,
    );
    let session = GlobalSessionDelegation {
        player,
        session_key: key,
        expires_at,
        spending_limit: 100_000_000,
        total_spent: 90_000_000,
        max_wager: 20_000_000,
        games_remaining: games,
        enabled,
        bump,
    };
    let mut wallet = Fixture::system(player);
    wallet.signer = true;
    Ok(vec![
        Fixture::program(pda, &session, 8 + GlobalSessionDelegation::INIT_SPACE)?,
        wallet,
        system_program(),
    ])
}

fn authorize_data(key: Pubkey) -> Vec<u8> {
    xfchess_game::instruction::AuthorizeGlobalSession {
        args: xfchess_game::account_ix::global_session_ix::AuthorizeGlobalSessionArgs {
            session_key: key,
            duration_secs: Some(3600),
            spending_limit: Some(200_000_000),
            max_wager: Some(30_000_000),
            games: Some(5),
            deposit_lamports: 0,
        },
    }
    .data()
}

#[test]
fn active_session_cannot_reset_allowance() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let mut accounts =
            session_accounts(Pubkey::new_unique(), Pubkey::new_unique(), NOW + 1, 1, true)?;
        let before = accounts.clone();
        assert_eq!(
            dispatch(&mut accounts, &authorize_data(Pubkey::new_unique()))?,
            Err(code(GameErrorCode::GlobalSessionAlreadyActive))
        );
        assert_eq!(accounts, before);
        Ok(())
    })
}

#[test]
fn wallet_reauthorization_resets_only_expired_exhausted_or_revoked_sessions() -> anyhow::Result<()>
{
    with_sbf_runtime(|| {
        for (expires, games, enabled) in [(NOW, 1, true), (NOW + 1, 0, true), (NOW + 1, 1, false)] {
            let old = Pubkey::new_unique();
            let new = Pubkey::new_unique();
            let player = Pubkey::new_unique();
            let mut accounts = session_accounts(player, old, expires, games, enabled)?;
            let balance = accounts[0].lamports;
            dispatch(&mut accounts, &authorize_data(new))??;
            let session =
                GlobalSessionDelegation::try_deserialize(&mut accounts[0].data.as_slice())?;
            assert_eq!(session.player, player);
            assert_eq!(session.session_key, new);
            assert_ne!(session.session_key, old);
            assert_eq!(
                (
                    session.total_spent,
                    session.spending_limit,
                    session.max_wager
                ),
                (0, 200_000_000, 30_000_000)
            );
            assert_eq!(
                (session.expires_at, session.games_remaining, session.enabled),
                (NOW + 3600, 5, true)
            );
            assert_eq!(accounts[0].lamports, balance, "unused deposit is preserved");
            let mut stale_signer = Fixture::system(old);
            stale_signer.signer = true;
            let escrow = Pubkey::find_program_address(
                &[WAGER_ESCROW_SEED, &GAME_ID.to_le_bytes()],
                &xfchess_game::ID,
            )
            .0;
            let mut spend_accounts = vec![
                accounts[0].clone(),
                stale_signer,
                Fixture::system(player),
                Fixture::system(game_key()),
                Fixture::system(escrow),
                system_program(),
            ];
            let before = spend_accounts.clone();
            let spend = xfchess_game::instruction::GlobalCreateGame {
                game_id: GAME_ID,
                wager_amount: 0,
                match_type: MatchType::Free,
                platform_fee: 0,
                base_time_seconds: 300,
                increment_seconds: 0,
            }
            .data();
            assert_eq!(
                dispatch(&mut spend_accounts, &spend)?,
                Err(code(GameErrorCode::InvalidSessionKey))
            );
            assert_eq!(
                spend_accounts, before,
                "rotated key cannot touch the leftover deposit"
            );
        }
        Ok(())
    })
}

#[test]
fn session_key_cannot_replace_player_wallet_authorization() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let mut accounts =
            session_accounts(Pubkey::new_unique(), Pubkey::new_unique(), NOW, 0, true)?;
        accounts[1].signer = false;
        let before = accounts.clone();
        assert!(dispatch(&mut accounts, &authorize_data(Pubkey::new_unique()))?.is_err());
        assert_eq!(accounts, before);
        let old =
            GlobalSessionDelegation::try_deserialize(&mut accounts[0].data.as_slice())?.session_key;
        accounts[1].key = old;
        accounts[1].signer = true;
        let before = accounts.clone();
        assert_eq!(
            dispatch(&mut accounts, &authorize_data(old))?,
            Err(seeds_error())
        );
        assert_eq!(accounts, before);
        Ok(())
    })
}

fn recovery_v2_accounts(wager: u64) -> anyhow::Result<Vec<Fixture>> {
    let mut accounts = recovery_accounts();
    let (key, bump) = Pubkey::find_program_address(
        &[RecoveryIdentity::SEED, &GAME_ID.to_le_bytes()],
        &xfchess_game::ID,
    );
    let record = RecoveryIdentity {
        game_id: GAME_ID,
        white: accounts[3].key,
        black: accounts[4].key,
        wager_amount: wager,
        status: RecoveryStatus::Delegated,
        bump,
    };
    accounts.push(Fixture::program(
        key,
        &record,
        8 + RecoveryIdentity::INIT_SPACE,
    )?);
    Ok(accounts)
}

#[test]
fn authorized_v2_recovery_rejects_either_forged_recipient_atomically() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
        for index in [3, 4] {
            let mut accounts = recovery_v2_accounts(100_000_000)?;
            accounts[index].key = Pubkey::new_unique();
            let before = accounts.clone();
            assert_eq!(
                dispatch(&mut accounts, &data)?,
                Err(code(GameErrorCode::RecoveryIdentityMismatch))
            );
            assert_eq!(accounts, before);
        }
        Ok(())
    })
}

#[test]
fn authenticated_zero_wager_recovery_succeeds_preserves_donations_and_rejects_retry(
) -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
        let mut accounts = recovery_v2_accounts(0)?;
        accounts[3].lamports = 0;
        accounts[4].lamports = 0;
        let escrow = accounts[1].lamports;
        let white = accounts[3].lamports;
        let black = accounts[4].lamports;
        let rent = accounts[0].lamports;
        let treasury = accounts[2].lamports;
        dispatch(&mut accounts, &data)??;
        assert_eq!(accounts[1].lamports, escrow);
        assert_eq!((accounts[3].lamports, accounts[4].lamports), (white, black));
        assert_eq!(
            (accounts[0].lamports, accounts[2].lamports),
            (0, treasury + rent)
        );
        assert_eq!(
            RecoveryIdentity::try_deserialize(&mut accounts[7].data.as_slice())?.status,
            RecoveryStatus::Recovered
        );
        let before = accounts.clone();
        // The VM purges the rent-drained Game. Its owner guard rejects a
        // second recovery before the terminal record can be consumed again.
        assert_eq!(
            dispatch(&mut accounts, &data)?,
            Err(code(GameErrorCode::GameNotStuckDelegation))
        );
        assert_eq!(accounts, before);
        Ok(())
    })
}

#[test]
fn recovery_record_binding_is_immutable_and_cannot_delegate_twice() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let mut game = waiting_game(white, NOW);
    let mut record = RecoveryIdentity::from_created_game(&game, 255)?;
    assert_eq!(record.status, RecoveryStatus::Waiting);
    assert_eq!(record.black, Pubkey::default());
    assert!(record.mark_delegated(&game).is_err());
    game.black = Pubkey::new_unique();
    game.status = GameStatus::Active;
    record.bind_joined_game(&game)?;
    assert_eq!(record.black, game.black);
    assert!(record.bind_joined_game(&game).is_err());
    record.mark_delegated(&game)?;
    assert_eq!(record.status, RecoveryStatus::Delegated);
    game.is_delegated = true;
    assert!(record.mark_delegated(&game).is_err());
    game.white = Pubkey::new_unique();
    assert!(record.require_game(&game).is_err());
    assert_eq!(8 + RecoveryIdentity::INIT_SPACE, 90);
    assert_eq!(Rent::default().minimum_balance(90), 1_517_280);
    Ok(())
}

#[test]
fn ordinary_settlement_must_supply_canonical_record_and_disables_recovery() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        assert!(xfchess_game::recovery_ix::close_identity(&[], GAME_ID).is_err());
        let mut accounts = recovery_v2_accounts(0)?;
        xfchess_game::recovery_ix::close_identity(&[accounts[7].info()], GAME_ID)?;
        let record = RecoveryIdentity::try_deserialize(&mut accounts[7].data.as_slice())?;
        assert_eq!(record.status, RecoveryStatus::Closed);
        let before = accounts.clone();
        let data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
        assert_eq!(
            dispatch(&mut accounts, &data)?,
            Err(code(GameErrorCode::RecoveryIdentityMismatch))
        );
        assert_eq!(accounts, before);
        Ok(())
    })
}

#[test]
fn recovery_sweeps_sub_rent_excess_to_treasury_instead_of_failing() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let mut accounts = recovery_v2_accounts(100_000_000)?;
        accounts[1].lamports = 200_000_001;
        let (white, black, treasury) = (
            accounts[3].lamports,
            accounts[4].lamports,
            accounts[2].lamports,
        );
        let game_rent = accounts[0].lamports;
        let total_before: u64 = accounts.iter().map(|a| a.lamports).sum();
        let data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
        dispatch(&mut accounts, &data)??;
        assert_eq!(
            accounts[1].lamports, 0,
            "escrow ends empty, not rent-paying"
        );
        assert_eq!(accounts[3].lamports, white + 100_000_000);
        assert_eq!(accounts[4].lamports, black + 100_000_000);
        assert_eq!(accounts[2].lamports, treasury + 1 + game_rent);
        assert_eq!(
            accounts.iter().map(|a| a.lamports).sum::<u64>(),
            total_before,
            "lamports are conserved"
        );
        Ok(())
    })
}

#[test]
fn global_join_v2_binds_authenticated_black_and_preserves_white() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let key = Pubkey::new_unique();
        let session = session_accounts(black, key, NOW + 100, 2, true)?;
        let mut signer = Fixture::system(key);
        signer.signer = true;
        let mut game = waiting_game(white, NOW);
        game.wager_amount = 10_000_000;
        game.match_type = MatchType::Rated;
        let (record_key, bump) = Pubkey::find_program_address(
            &[RecoveryIdentity::SEED, &GAME_ID.to_le_bytes()],
            &xfchess_game::ID,
        );
        let record =
            RecoveryIdentity::new(GAME_ID, white, Pubkey::default(), game.wager_amount, bump)?;
        let escrow_key = Pubkey::find_program_address(
            &[WAGER_ESCROW_SEED, &GAME_ID.to_le_bytes()],
            &xfchess_game::ID,
        )
        .0;
        let mut accounts = vec![
            session[0].clone(),
            signer,
            Fixture::system(black),
            Fixture::program(game_key(), &game, 8 + Game::INIT_SPACE)?,
            profile_accounts(black)?[0].clone(),
            profile_accounts(white)?[0].clone(),
            Fixture::system(escrow_key),
            system_program(),
            Fixture::program(record_key, &record, 8 + RecoveryIdentity::INIT_SPACE)?,
        ];
        dispatch(
            &mut accounts,
            &xfchess_game::instruction::GlobalJoinGameV2 { game_id: GAME_ID }.data(),
        )??;
        let record = RecoveryIdentity::try_deserialize(&mut accounts[8].data.as_slice())?;
        assert_eq!(
            (record.white, record.black, record.status),
            (white, black, RecoveryStatus::Ready)
        );
        let game = Game::try_deserialize(&mut accounts[3].data.as_slice())?;
        assert_eq!(
            (game.white, game.black, game.status),
            (white, black, GameStatus::Active)
        );
        let session = GlobalSessionDelegation::try_deserialize(&mut accounts[0].data.as_slice())?;
        assert_eq!(
            (session.total_spent, session.games_remaining),
            (100_000_000, 1)
        );
        assert_eq!(accounts[6].lamports, 1_010_000_000);
        Ok(())
    })
}

#[test]
fn legacy_lifecycle_cannot_omit_or_reuse_a_v2_identity() -> anyhow::Result<()> {
    assert!(xfchess_game::recovery_ix::require_legacy_identity_absent(&[], GAME_ID).is_err());
    let mut accounts = recovery_v2_accounts(0)?;
    let before = accounts[7].clone();
    assert!(xfchess_game::recovery_ix::require_legacy_identity_absent(
        &[accounts[7].info()],
        GAME_ID
    )
    .is_err());
    assert_eq!(accounts[7], before);
    let mut empty = Fixture::system(before.key);
    xfchess_game::recovery_ix::require_legacy_identity_absent(&[empty.info()], GAME_ID)?;
    empty.key = Pubkey::new_unique();
    assert!(
        xfchess_game::recovery_ix::require_legacy_identity_absent(&[empty.info()], GAME_ID)
            .is_err()
    );
    Ok(())
}

#[test]
fn global_session_withdraw_never_takes_rent_and_requires_the_owning_wallet() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let player = Pubkey::new_unique();
        let data = xfchess_game::instruction::WithdrawGlobalSession {}.data();
        let rent_min = {
            let mut accounts = session_accounts(player, Pubkey::new_unique(), NOW + 1, 1, true)?;
            Rent::default().minimum_balance(accounts.remove(0).data.len())
        };

        // A different wallet cannot reach the vault.
        let mut accounts = session_accounts(player, Pubkey::new_unique(), NOW + 1, 1, true)?;
        accounts.pop();
        accounts[1].key = Pubkey::new_unique();
        let before = accounts.clone();
        assert_eq!(dispatch(&mut accounts, &data)?, Err(seeds_error()));
        assert_eq!(accounts, before);

        // The owner recovers everything above rent, exactly once.
        let mut accounts = session_accounts(player, Pubkey::new_unique(), NOW + 1, 1, true)?;
        accounts.pop();
        let (vault_before, wallet_before) = (accounts[0].lamports, accounts[1].lamports);
        dispatch(&mut accounts, &data)??;
        assert_eq!(accounts[0].lamports, rent_min);
        assert_eq!(
            accounts[1].lamports,
            wallet_before + (vault_before - rent_min)
        );
        let after_first = accounts.clone();
        dispatch(&mut accounts, &data)??;
        assert_eq!(accounts, after_first, "second withdrawal moves nothing");
        Ok(())
    })
}

#[test]
fn recovery_only_runs_for_a_delegated_identity_and_never_twice() -> anyhow::Result<()> {
    with_sbf_runtime(|| {
        let data = xfchess_game::instruction::RecoverStuckDelegationV2 { game_id: GAME_ID }.data();
        for status in [
            RecoveryStatus::Waiting,
            RecoveryStatus::Ready,
            RecoveryStatus::Closed,
            RecoveryStatus::Recovered,
        ] {
            let mut accounts = recovery_v2_accounts(100_000_000)?;
            let record_index = accounts.len() - 1;
            let mut record =
                RecoveryIdentity::try_deserialize(&mut accounts[record_index].data.as_slice())?;
            record.status = status;
            let mut bytes = Vec::new();
            record.try_serialize(&mut bytes)?;
            bytes.resize(accounts[record_index].data.len(), 0);
            accounts[record_index].data = bytes;
            let before = accounts.clone();
            assert_eq!(
                dispatch(&mut accounts, &data)?,
                Err(code(GameErrorCode::RecoveryIdentityMismatch)),
                "{status:?}"
            );
            assert_eq!(accounts, before, "{status:?}: nothing may move");
        }
        Ok(())
    })
}
