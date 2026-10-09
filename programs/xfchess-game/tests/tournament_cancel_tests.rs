mod common;

use anchor_lang::{AccountDeserialize, InstructionData, Space, ToAccountMetas};
use solana_program_test::{processor, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
};
use xfchess_game::{
    errors::GameErrorCode,
    state::{Tournament, TournamentPlayersShard, TournamentStatus},
    tournament_ix::lifecycle::initialize_escrow::TournamentEscrow,
};

const ID: u64 = 42;
const FEE: u64 = 2_000_000;
const GUARANTEE: u64 = 3_000_000;
const WALLET: u64 = 100_000_000;

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[anchor_lang::prelude::AccountInfo],
    data: &[u8],
) -> solana_program::entrypoint::ProgramResult {
    // Anchor ties the account slice and its inner references to one lifetime.
    // ProgramTest owns both for this synchronous call; no references escape.
    let accounts = unsafe { std::mem::transmute(accounts) };
    xfchess_game::entry(program_id, accounts, data)
}

fn pda(seed: &[u8]) -> Pubkey {
    Pubkey::find_program_address(&[seed, &ID.to_le_bytes()], &xfchess_game::ID).0
}

fn shard_pda(index: u8) -> Pubkey {
    Pubkey::find_program_address(
        &[b"tourney_players", &[index], &ID.to_le_bytes()],
        &xfchess_game::ID,
    )
    .0
}

struct Fixture {
    authority: Keypair,
    host: Keypair,
    players: [Pubkey; 2],
    tournament: Tournament,
    shards: [TournamentPlayersShard; 4],
    escrow_balance: u64,
}

impl Fixture {
    fn new(status: TournamentStatus, max_players: u16) -> Self {
        let authority = Keypair::new();
        let host = Keypair::new();
        let players = [Pubkey::new_unique(), Pubkey::new_unique()];
        // Zero bytes give valid empty optional fields and vectors for this fixture.
        let mut tournament =
            Tournament::try_deserialize_unchecked(&mut &vec![0u8; 8 + Tournament::INIT_SPACE][..])
                .unwrap();
        tournament.tournament_id = ID;
        tournament.authority = authority.pubkey();
        tournament.host_treasury = host.pubkey();
        tournament.bump =
            Pubkey::find_program_address(&[b"tournament", &ID.to_le_bytes()], &xfchess_game::ID).1;
        tournament.status = status;
        tournament.max_players = max_players;
        tournament.num_registered_players = 2;
        tournament.entry_fee = FEE;
        tournament.prize_pool = GUARANTEE;
        let shards = std::array::from_fn(|index| TournamentPlayersShard {
            tournament_id: ID,
            shard_id: index as u8,
            players: if index == 0 { players.to_vec() } else { vec![] },
            player_elos: if index == 0 { vec![1500; 2] } else { vec![] },
            swiss_standings: vec![],
        });
        Self {
            authority,
            host,
            players,
            tournament,
            shards,
            escrow_balance: Rent::default().minimum_balance(8)
                + GUARANTEE
                + if status == TournamentStatus::Registration {
                    2 * FEE
                } else {
                    0
                },
        }
    }

    async fn start(&self) -> ProgramTestContext {
        // Execute the current Rust entrypoint, never a potentially stale .so.
        let mut pt = ProgramTest::new(
            "xfchess_game",
            xfchess_game::ID,
            processor!(process_instruction),
        );
        pt.prefer_bpf(false);
        pt.add_account(
            pda(b"tournament"),
            common::program_account(&self.tournament, 8 + Tournament::INIT_SPACE),
        );
        let mut escrow = common::program_account(&TournamentEscrow {}, 8);
        escrow.lamports = self.escrow_balance;
        pt.add_account(pda(b"t_escrow"), escrow);
        for (index, shard) in self.shards.iter().enumerate() {
            pt.add_account(
                shard_pda(index as u8),
                common::program_account(shard, 8 + TournamentPlayersShard::space_for()),
            );
        }
        for key in [
            self.authority.pubkey(),
            self.host.pubkey(),
            self.players[0],
            self.players[1],
        ] {
            pt.add_account(key, common::system_account(WALLET));
        }
        // Cancellation with SOL only still validates the token program account.
        let mut token_program = common::system_account(1);
        token_program.executable = true;
        pt.add_account(anchor_spl::token::ID, token_program);
        pt.start_with_context().await
    }

