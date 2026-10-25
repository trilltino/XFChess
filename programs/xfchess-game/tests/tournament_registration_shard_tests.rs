//! Registration must see every player shard and fill the seats that exist.
//! Fixtures place players directly into shard accounts so tournaments larger
//! than one shard can be exercised without hundreds of registration calls.
#[path = "common/agave.rs"]
pub mod agave;
use agave::{ProgramTest, ProgramTestContext};
use anchor_lang::{prelude::Rent, AccountDeserialize, InstructionData, Space, ToAccountMetas};
use anyhow::Context as _;
pub mod common;

use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{PlayerProfile, Tournament, TournamentPlayersShard, TournamentStatus};
use xfchess_game::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;

const ID: u64 = 9_001;

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

fn binding_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"t_pass", &ID.to_le_bytes()], &xfchess_game::ID).0
}

struct Table {
    host: Pubkey,
    tournament: Tournament,
    shards: [TournamentPlayersShard; 4],
}

impl Table {
    /// `layout[i]` is how many players sit in shard `i`; `named` places
    /// specific players at the front of the given shard.
    fn new(
        max_players: u16,
        layout: [usize; 4],
        named: &[(usize, Pubkey)],
    ) -> anyhow::Result<Self> {
        let host = Pubkey::new_unique();
        let mut tournament =
            Tournament::try_deserialize_unchecked(&mut &vec![0u8; 8 + Tournament::INIT_SPACE][..])
                .context("zeroed tournament")?;
        tournament.tournament_id = ID;
        tournament.host_treasury = host;
        tournament.bump =
            Pubkey::find_program_address(&[b"tournament", &ID.to_le_bytes()], &xfchess_game::ID).1;
        tournament.status = TournamentStatus::Registration;
        tournament.max_players = max_players;
        tournament.elo_max = u32::MAX;
        tournament.num_registered_players = layout.iter().sum::<usize>() as u16;
        tournament.player_count = tournament.num_registered_players;
        let shards = std::array::from_fn(|index| {
            let mut players: Vec<Pubkey> = Vec::new();
            for (shard, key) in named {
                if *shard == index {
                    players.push(*key);
                }
            }
            while players.len() < layout[index] {
                players.push(Pubkey::new_unique());
            }
            TournamentPlayersShard {
                tournament_id: ID,
                shard_id: index as u8,
                player_elos: vec![1500; players.len()],
                players,
                swiss_standings: vec![],
            }
        });
        Ok(Self {
            host,
            tournament,
            shards,
        })
    }

    async fn start(&self, player: &Keypair) -> anyhow::Result<ProgramTestContext> {
        std::env::set_var(
            "SBF_OUT_DIR",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy"),
        );
        let mut pt = ProgramTest::new("xfchess_game", xfchess_game::ID, None);
        pt.add_account(
            pda(b"tournament"),
            common::program_account(&self.tournament, 8 + Tournament::INIT_SPACE)?,
        );
        let mut escrow = common::program_account(&TournamentEscrow {}, 8)?;
        escrow.lamports = Rent::default().minimum_balance(8);
        pt.add_account(pda(b"t_escrow"), escrow);
        for (index, shard) in self.shards.iter().enumerate() {
            pt.add_account(
                shard_pda(index as u8),
                common::program_account(shard, 8 + TournamentPlayersShard::space_for())?,
            );
        }
        let profile = PlayerProfile {
            authority: player.pubkey(),
            ..Default::default()
        };
        pt.add_account(
            common::profile_pda(&player.pubkey()).0,
            common::program_account(&profile, 8 + PlayerProfile::INIT_SPACE)?,
        );
        pt.add_account(player.pubkey(), common::system_account(1_000_000_000));
        pt.add_account(self.host, common::system_account(1_000_000));
        Ok(pt.start_with_context().await)
    }

