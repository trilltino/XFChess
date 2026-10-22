//! Live Devnet run of the tournament pass flow against the deployed `xfchess_game` and
//! `xfchess_pass` programs. It lands real transactions, including refused ones:
//! - a pass moves only to players the game program has attested, and registering burns it;
//! - attestations expire and a refresh renews them;
//! - the organizer revokes a player, and a revoked record cannot be refreshed back;
//! - the bridge refuses ratings outside the range, the wrong mint and the wrong program, and the
//!   pass program refuses attestations from anyone but the game's registrar PDA;
//! - binding refuses a mint whose authority can still mint;
//! - an unverified profile cannot be attested, and the organizer can pause pass transfers.
//!
//! Needs funded Devnet keypairs, so it is ignored by default:
//! `VPS_KEYPAIR=<vps_authority.json> KYC_KEYPAIR=<kyc_authority.json> PAYER_KEYPAIR=<funded
//! throwaway.json> cargo test -p
//! xfchess-game --test tournament_pass_devnet -- --ignored --nocapture`

use std::{
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anchor_lang::{
    solana_program::instruction::AccountMeta, AccountDeserialize, InstructionData, ToAccountMetas,
};
use anchor_spl::token_2022::{
    spl_token_2022::{
        extension::{transfer_hook, ExtensionType, StateWithExtensions},
        instruction as token_instruction,
        state::{Account as TokenAccountState, Mint as MintState},
    },
    ID as TOKEN_2022,
};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcSendTransactionConfig;
use solana_sdk::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_system_interface::instruction as system_instruction;
use xfchess_game::{
    constants::{PASS_REGISTRAR_SEED, XFCHESS_PASS_PROGRAM_ID},
    errors::GameErrorCode,
    state::{Tournament, TournamentPlayersShard, TournamentType},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const DEVNET: &str = "https://api.devnet.solana.com";
const PASS_INITIALIZE: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const PASS_ISSUE: [u8; 8] = [29, 88, 236, 137, 74, 17, 193, 144];
const PASS_ERROR_PAUSED: u32 = 0xD003;
const PASS_ERROR_REVOKED: u32 = 0xD004;
const PASS_ERROR_EXPIRED: u32 = 0xD005;
const PASS_ERROR_NOT_ELIGIBLE: u32 = 0xD007;
const PASS_ERROR_UNAUTHORIZED_REGISTRAR: u32 = 0xD00B;
const ENTRY_FEE: u64 = 20_000_000;
const PRIZE: u64 = 50_000_000;
/// Short enough to watch an attestation expire during the run.
const MAX_AGE_SLOTS: u64 = 90;

fn game_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &xfchess_game::ID).0
}

fn pass_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &XFCHESS_PASS_PROGRAM_ID).0
}

fn record_pda(mint: &Pubkey, player: &Pubkey) -> Pubkey {
    pass_pda(&[b"eligibility", mint.as_ref(), player.as_ref()])
}

fn config_pda(mint: &Pubkey) -> Pubkey {
    pass_pda(&[b"pass-config", mint.as_ref()])
}

fn keypair_from_env(name: &str) -> TestResult<Keypair> {
    let path = std::env::var(name).map_err(|_| format!("set {name} to a keypair file"))?;
    read_keypair_file(&path).map_err(|error| format!("cannot read {name}: {error}").into())
}

fn signed(
    rpc: &RpcClient,
    instructions: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> TestResult<Transaction> {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    Ok(Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &signers,
        rpc.get_latest_blockhash()?,
    ))
}