    fn ix(&self, included: [bool; 3]) -> Instruction {
        let mut accounts = xfchess_game::__client_accounts_cancel_tournament::CancelTournament {
            tournament: pda(b"tournament"),
            tournament_players_shard_0: shard_pda(0),
            tournament_players_shard_1: included[0].then(|| shard_pda(1)),
            tournament_players_shard_2: included[1].then(|| shard_pda(2)),
            tournament_players_shard_3: included[2].then(|| shard_pda(3)),
            usdc_prize_escrow_authority: pda(b"t_usdc_prize"),
            usdc_prize_escrow: None,
            operator_usdc_ata: None,
            usdc_mint: None,
            escrow_pda: pda(b"t_escrow"),
            host_treasury: self.host.pubkey(),
            authority: self.authority.pubkey(),
            token_program: anchor_spl::token::ID,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None);
        if self.tournament.entry_fee > 0 {
            accounts.extend(self.players.map(|key| AccountMeta::new(key, false)));
        }
        Instruction {
            program_id: xfchess_game::ID,
            accounts,
            data: xfchess_game::instruction::CancelTournament { tournament_id: ID }.data(),
        }
    }

    async fn snapshot(&self, ctx: &mut ProgramTestContext) -> Vec<Account> {
        let mut accounts = vec![];
        for key in [
            pda(b"tournament"),
            pda(b"t_escrow"),
            self.host.pubkey(),
            self.authority.pubkey(),
            self.players[0],
            self.players[1],
            shard_pda(0),
            shard_pda(1),
            shard_pda(2),
            shard_pda(3),
        ] {
            accounts.push(ctx.banks_client.get_account(key).await.unwrap().unwrap());
        }
        accounts
    }

    async fn rejects(&self, ix: Instruction, expected: GameErrorCode) {
        let mut ctx = self.start().await;
        let before = self.snapshot(&mut ctx).await;
        let error = common::send(&mut ctx, ix, &[&self.authority, &self.host])
            .await
            .unwrap_err();
        assert_eq!(common::custom_code(&error), Some(common::ec(expected)));
        assert_eq!(
            self.snapshot(&mut ctx).await,
            before,
            "failed cancellation must be atomic"
        );
    }
}

#[tokio::test]
async fn requires_every_expected_shard_even_when_empty_or_free() {
    for status in [TournamentStatus::Registration, TournamentStatus::Active] {
        for (max, slots) in [
            (128, [false, false, false]),
            (128, [false, true, false]),
            (256, [true, false, true]),
            (256, [true, true, false]),
        ] {
            let mut f = Fixture::new(status, max);
            f.tournament.entry_fee = 0;
            f.rejects(f.ix(slots), GameErrorCode::InvalidTournamentStatus)
                .await;
        }
    }
}

#[tokio::test]
async fn rejects_inexact_registered_coverage_and_malformed_shards() {
    for count in [1, 3] {
        let mut f = Fixture::new(TournamentStatus::Registration, 128);
        f.tournament.num_registered_players = count;
        f.rejects(
            f.ix([true, false, false]),
            GameErrorCode::InvalidTournamentStatus,
        )
        .await;
    }
    let mut f = Fixture::new(TournamentStatus::Registration, 64);
    f.shards[0].player_elos.pop();
    f.rejects(f.ix([false; 3]), GameErrorCode::InvalidTournamentStatus)
        .await;
}