    fn register_ix(&self, player: &Pubkey, supplied: [bool; 3]) -> Instruction {
        let accounts = xfchess_game::__client_accounts_register_player::RegisterPlayer {
            tournament: pda(b"tournament"),
            player_profile: common::profile_pda(player).0,
            player: *player,
            escrow_pda: pda(b"t_escrow"),
            tournament_players_shard_0: shard_pda(0),
            tournament_players_shard_1: supplied[0].then(|| shard_pda(1)),
            tournament_players_shard_2: supplied[1].then(|| shard_pda(2)),
            tournament_players_shard_3: supplied[2].then(|| shard_pda(3)),
            host_treasury: self.host,
            system_program: solana_system_interface::program::ID,
            pass_binding: binding_pda(),
            pass_mint: None,
            player_pass_account: None,
            token_program_2022: None,
        }
        .to_account_metas(None);
        Instruction {
            program_id: xfchess_game::ID,
            accounts,
            data: xfchess_game::instruction::RegisterPlayer { tournament_id: ID }.data(),
        }
    }
}

async fn shard_players(ctx: &mut ProgramTestContext, index: u8) -> anyhow::Result<Vec<Pubkey>> {
    let account = ctx
        .banks_client
        .get_account(shard_pda(index))
        .await?
        .context("shard missing")?;
    Ok(TournamentPlayersShard::try_deserialize(&mut account.data.as_slice())?.players)
}

#[tokio::test]
async fn a_registered_player_cannot_register_again_by_omitting_their_shard() -> anyhow::Result<()> {
    // 256-seat tournament: shard 0 and 1 full, shard 2 filling. The player
    // sits in shard 1 and supplies shards 0 and 2 only.
    let player = Keypair::new();
    let table = Table::new(256, [64, 64, 2, 0], &[(1, player.pubkey())])?;
    let mut ctx = table.start(&player).await?;
    let err = common::send(
        &mut ctx,
        table.register_ix(&player.pubkey(), [false, true, false]),
        &[&player],
    )
    .await
    .err()
    .context("registration with a required shard omitted must fail")?;
    assert_eq!(
        common::custom_code(&err),
        Some(common::ec(GameErrorCode::InvalidTournamentStatus))
    );
    let seats = shard_players(&mut ctx, 2).await?.len();
    assert_eq!(seats, 2, "no seat was taken");

    // With every required shard supplied the duplicate is recognised.
    let err = common::send(
        &mut ctx,
        table.register_ix(&player.pubkey(), [true, true, true]),
        &[&player],
    )
    .await
    .err()
    .context("duplicate registration must fail")?;
    assert_eq!(
        common::custom_code(&err),
        Some(common::ec(GameErrorCode::AlreadyRegistered))
    );
    Ok(())
}

#[tokio::test]
async fn registration_uses_a_shard_with_room_after_players_leave() -> anyhow::Result<()> {
    // 128 seats, shard 0 emptied by departures, shard 1 holding 64: the count
    // alone (64) points at the full shard 1 although 64 seats are free.
    let player = Keypair::new();
    let table = Table::new(128, [0, 64, 0, 0], &[])?;
    let mut ctx = table.start(&player).await?;
    common::send(
        &mut ctx,
        table.register_ix(&player.pubkey(), [true, false, false]),
        &[&player],
    )
    .await
    .context("a free seat exists in shard 0")?;
    assert_eq!(shard_players(&mut ctx, 0).await?, vec![player.pubkey()]);
    assert_eq!(shard_players(&mut ctx, 1).await?.len(), 64);
    Ok(())
}

#[tokio::test]
async fn a_full_tournament_still_refuses_registration() -> anyhow::Result<()> {
    let player = Keypair::new();
    let table = Table::new(128, [64, 64, 0, 0], &[])?;
    let mut ctx = table.start(&player).await?;
    let err = common::send(
        &mut ctx,
        table.register_ix(&player.pubkey(), [true, false, false]),
        &[&player],
    )
    .await
    .err()
    .context("a full tournament must refuse")?;
    assert_eq!(
        common::custom_code(&err),
        Some(common::ec(GameErrorCode::TournamentFull))
    );
    Ok(())
}