fn send(
    rpc: &RpcClient,
    label: &str,
    instructions: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> TestResult {
    let transaction = signed(rpc, instructions, payer, extra)?;
    let signature = rpc
        .send_and_confirm_transaction(&transaction)
        .map_err(|error| format!("{label}: {error}"))?;
    println!("  ok      {label}: {signature}");
    Ok(())
}

/// Lands a transaction that must fail: it skips preflight so the failure is recorded on chain,
/// then returns the error from the confirmed status.
fn landed_failure(
    rpc: &RpcClient,
    label: &str,
    instructions: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> TestResult<TransactionError> {
    let transaction = signed(rpc, instructions, payer, extra)?;
    let signature = rpc.send_transaction_with_config(
        &transaction,
        RpcSendTransactionConfig {
            skip_preflight: true,
            ..RpcSendTransactionConfig::default()
        },
    )?;
    for _ in 0..60 {
        if let Some(status) = rpc.get_signature_statuses(&[signature])?.value.remove(0) {
            let error = status
                .err
                .ok_or_else(|| format!("{label}: expected a refusal, it succeeded"))?;
            println!("  refused {label}: {error:?} ({signature})");
            return Ok(error);
        }
        sleep(Duration::from_millis(500));
    }
    Err(format!("{label}: never confirmed").into())
}

fn custom(error: &TransactionError) -> Option<u32> {
    match error {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

fn game_code(error: GameErrorCode) -> Option<u32> {
    Some(6000 + error as u32)
}

fn account_data(rpc: &RpcClient, key: &Pubkey) -> TestResult<Vec<u8>> {
    Ok(rpc
        .get_account_with_commitment(key, CommitmentConfig::confirmed())?
        .value
        .ok_or("account missing")?
        .data)
}

fn token_balance(rpc: &RpcClient, token_account: &Pubkey) -> TestResult<u64> {
    let data = account_data(rpc, token_account)?;
    Ok(StateWithExtensions::<TokenAccountState>::unpack(&data)?
        .base
        .amount)
}

/// (verified_elo, verified_at_slot, is_revoked) of a pass `EligibilityRecord`.
fn record_fields(rpc: &RpcClient, mint: &Pubkey, player: &Pubkey) -> TestResult<(u32, u64, bool)> {
    let data = account_data(rpc, &record_pda(mint, player))?;
    assert_eq!(&data[9..41], mint.as_ref());
    assert_eq!(&data[41..73], player.as_ref());
    Ok((
        u32::from_le_bytes(data[73..77].try_into()?),
        u64::from_le_bytes(data[77..85].try_into()?),
        data[85] != 0,
    ))
}

fn pass_transfer_ix(
    mint: &Pubkey,
    source: &Pubkey,
    destination: &Pubkey,
    owner: &Pubkey,
    destination_owner: &Pubkey,
) -> TestResult<Instruction> {
    let mut instruction =
        token_instruction::transfer_checked(&TOKEN_2022, source, mint, destination, owner, &[], 1, 0)?;
    instruction.accounts.extend([
        AccountMeta::new_readonly(config_pda(mint), false),
        AccountMeta::new_readonly(record_pda(mint, destination_owner), false),
        AccountMeta::new_readonly(XFCHESS_PASS_PROGRAM_ID, false),
        AccountMeta::new_readonly(pass_pda(&[b"extra-account-metas", mint.as_ref()]), false),
    ]);
    Ok(instruction)
}

fn token_account_ixs(
    rpc: &RpcClient,
    payer: &Pubkey,
    account: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
) -> TestResult<Vec<Instruction>> {
    let len = ExtensionType::try_calculate_account_len::<TokenAccountState>(&[
        ExtensionType::TransferHookAccount,
        ExtensionType::ImmutableOwner,
    ])?;
    Ok(vec![
        system_instruction::create_account(
            payer,
            &account.pubkey(),
            rpc.get_minimum_balance_for_rent_exemption(len)?,
            len as u64,
            &TOKEN_2022,
        ),
        token_instruction::initialize_immutable_owner(&TOKEN_2022, &account.pubkey())?,
        token_instruction::initialize_account3(&TOKEN_2022, &account.pubkey(), mint, owner)?,
    ])
}

fn create_hooked_mint(rpc: &RpcClient, payer: &Keypair, label: &str) -> TestResult<Pubkey> {
    let mint = Keypair::new();
    let len = ExtensionType::try_calculate_account_len::<MintState>(&[ExtensionType::TransferHook])?;
    send(
        rpc,
        label,
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &mint.pubkey(),
                rpc.get_minimum_balance_for_rent_exemption(len)?,
                len as u64,
                &TOKEN_2022,
            ),
            transfer_hook::instruction::initialize(
                &TOKEN_2022,
                &mint.pubkey(),
                Some(payer.pubkey()),
                Some(XFCHESS_PASS_PROGRAM_ID),
            )?,
            token_instruction::initialize_mint2(&TOKEN_2022, &mint.pubkey(), &payer.pubkey(), None, 0)?,
        ],
        payer,
        &[&mint],
    )?;
    Ok(mint.pubkey())
}

struct Tournaments {
    id: u64,
    tournament: Pubkey,
    binding: Pubkey,
}

impl Tournaments {
    fn new(id: u64) -> Self {
        Self {
            id,
            tournament: game_pda(&[b"tournament", &id.to_le_bytes()]),
            binding: game_pda(&[b"t_pass", &id.to_le_bytes()]),
        }
    }

    fn init_ix(&self, vps: &Pubkey, elo: (u32, u32)) -> Instruction {
        Instruction {
            program_id: xfchess_game::ID,
            accounts: xfchess_game::__client_accounts_initialize_tournament::InitializeTournament {
                tournament: self.tournament,
                usdc_prize_escrow_authority: game_pda(&[b"t_usdc_prize", &self.id.to_le_bytes()]),
                usdc_prize_escrow: None,
                usdc_mint: None,
                authority: *vps,
                token_program: anchor_spl::token::ID,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: solana_system_interface::program::ID,
            }
            .to_account_metas(None),
            data: xfchess_game::instruction::InitializeTournament {
                tournament_id: self.id,
                name: "Pass bridge Devnet".to_string(),
                entry_fee: ENTRY_FEE,
                max_players: 4,
                tournament_type: TournamentType::SingleElimination,
                elo_min: elo.0,
                elo_max: elo.1,
                min_players: 2,
                prize_shares: [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0],
                platform_fee: 0,
                winner_takes_all: false,
                host_treasury: *vps,
                usdc_mint: None,
                base_time_seconds: 300,
                increment_seconds: 0,
            }
            .data(),
        }
    }

    fn bind_ix(&self, vps: &Pubkey, mint: &Pubkey) -> Instruction {
        Instruction {
            program_id: xfchess_game::ID,
            accounts: xfchess_game::__client_accounts_bind_tournament_pass::BindTournamentPass {
                tournament: self.tournament,
                pass_binding: self.binding,
                pass_mint: *mint,
                authority: *vps,
                system_program: solana_system_interface::program::ID,
            }
            .to_account_metas(None),
            data: xfchess_game::instruction::BindTournamentPass { tournament_id: self.id }.data(),
        }
    }

    fn bridge_ix(&self, mint: &Pubkey, player: &Pubkey, payer: &Pubkey) -> Instruction {
        self.bridge_with(mint, player, payer, XFCHESS_PASS_PROGRAM_ID)
    }

    fn bridge_with(
        &self,
        mint: &Pubkey,
        player: &Pubkey,
        payer: &Pubkey,
        pass_program: Pubkey,
    ) -> Instruction {
        Instruction {
            program_id: xfchess_game::ID,
            accounts: xfchess_game::__client_accounts_issue_pass_eligibility::IssuePassEligibility {
                tournament: self.tournament,
                pass_binding: self.binding,
                player: *player,
                player_profile: game_pda(&[b"profile", player.as_ref()]),
                pass_mint: *mint,
                pass_config: config_pda(mint),
                eligibility_record: record_pda(mint, player),
                registrar: game_pda(&[PASS_REGISTRAR_SEED]),
                payer: *payer,
                pass_program,
                system_program: solana_system_interface::program::ID,
            }
            .to_account_metas(None),
            data: xfchess_game::instruction::IssuePassEligibility { tournament_id: self.id }.data(),
        }
    }

    fn pause_ix(&self, mint: &Pubkey, authority: &Pubkey, is_paused: bool) -> Instruction {
        Instruction {
            program_id: xfchess_game::ID,
            accounts: xfchess_game::__client_accounts_set_pass_paused::SetPassPaused {
                tournament: self.tournament,
                pass_binding: self.binding,
                pass_mint: *mint,
                pass_config: config_pda(mint),
                registrar: game_pda(&[PASS_REGISTRAR_SEED]),
                authority: *authority,
                pass_program: XFCHESS_PASS_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: xfchess_game::instruction::SetPassPaused { tournament_id: self.id, is_paused }.data(),
        }
    }

    fn revoke_ix(&self, mint: &Pubkey, player: &Pubkey, authority: &Pubkey) -> Instruction {
        Instruction {
            program_id: xfchess_game::ID,
            accounts: xfchess_game::__client_accounts_revoke_pass_eligibility::RevokePassEligibility {
                tournament: self.tournament,
                pass_binding: self.binding,
                player: *player,
                pass_mint: *mint,
                pass_config: config_pda(mint),
                eligibility_record: record_pda(mint, player),
                registrar: game_pda(&[PASS_REGISTRAR_SEED]),
                authority: *authority,
                pass_program: XFCHESS_PASS_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: xfchess_game::instruction::RevokePassEligibility { tournament_id: self.id }.data(),
        }
    }
}

fn verify_profile_ix(player: &Pubkey, kyc: &Pubkey) -> Instruction {
    Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_verify_profile::VerifyProfile {
            player_profile: game_pda(&[b"profile", player.as_ref()]),
            admin: *kyc,
            player: *player,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::VerifyProfile {}.data(),
    }
}

fn init_profile_ix(player: &Pubkey, username: String) -> Instruction {
    Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_init_profile::InitProfile {
            player_profile: game_pda(&[b"profile", player.as_ref()]),
            username_record: game_pda(&[b"username", username.as_bytes()]),
            player: *player,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitProfile {
            username,
            country: "GB".to_string(),
            date_of_birth: 631_152_000,
        }
        .data(),
    }
}

/// Waits until the cluster is past the slot at which an attestation expires.
fn wait_for_expiry(rpc: &RpcClient, attested_at: u64) -> TestResult {
    while rpc.get_slot()? <= attested_at + MAX_AGE_SLOTS + 2 {
        sleep(Duration::from_secs(2));
    }
    Ok(())
}

#[test]
#[ignore = "spends Devnet SOL; needs VPS_KEYPAIR and PAYER_KEYPAIR"]
fn the_deployed_programs_attest_expire_revoke_and_burn_passes() -> TestResult {
    let rpc = RpcClient::new_with_commitment(DEVNET.to_string(), CommitmentConfig::confirmed());
    let vps = keypair_from_env("VPS_KEYPAIR")?;
    let payer = keypair_from_env("PAYER_KEYPAIR")?;
    let kyc = keypair_from_env("KYC_KEYPAIR")?;
    let (registered, expiring, revoked) = (Keypair::new(), Keypair::new(), Keypair::new());
    let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let main = Tournaments::new(id);
    let narrow = Tournaments::new(id + 1);
    let registrar = game_pda(&[PASS_REGISTRAR_SEED]);
    let escrow = game_pda(&[b"t_escrow", &id.to_le_bytes()]);
    let shard = game_pda(&[b"tourney_players", &[0u8], &id.to_le_bytes()]);
    println!("tournament {id}, game {}, pass program {XFCHESS_PASS_PROGRAM_ID}", xfchess_game::ID);
    for program in [xfchess_game::ID, XFCHESS_PASS_PROGRAM_ID] {
        assert!(rpc.get_account(&program)?.executable, "{program} is not deployed on Devnet");
    }

    // A blitz tournament for ratings 1000..=1400 (a new profile counts as 1200) and a second
    // one for 1300..=1400 that the same players are too low for.
    let escrow_ix = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_initialize_tournament_escrow::InitializeTournamentEscrow {
            tournament: main.tournament,
            escrow_pda: escrow,
            authority: vps.pubkey(),
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitializeTournamentEscrow { tournament_id: id }.data(),
    };
    let shards_ix = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_initialize_shards_small::InitializeShardsSmall {
            tournament: main.tournament,
            tournament_players_shard_0: shard,
            authority: vps.pubkey(),
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitializeShardsSmall { tournament_id: id }.data(),
    };
    send(
        &rpc,
        "initialize the tournament, escrow and shards",
        &[main.init_ix(&vps.pubkey(), (1_000, 1_400)), escrow_ix, shards_ix],
        &vps,
        &[],
    )?;
    send(
        &rpc,
        "initialize the narrow-range tournament",
        &[narrow.init_ix(&vps.pubkey(), (1_300, 1_400))],
        &vps,
        &[],
    )?;
    let fund_prize = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_fund_sol_prize::FundSolPrize {
            tournament: main.tournament,
            escrow_pda: escrow,
            operator: vps.pubkey(),
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::FundSolPrize { tournament_id: id, amount: PRIZE }.data(),
    };
    send(&rpc, "fund the guaranteed prize", &[fund_prize], &vps, &[])?;

    // Three players with profiles; only the first registers, so only it needs a funded wallet.
    send(
        &rpc,
        "fund the registering player",
        &[system_instruction::transfer(&payer.pubkey(), &registered.pubkey(), 100_000_000)],
        &payer,
        &[],
    )?;
    send(
        &rpc,
        "fund the other players",
        &[
            system_instruction::transfer(&payer.pubkey(), &expiring.pubkey(), 30_000_000),
            system_instruction::transfer(&payer.pubkey(), &revoked.pubkey(), 30_000_000),
        ],
        &payer,
        &[],
    )?;
    for (index, player) in [&registered, &expiring, &revoked].into_iter().enumerate() {
        send(
            &rpc,
            "create a player profile",
            &[init_profile_ix(&player.pubkey(), format!("pass{}x{index}", id % 10_000_000))],
            player,
            &[],
        )?;
    }

    // A pass mint hooked by the deployed pass program. The pass is initialized with the game's
    // registrar PDA only after the supply is fixed, which the pass program and binding both need.
    let mint = create_hooked_mint(&rpc, &payer, "create the hooked pass mint")?;
    let treasury = Keypair::new();
    let mut ixs = token_account_ixs(&rpc, &payer.pubkey(), &treasury, &mint, &payer.pubkey())?;
    ixs.push(token_instruction::mint_to(&TOKEN_2022, &mint, &treasury.pubkey(), &payer.pubkey(), &[], 3)?);
    send(&rpc, "mint three passes to the organizer", &ixs, &payer, &[&treasury])?;

    let error = landed_failure(
        &rpc,
        "bind while the organizer can still mint",
        &[main.bind_ix(&vps.pubkey(), &mint)],
        &vps,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::PassMintAuthorityActive));
    send(
        &rpc,
        "revoke the mint authority",
        &[token_instruction::set_authority(
            &TOKEN_2022,
            &mint,
            None,
            token_instruction::AuthorityType::MintTokens,
            &payer.pubkey(),
            &[],
        )?],
        &payer,
        &[],
    )?;
    let mut data = PASS_INITIALIZE.to_vec();
    data.extend_from_slice(&id.to_le_bytes());
    data.extend_from_slice(registrar.as_ref());
    data.extend_from_slice(&1_000u32.to_le_bytes());
    data.extend_from_slice(&1_400u32.to_le_bytes());
    data.extend_from_slice(&MAX_AGE_SLOTS.to_le_bytes());
    send(
        &rpc,
        "initialize the pass for the tournament",
        &[Instruction {
            program_id: XFCHESS_PASS_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(config_pda(&mint), false),
                AccountMeta::new(pass_pda(&[b"extra-account-metas", mint.as_ref()]), false),
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_system_interface::program::ID, false),
            ],
            data,
        }],
        &payer,
        &[],
    )?;
    send(&rpc, "bind the pass to the tournament", &[main.bind_ix(&vps.pubkey(), &mint)], &vps, &[])?;
    send(&rpc, "bind the same pass to the narrow tournament", &[narrow.bind_ix(&vps.pubkey(), &mint)], &vps, &[])?;

    let accounts: Vec<Keypair> = (0..3).map(|_| Keypair::new()).collect();
    let mut ixs = vec![];
    for (account, owner) in accounts.iter().zip([&registered, &expiring, &revoked]) {
        ixs.extend(token_account_ixs(&rpc, &payer.pubkey(), account, &mint, &owner.pubkey())?);
    }
    send(&rpc, "create the players' pass accounts", &ixs, &payer, &accounts.iter().collect::<Vec<_>>())?;
    let transfer = |to: usize, owner: &Keypair| {
        pass_transfer_ix(&mint, &treasury.pubkey(), &accounts[to].pubkey(), &payer.pubkey(), &owner.pubkey())
    };

    // An unverified profile cannot be attested; the KYC authority then verifies all three players.
    let error = landed_failure(
        &rpc,
        "attest a profile that is not KYC-verified",
        &[main.bridge_ix(&mint, &registered.pubkey(), &payer.pubkey())],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::ProfileNotVerified));
    let verifications: Vec<Instruction> = [&registered, &expiring, &revoked]
        .iter()
        .map(|player| verify_profile_ix(&player.pubkey(), &kyc.pubkey()))
        .collect();
    send(&rpc, "the KYC authority verifies the three profiles", &verifications, &payer, &[&kyc])?;

    // Refusals: the hook before attestation, and the bridge and pass program on bad input.
    let error = landed_failure(&rpc, "pass transfer before attestation", &[transfer(0, &registered)?], &payer, &[])?;
    assert_eq!(custom(&error), Some(PASS_ERROR_NOT_ELIGIBLE));
    let error = landed_failure(
        &rpc,
        "attest a rating below the tournament range",
        &[narrow.bridge_ix(&mint, &registered.pubkey(), &payer.pubkey())],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::EloOutOfRange));
    let other_mint = create_hooked_mint(&rpc, &payer, "create a second hooked mint")?;
    let error = landed_failure(
        &rpc,
        "attest through a mint that is not the tournament's pass",
        &[main.bridge_ix(&other_mint, &registered.pubkey(), &payer.pubkey())],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::PassMintMismatch));
    let error = landed_failure(
        &rpc,
        "attest through a program that is not the pass program",
        &[main.bridge_with(&mint, &registered.pubkey(), &payer.pubkey(), solana_system_interface::program::ID)],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::InvalidAccountOwner));
    let mut forged = PASS_ISSUE.to_vec();
    forged.extend_from_slice(&1_200u32.to_le_bytes());
    let stranger = Keypair::new().pubkey();
    let error = landed_failure(
        &rpc,
        "attest directly to the pass program without the registrar",
        &[Instruction {
            program_id: XFCHESS_PASS_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(config_pda(&mint), false),
                AccountMeta::new(record_pda(&mint, &stranger), false),
                AccountMeta::new_readonly(stranger, false),
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(solana_system_interface::program::ID, false),
            ],
            data: forged,
        }],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), Some(PASS_ERROR_UNAUTHORIZED_REGISTRAR));

    // Player 1: attested, receives the pass, registers by burning it.
    send(&rpc, "attest player 1", &[main.bridge_ix(&mint, &registered.pubkey(), &payer.pubkey())], &payer, &[])?;
    assert_eq!(record_fields(&rpc, &mint, &registered.pubkey())?.0, 1_200, "the profile's rating");
    send(&rpc, "transfer a pass to player 1", &[transfer(0, &registered)?], &payer, &[])?;
    assert_eq!(token_balance(&rpc, &accounts[0].pubkey())?, 1);
    let register = Instruction {
        program_id: xfchess_game::ID,
        accounts: xfchess_game::__client_accounts_register_player::RegisterPlayer {
            tournament: main.tournament,
            player_profile: game_pda(&[b"profile", registered.pubkey().as_ref()]),
            player: registered.pubkey(),
            escrow_pda: escrow,
            tournament_players_shard_0: shard,
            tournament_players_shard_1: None,
            tournament_players_shard_2: None,
            tournament_players_shard_3: None,
            host_treasury: vps.pubkey(),
            system_program: solana_system_interface::program::ID,
            pass_binding: main.binding,
            pass_mint: Some(mint),
            player_pass_account: Some(accounts[0].pubkey()),
            token_program_2022: Some(TOKEN_2022),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::RegisterPlayer { tournament_id: id }.data(),
    };
    send(&rpc, "player 1 registers, burning the pass", &[register], &registered, &[])?;
    assert_eq!(token_balance(&rpc, &accounts[0].pubkey())?, 0, "the pass is burned");
    let shard_state = TournamentPlayersShard::try_deserialize(&mut account_data(&rpc, &shard)?.as_slice())?;
    assert_eq!(shard_state.players, vec![registered.pubkey()]);
    assert_eq!(shard_state.player_elos, vec![1_200]);
    let state = Tournament::try_deserialize(&mut account_data(&rpc, &main.tournament)?.as_slice())?;
    assert_eq!(state.num_registered_players, 1);

    // Player 2: the attestation expires, the transfer is refused, a refresh renews it.
    send(&rpc, "attest player 2", &[main.bridge_ix(&mint, &expiring.pubkey(), &payer.pubkey())], &payer, &[])?;
    let (_, attested_at, _) = record_fields(&rpc, &mint, &expiring.pubkey())?;
    wait_for_expiry(&rpc, attested_at)?;
    let error = landed_failure(&rpc, "pass transfer after the attestation expired", &[transfer(1, &expiring)?], &payer, &[])?;
    assert_eq!(custom(&error), Some(PASS_ERROR_EXPIRED));
    send(&rpc, "refresh player 2's attestation", &[main.bridge_ix(&mint, &expiring.pubkey(), &payer.pubkey())], &payer, &[])?;
    assert!(record_fields(&rpc, &mint, &expiring.pubkey())?.1 > attested_at);
    send(&rpc, "the organizer pauses pass transfers", &[main.pause_ix(&mint, &vps.pubkey(), true)], &vps, &[])?;
    let error = landed_failure(&rpc, "pass transfer while paused", &[transfer(1, &expiring)?], &payer, &[])?;
    assert_eq!(custom(&error), Some(PASS_ERROR_PAUSED));
    send(&rpc, "the organizer resumes pass transfers", &[main.pause_ix(&mint, &vps.pubkey(), false)], &vps, &[])?;
    send(&rpc, "transfer a pass to player 2 after the refresh", &[transfer(1, &expiring)?], &payer, &[])?;
    assert_eq!(token_balance(&rpc, &accounts[1].pubkey())?, 1);

    // Player 3: revoked by the organizer, cannot be refreshed back or receive the pass.
    send(&rpc, "attest player 3", &[main.bridge_ix(&mint, &revoked.pubkey(), &payer.pubkey())], &payer, &[])?;
    let error = landed_failure(
        &rpc,
        "revoke as someone other than the organizer",
        &[main.revoke_ix(&mint, &revoked.pubkey(), &payer.pubkey())],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::NotTournamentAuthority));
    send(&rpc, "the organizer revokes player 3", &[main.revoke_ix(&mint, &revoked.pubkey(), &vps.pubkey())], &vps, &[])?;
    assert!(record_fields(&rpc, &mint, &revoked.pubkey())?.2);
    let error = landed_failure(
        &rpc,
        "refresh a revoked record",
        &[main.bridge_ix(&mint, &revoked.pubkey(), &payer.pubkey())],
        &payer,
        &[],
    )?;
    assert_eq!(custom(&error), game_code(GameErrorCode::PassEligibilityRevoked));
    let error = landed_failure(&rpc, "pass transfer to the revoked player", &[transfer(2, &revoked)?], &payer, &[])?;
    assert_eq!(custom(&error), Some(PASS_ERROR_REVOKED));
    assert_eq!(token_balance(&rpc, &accounts[2].pubkey())?, 0);

    println!("\nmint {mint}\ntournament {id} (narrow range {})", narrow.id);
    Ok(())
}