#[tokio::test]
async fn rejects_duplicate_players_across_shards() {
    let mut f = Fixture::new(TournamentStatus::Registration, 128);
    f.shards[0].players.pop();
    f.shards[0].player_elos.pop();
    f.shards[1].players.push(f.players[0]);
    f.shards[1].player_elos.push(1500);
    f.rejects(
        f.ix([true, false, false]),
        GameErrorCode::DuplicatePlayerAccount,
    )
    .await;
}

#[tokio::test]
async fn rejects_missing_extra_duplicate_wrong_and_readonly_recipients() {
    for case in 0..5 {
        let f = Fixture::new(TournamentStatus::Registration, 64);
        let mut ix = f.ix([false; 3]);
        let n = ix.accounts.len();
        match case {
            0 => {
                ix.accounts.pop();
            }
            1 => ix.accounts.push(AccountMeta::new(f.players[0], false)),
            2 => ix.accounts[n - 1].pubkey = f.players[0],
            3 => ix.accounts.swap(n - 1, n - 2),
            _ => ix.accounts[n - 1].is_writable = false,
        }
        f.rejects(ix, GameErrorCode::InvalidRemainingAccounts).await;
    }
}

#[tokio::test]
async fn refunds_cannot_spend_escrow_rent() {
    let mut f = Fixture::new(TournamentStatus::Registration, 64);
    f.tournament.prize_pool = 0;
    f.escrow_balance = Rent::default().minimum_balance(8) + 2 * FEE - 1;
    f.rejects(
        f.ix([false; 3]),
        GameErrorCode::InsufficientTreasuryForRefund,
    )
    .await;
}

#[tokio::test]
async fn guarantee_cannot_spend_rent_and_rolls_back_refunds() {
    let mut f = Fixture::new(TournamentStatus::Registration, 64);
    f.escrow_balance -= 1;
    f.rejects(f.ix([false; 3]), GameErrorCode::InsufficientFunds)
        .await;
}

#[tokio::test]
async fn registration_cancel_preserves_rent_and_cannot_repeat() {
    successful_cancel(TournamentStatus::Registration).await;
}

#[tokio::test]
#[ignore = "Anchor 1.1.2 SOL transfer CPI is unsupported by the native host processor"]
async fn active_cancel_refunds_host_and_cannot_repeat() {
    successful_cancel(TournamentStatus::Active).await;
}

async fn successful_cancel(status: TournamentStatus) {
    let mut f = Fixture::new(status, 256);
    f.shards[0].players.pop();
    f.shards[0].player_elos.pop();
    f.shards[1].players.push(f.players[1]);
    f.shards[1].player_elos.push(1500);
    let mut ctx = f.start().await;
    let ix = f.ix([true; 3]);
    common::send(&mut ctx, ix.clone(), &[&f.authority, &f.host])
        .await
        .unwrap();
    let after = f.snapshot(&mut ctx).await;
    let tournament = Tournament::try_deserialize(&mut &after[0].data[..]).unwrap();
    assert_eq!(tournament.status, TournamentStatus::Cancelled);
    assert_eq!(tournament.prize_pool, 0);
    assert!(!tournament.usdc_prize_funded);
    assert_eq!(after[1].lamports, Rent::default().minimum_balance(8));
    assert_eq!(
        after[2].lamports,
        WALLET + GUARANTEE
            - if status == TournamentStatus::Active {
                2 * FEE
            } else {
                0
            }
    );
    assert_eq!(after[4].lamports, WALLET + FEE);
    assert_eq!(after[5].lamports, WALLET + FEE);
    ctx.last_blockhash = ctx.get_new_latest_blockhash().await.unwrap();
    let error = common::send(&mut ctx, ix, &[&f.authority, &f.host])
        .await
        .unwrap_err();
    assert_eq!(
        common::custom_code(&error),
        Some(common::ec(GameErrorCode::TournamentNotActive))
    );
    assert_eq!(f.snapshot(&mut ctx).await, after);
}
