//! Rating-from-profile registration and the optional tournament pass. The compiled game program
//! (`cargo build-sbf` first, or set `SBF_OUT_DIR`) runs against the real Token-2022 program; the
//! pass hook itself is not loaded here because the game only reads the mint's hook extension, the
//! pass program's config and eligibility accounts (installed below with their real byte layouts)
//! and burns (burning never invokes a hook).

mod common;

use anchor_lang::{
    solana_program::{program_option::COption, program_pack::Pack},
    AccountDeserialize, InstructionData, Space, ToAccountMetas,
};
use anchor_spl::token_2022::{
    spl_token_2022::{
        extension::{transfer_hook, BaseStateWithExtensions, ExtensionType, StateWithExtensions},
        instruction as token_instruction,
        state::{Account as TokenAccountState, Mint as MintState},
    },
    ID as TOKEN_2022,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_system_interface::instruction as system_instruction;
use xfchess_game::{
    constants::{PASS_REGISTRAR_SEED, XFCHESS_PASS_PROGRAM_ID},
    errors::GameErrorCode,
    state::{
        PlayerProfile, Tournament, TournamentPassBinding, TournamentPlayersShard,
        TournamentStatus,
    },
    tournament_ix::lifecycle::initialize_escrow::TournamentEscrow,
};

const ID: u64 = 7;
const BLITZ_BASE_SECONDS: u64 = 300;
const WALLET: u64 = 100_000_000;

fn pda(seed: &[u8]) -> Pubkey {
    Pubkey::find_program_address(&[seed, &ID.to_le_bytes()], &xfchess_game::ID).0
}

fn shard_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"tourney_players", &[0u8], &ID.to_le_bytes()], &xfchess_game::ID).0
}

fn binding_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"t_pass", &ID.to_le_bytes()], &xfchess_game::ID)
}

fn code(error: GameErrorCode) -> Option<u32> {
    Some(common::ec(error))
}

fn registrar_pda() -> Pubkey {
    Pubkey::find_program_address(&[PASS_REGISTRAR_SEED], &xfchess_game::ID).0
}

fn pass_config_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"pass-config", mint.as_ref()], &XFCHESS_PASS_PROGRAM_ID).0
}

fn pass_record_pda(mint: &Pubkey, player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"eligibility", mint.as_ref(), player.as_ref()],
        &XFCHESS_PASS_PROGRAM_ID,
    )
    .0
}

fn raw_account(data: Vec<u8>, owner: Pubkey) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

/// The fields of the pass program's `TournamentPassConfig` that the game reads, serialized in its
/// layout: discriminator, version, bumps, mint, tournament id, registrar, rating range, max age,
/// paused flag.
#[derive(Clone, Copy)]
struct PassConfig {
    mint: Pubkey,
    tournament_id: u64,
    registrar: Pubkey,
    min_elo: u32,
    max_elo: u32,
}

impl PassConfig {
    /// A config the game accepts for the fixture tournament (range 1400..=1600).
    fn valid(mint: Pubkey) -> Self {
        Self {
            mint,
            tournament_id: ID,
            registrar: registrar_pda(),
            min_elo: 1_400,
            max_elo: 1_600,
        }
    }

    fn account(&self, owner: Pubkey) -> Account {
        let mut data = vec![55, 121, 229, 33, 132, 84, 144, 61, 1, 255, 254];
        data.extend_from_slice(self.mint.as_ref());
        data.extend_from_slice(&self.tournament_id.to_le_bytes());
        data.extend_from_slice(self.registrar.as_ref());
        data.extend_from_slice(&self.min_elo.to_le_bytes());
        data.extend_from_slice(&self.max_elo.to_le_bytes());
        data.extend_from_slice(&100u64.to_le_bytes());
        data.push(0);
        raw_account(data, owner)
    }
}

/// The pass program's `EligibilityRecord` for `player`, as its `issue_eligibility` writes it.
fn pass_record_account(mint: &Pubkey, player: &Pubkey, is_revoked: bool, owner: Pubkey) -> Account {
    let mut data = vec![63, 254, 108, 207, 86, 176, 56, 170, 255];
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(player.as_ref());
    data.extend_from_slice(&1_500u32.to_le_bytes());
    data.extend_from_slice(&1u64.to_le_bytes());
    data.push(u8::from(is_revoked));
    raw_account(data, owner)
}

fn install_pass_config(ctx: &mut ProgramTestContext, config: &PassConfig) {
    ctx.set_account(
        &pass_config_pda(&config.mint),
        &config.account(XFCHESS_PASS_PROGRAM_ID).into(),
    );
}

fn install_pass_record(
    ctx: &mut ProgramTestContext,
    mint: &Pubkey,
    player: &Pubkey,
    is_revoked: bool,
) {
    ctx.set_account(
        &pass_record_pda(mint, player),
        &pass_record_account(mint, player, is_revoked, XFCHESS_PASS_PROGRAM_ID).into(),
    );
}

/// A player with a profile whose ratings are given as display Elo (stored x100).
struct Player {
    keypair: Keypair,
    blitz: u32,
    classical: u32,
}

impl Player {
    fn new(blitz: u32, classical: u32) -> Self {
        Self {
            keypair: Keypair::new(),
            blitz,
            classical,
        }
    }

    fn key(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    fn profile(&self) -> PlayerProfile {
        PlayerProfile {
            authority: self.key(),
            elo_blitz: f64::from(self.blitz) * 100.0,
            elo_rating: f64::from(self.classical) * 100.0,
            ..Default::default()
        }
    }
}

struct Fixture {
    authority: Keypair,
    host: Pubkey,
    players: Vec<Player>,
    tournament: Tournament,
    /// An initialized mint owned by the classic token program.
    legacy_mint: Pubkey,
}

impl Fixture {
    fn new(players: Vec<Player>) -> Self {
        let authority = Keypair::new();
        let host = Pubkey::new_unique();
        // Zero bytes give valid empty optional fields and vectors for this fixture.
        let mut tournament =
            Tournament::try_deserialize_unchecked(&mut &vec![0u8; 8 + Tournament::INIT_SPACE][..])
                .expect("zeroed tournament");
        tournament.tournament_id = ID;
        tournament.authority = authority.pubkey();
        tournament.host_treasury = host;
        tournament.bump =
            Pubkey::find_program_address(&[b"tournament", &ID.to_le_bytes()], &xfchess_game::ID).1;
        tournament.status = TournamentStatus::Registration;
        tournament.max_players = 8;
        tournament.elo_min = 1_400;
        tournament.elo_max = 1_600;
        tournament.base_time_seconds = BLITZ_BASE_SECONDS;
        Self {
            authority,
            host,
            players,
            tournament,
            legacy_mint: Pubkey::new_unique(),
        }
    }

    async fn start(&self) -> ProgramTestContext {
        // Account creation and the burn are CPIs, which only work in the compiled program.
        if std::env::var_os("SBF_OUT_DIR").is_none() {
            std::env::set_var(
                "SBF_OUT_DIR",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy"),
            );
        }
        let mut pt = ProgramTest::new("xfchess_game", xfchess_game::ID, None);
        pt.add_account(
            pda(b"tournament"),
            common::program_account_value(&self.tournament, 8 + Tournament::INIT_SPACE),
        );
        let mut escrow = common::program_account_value(&TournamentEscrow {}, 8);
        escrow.lamports = Rent::default().minimum_balance(8);
        pt.add_account(pda(b"t_escrow"), escrow);
        pt.add_account(
            shard_pda(),
            common::program_account_value(
                &TournamentPlayersShard {
                    tournament_id: ID,
                    shard_id: 0,
                    players: vec![],
                    player_elos: vec![],
                    swiss_standings: vec![],
                },
                8 + TournamentPlayersShard::space_for(),
            ),
        );
        for key in [self.authority.pubkey(), self.host] {
            pt.add_account(key, common::system_account(WALLET));
        }
        for player in &self.players {
            pt.add_account(player.key(), common::system_account(WALLET));
            pt.add_account(
                common::profile_pda(&player.key()).0,
                common::program_account_value(&player.profile(), 8 + PlayerProfile::INIT_SPACE),
            );
        }
        let mut legacy_mint = vec![0u8; MintState::LEN];
        MintState::pack(
            MintState {
                mint_authority: COption::Some(self.authority.pubkey()),
                supply: 0,
                decimals: 0,
                is_initialized: true,
                freeze_authority: COption::None,
            },
            &mut legacy_mint,
        )
        .expect("pack the legacy mint");
        pt.add_account(
            self.legacy_mint,
            Account {
                lamports: Rent::default().minimum_balance(MintState::LEN),
                data: legacy_mint,
                owner: anchor_spl::token::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        pt.start_with_context().await
    }

    fn bind_ix(&self, signer: &Pubkey, mint: &Pubkey) -> Instruction {
        let accounts = xfchess_game::__client_accounts_bind_tournament_pass::BindTournamentPass {
            tournament: pda(b"tournament"),
            pass_binding: binding_pda().0,
            pass_mint: *mint,
            pass_config: pass_config_pda(mint),
            registrar: registrar_pda(),
            authority: *signer,
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None);
        Instruction {
            program_id: xfchess_game::ID,
            accounts,
            data: xfchess_game::instruction::BindTournamentPass { tournament_id: ID }.data(),
        }
    }

    /// `pass` is the pass mint and the token account to redeem; `None` supplies neither. A pass
    /// also supplies the player's eligibility record address as the first remaining account.
    fn register_ix(&self, player: &Pubkey, pass: Option<(Pubkey, Pubkey)>) -> Instruction {
        let mut accounts = xfchess_game::__client_accounts_register_player::RegisterPlayer {
            tournament: pda(b"tournament"),
            player_profile: common::profile_pda(player).0,
            player: *player,
            escrow_pda: pda(b"t_escrow"),
            tournament_players_shard_0: shard_pda(),
            tournament_players_shard_1: None,
            tournament_players_shard_2: None,
            tournament_players_shard_3: None,
            host_treasury: self.host,
            system_program: solana_system_interface::program::ID,
            pass_binding: binding_pda().0,
            pass_mint: pass.map(|(mint, _)| mint),
            player_pass_account: pass.map(|(_, token_account)| token_account),
            token_program_2022: pass.map(|_| TOKEN_2022),
        }
        .to_account_metas(None);
        if let Some((mint, _)) = pass {
            accounts.push(solana_sdk::instruction::AccountMeta::new_readonly(
                pass_record_pda(&mint, player),
                false,
            ));
        }
        Instruction {
            program_id: xfchess_game::ID,
            accounts,
            data: xfchess_game::instruction::RegisterPlayer { tournament_id: ID }.data(),
        }
    }
}

async fn send_all(ctx: &mut ProgramTestContext, instructions: &[Instruction], extra: &[&Keypair]) {
    let mut signers: Vec<&Keypair> = vec![&ctx.payer];
    signers.extend_from_slice(extra);
    let blockhash = ctx.banks_client.get_latest_blockhash().await.expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&ctx.payer.pubkey()),
        &signers,
        blockhash,
    );
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .expect("setup transaction");
}

/// A Token-2022 mint owned by the payer. `hook` adds the TransferHook extension.
async fn create_mint(ctx: &mut ProgramTestContext, hook: Option<Pubkey>, decimals: u8) -> Pubkey {
    let mint = Keypair::new();
    let payer = ctx.payer.pubkey();
    let extensions: &[ExtensionType] = if hook.is_some() {
        &[ExtensionType::TransferHook]
    } else {
        &[]
    };
    let len = ExtensionType::try_calculate_account_len::<MintState>(extensions).expect("length");
    let rent = ctx.banks_client.get_rent().await.expect("rent").minimum_balance(len);
    let mut instructions = vec![system_instruction::create_account(
        &payer,
        &mint.pubkey(),
        rent,
        len as u64,
        &TOKEN_2022,
    )];
    if let Some(hook_program) = hook {
        instructions.push(
            transfer_hook::instruction::initialize(
                &TOKEN_2022,
                &mint.pubkey(),
                Some(payer),
                Some(hook_program),
            )
            .expect("hook extension"),
        );
    }
    instructions.push(
        token_instruction::initialize_mint2(&TOKEN_2022, &mint.pubkey(), &payer, None, decimals)
            .expect("initialize mint"),
    );
    send_all(ctx, &instructions, &[&mint]).await;
    mint.pubkey()
}

/// A token account of `mint` for `owner`, holding `amount`; `hooked` for hooked mints.
/// Revokes the payer's mint authority so the pass supply is fixed, as binding requires.
async fn revoke_mint_authority(ctx: &mut ProgramTestContext, mint: &Pubkey) {
    let payer = ctx.payer.pubkey();
    send_all(
        ctx,
        &[token_instruction::set_authority(
            &TOKEN_2022,
            mint,
            None,
            token_instruction::AuthorityType::MintTokens,
            &payer,
            &[],
        )
        .expect("set authority")],
        &[],
    )
    .await;
}

async fn create_token_account(
    ctx: &mut ProgramTestContext,
    mint: &Pubkey,
    owner: &Pubkey,
    hooked: bool,
    amount: u64,
) -> Pubkey {
    let token_account = Keypair::new();
    let payer = ctx.payer.pubkey();
    let extensions: &[ExtensionType] = if hooked {
        &[ExtensionType::TransferHookAccount]
    } else {
        &[]
    };
    let len =
        ExtensionType::try_calculate_account_len::<TokenAccountState>(extensions).expect("length");
    let rent = ctx.banks_client.get_rent().await.expect("rent").minimum_balance(len);
    let mut instructions = vec![
        system_instruction::create_account(
            &payer,
            &token_account.pubkey(),
            rent,
            len as u64,
            &TOKEN_2022,
        ),
        token_instruction::initialize_account3(&TOKEN_2022, &token_account.pubkey(), mint, owner)
            .expect("initialize account"),
    ];
    if amount > 0 {
        instructions.push(
            token_instruction::mint_to(&TOKEN_2022, mint, &token_account.pubkey(), &payer, &[], amount)
                .expect("mint to"),
        );
    }
    send_all(ctx, &instructions, &[&token_account]).await;
    token_account.pubkey()
}

async fn data_of(ctx: &mut ProgramTestContext, key: Pubkey) -> Vec<u8> {
    ctx.banks_client
        .get_account(key)
        .await
        .expect("account lookup")
        .expect("account exists")
        .data
}

async fn token_balance(ctx: &mut ProgramTestContext, token_account: Pubkey) -> u64 {
    let data = data_of(ctx, token_account).await;
    StateWithExtensions::<TokenAccountState>::unpack(&data)
        .expect("token account")
        .base
        .amount
}

async fn mint_supply(ctx: &mut ProgramTestContext, mint: Pubkey) -> u64 {
    let data = data_of(ctx, mint).await;
    StateWithExtensions::<MintState>::unpack(&data)
        .expect("mint")
        .base
        .supply
}

async fn shard(ctx: &mut ProgramTestContext) -> TournamentPlayersShard {
    let data = data_of(ctx, shard_pda()).await;
    TournamentPlayersShard::try_deserialize(&mut data.as_slice()).expect("shard")
}

async fn registration_error(
    ctx: &mut ProgramTestContext,
    instruction: Instruction,
    player: &Player,
) -> Option<u32> {
    let error: TransactionError = common::send(ctx, instruction, &[&player.keypair])
        .await
        .expect_err("registration must be refused");
    common::custom_code(&error)
}

#[tokio::test]
async fn the_rating_is_read_from_the_profile_bucket_of_the_time_control() {
    // Blitz tournament, range 1400..=1600: only the blitz rating counts.
    let fixture = Fixture::new(vec![
        Player::new(1_500, 2_500),
        Player::new(1_700, 1_500),
        Player::new(1_500, 1_500),
    ]);
    let mut ctx = fixture.start().await;

    common::send(
        &mut ctx,
        fixture.register_ix(&fixture.players[0].key(), None),
        &[&fixture.players[0].keypair],
    )
    .await
    .expect("a blitz rating in range registers");
    assert_eq!(shard(&mut ctx).await.player_elos, vec![1_500]);

    let high_blitz = &fixture.players[1];
    assert_eq!(
        registration_error(&mut ctx, fixture.register_ix(&high_blitz.key(), None), high_blitz)
            .await,
        code(GameErrorCode::EloOutOfRange),
        "a classical rating in range must not admit a blitz player rated too high"
    );
    assert_eq!(shard(&mut ctx).await.players, vec![fixture.players[0].key()]);
}

#[tokio::test]
async fn an_unrated_profile_counts_as_the_initial_rating() {
    let mut fixture = Fixture::new(vec![Player::new(0, 0)]);
    fixture.tournament.elo_min = 1_100;
    fixture.tournament.elo_max = 1_300;
    let mut ctx = fixture.start().await;
    common::send(
        &mut ctx,
        fixture.register_ix(&fixture.players[0].key(), None),
        &[&fixture.players[0].keypair],
    )
    .await
    .expect("1200 is in range");
    assert_eq!(shard(&mut ctx).await.player_elos, vec![1_200]);
}

#[tokio::test]
async fn the_organizer_binds_a_hooked_zero_decimal_token_2022_mint() {
    let fixture = Fixture::new(vec![]);
    let mut ctx = fixture.start().await;
    let mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    revoke_mint_authority(&mut ctx, &mint).await;

    common::send(
        &mut ctx,
        fixture.bind_ix(&fixture.authority.pubkey(), &mint),
        &[&fixture.authority],
    )
    .await
    .expect("bind");
    let data = data_of(&mut ctx, binding_pda().0).await;
    let binding = TournamentPassBinding::try_deserialize(&mut data.as_slice()).expect("binding");
    assert_eq!(
        (binding.tournament_id, binding.pass_mint, binding.bump),
        (ID, mint, binding_pda().1)
    );

    // An identical transaction on the same blockhash is deduplicated and reports success.
    ctx.last_blockhash = ctx.get_new_latest_blockhash().await.expect("blockhash");
    let second = common::send(
        &mut ctx,
        fixture.bind_ix(&fixture.authority.pubkey(), &mint),
        &[&fixture.authority],
    )
    .await
    .expect_err("a second binding");
    assert!(common::custom_code(&second).is_some(), "{second:?}");
}

#[tokio::test]
async fn binding_refuses_other_signers_and_mints_that_are_not_hooked_passes() {
    let fixture = Fixture::new(vec![]);
    let mut ctx = fixture.start().await;
    let stranger = Keypair::new();
    let good = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    ctx.set_account(
        &stranger.pubkey(),
        &common::system_account(WALLET).into(),
    );

    let error = common::send(
        &mut ctx,
        fixture.bind_ix(&stranger.pubkey(), &good),
        &[&stranger],
    )
    .await
    .expect_err("only the organizer binds");
    assert_eq!(
        common::custom_code(&error),
        code(GameErrorCode::NotTournamentAuthority)
    );

    let other_hook = create_mint(&mut ctx, Some(Pubkey::new_unique()), 0).await;
    let no_hook = create_mint(&mut ctx, None, 0).await;
    let divisible = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 6).await;
    for (label, mint) in [
        ("another hook program", other_hook),
        ("no hook extension", no_hook),
        ("non-zero decimals", divisible),
        ("a classic token program mint", fixture.legacy_mint),
    ] {
        let error = common::send(
            &mut ctx,
            fixture.bind_ix(&fixture.authority.pubkey(), &mint),
            &[&fixture.authority],
        )
        .await
        .expect_err(label);
        assert_eq!(
            common::custom_code(&error),
            code(GameErrorCode::InvalidPassMint),
            "{label}"
        );
    }
    assert!(ctx
        .banks_client
        .get_account(binding_pda().0)
        .await
        .expect("lookup")
        .is_none());
}

#[tokio::test]
async fn binding_needs_an_open_tournament_with_no_players_yet() {
    for (status, registered, expected) in [
        (
            TournamentStatus::Active,
            0,
            GameErrorCode::TournamentNotInRegistration,
        ),
        (
            TournamentStatus::Registration,
            1,
            GameErrorCode::InvalidTournamentStatus,
        ),
    ] {
        let mut fixture = Fixture::new(vec![]);
        fixture.tournament.status = status;
        fixture.tournament.num_registered_players = registered;
        let mut ctx = fixture.start().await;
        let mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
        let error = common::send(
            &mut ctx,
            fixture.bind_ix(&fixture.authority.pubkey(), &mint),
            &[&fixture.authority],
        )
        .await
        .expect_err("binding refused");
        assert_eq!(common::custom_code(&error), code(expected));
    }
}

/// A bound tournament with `players[0]` holding two passes and `players[1]` holding none.
struct Bound {
    fixture: Fixture,
    ctx: ProgramTestContext,
    mint: Pubkey,
    holder_pass: Pubkey,
    empty_pass: Pubkey,
}

async fn bound() -> Bound {
    let fixture = Fixture::new(vec![Player::new(1_500, 1_500), Player::new(1_500, 1_500)]);
    let mut ctx = fixture.start().await;
    let mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    let holder_pass = create_token_account(&mut ctx, &mint, &fixture.players[0].key(), true, 2).await;
    let empty_pass = create_token_account(&mut ctx, &mint, &fixture.players[1].key(), true, 0).await;
    revoke_mint_authority(&mut ctx, &mint).await;
    common::send(
        &mut ctx,
        fixture.bind_ix(&fixture.authority.pubkey(), &mint),
        &[&fixture.authority],
    )
    .await
    .expect("bind");
    Bound {
        fixture,
        ctx,
        mint,
        holder_pass,
        empty_pass,
    }
}

#[tokio::test]
async fn a_bound_tournament_refuses_registration_without_a_valid_pass() {
    let Bound {
        fixture,
        mut ctx,
        mint,
        holder_pass,
        empty_pass,
    } = bound().await;
    let (holder, empty) = (&fixture.players[0], &fixture.players[1]);
    let other_mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    let other_mint_pass = create_token_account(&mut ctx, &other_mint, &empty.key(), true, 1).await;

    let cases = [
        (
            "no pass accounts",
            holder,
            None,
            GameErrorCode::PassRequired,
        ),
        (
            "an empty pass account",
            empty,
            Some((mint, empty_pass)),
            GameErrorCode::PassNotHeld,
        ),
        (
            "someone else's pass account",
            empty,
            Some((mint, holder_pass)),
            GameErrorCode::PassNotHeld,
        ),
        (
            "a pass of another mint",
            empty,
            Some((other_mint, other_mint_pass)),
            GameErrorCode::PassMintMismatch,
        ),
    ];
    for (label, player, pass, expected) in cases {
        let instruction = fixture.register_ix(&player.key(), pass);
        assert_eq!(
            registration_error(&mut ctx, instruction, player).await,
            code(expected),
            "{label}"
        );
    }
    assert_eq!(token_balance(&mut ctx, holder_pass).await, 2);
    assert_eq!(mint_supply(&mut ctx, mint).await, 2);
    assert!(shard(&mut ctx).await.players.is_empty());
}

#[tokio::test]
async fn registering_burns_exactly_one_pass_and_cannot_be_repeated() {
    let Bound {
        fixture,
        mut ctx,
        mint,
        holder_pass,
        ..
    } = bound().await;
    let holder = &fixture.players[0];

    common::send(
        &mut ctx,
        fixture.register_ix(&holder.key(), Some((mint, holder_pass))),
        &[&holder.keypair],
    )
    .await
    .expect("a pass holder registers");
    assert_eq!(token_balance(&mut ctx, holder_pass).await, 1);
    assert_eq!(mint_supply(&mut ctx, mint).await, 1);
    let registered = shard(&mut ctx).await;
    assert_eq!(registered.players, vec![holder.key()]);
    assert_eq!(registered.player_elos, vec![1_500]);

    // The second pass is not a second seat.
    // An identical transaction on the same blockhash is deduplicated and reports success.
    ctx.last_blockhash = ctx.get_new_latest_blockhash().await.expect("blockhash");
    assert_eq!(
        registration_error(
            &mut ctx,
            fixture.register_ix(&holder.key(), Some((mint, holder_pass))),
            holder,
        )
        .await,
        code(GameErrorCode::AlreadyRegistered)
    );
    assert_eq!(token_balance(&mut ctx, holder_pass).await, 1);
    assert_eq!(mint_supply(&mut ctx, mint).await, 1);
}

#[tokio::test]
async fn a_pass_does_not_override_the_rating_range() {
    let mut fixture = Fixture::new(vec![Player::new(1_700, 1_700)]);
    fixture.tournament.elo_max = 1_600;
    let mut ctx = fixture.start().await;
    let mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    let pass = create_token_account(&mut ctx, &mint, &fixture.players[0].key(), true, 1).await;
    revoke_mint_authority(&mut ctx, &mint).await;
    common::send(
        &mut ctx,
        fixture.bind_ix(&fixture.authority.pubkey(), &mint),
        &[&fixture.authority],
    )
    .await
    .expect("bind");

    let player = &fixture.players[0];
    assert_eq!(
        registration_error(
            &mut ctx,
            fixture.register_ix(&player.key(), Some((mint, pass))),
            player,
        )
        .await,
        code(GameErrorCode::EloOutOfRange)
    );
    assert_eq!(token_balance(&mut ctx, pass).await, 1, "the pass is not spent on a refusal");
}

#[tokio::test]
async fn a_tournament_without_a_binding_ignores_supplied_pass_accounts() {
    let fixture = Fixture::new(vec![Player::new(1_500, 1_500)]);
    let mut ctx = fixture.start().await;
    let mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    let pass = create_token_account(&mut ctx, &mint, &fixture.players[0].key(), true, 1).await;
    let player = &fixture.players[0];
    common::send(
        &mut ctx,
        fixture.register_ix(&player.key(), Some((mint, pass))),
        &[&player.keypair],
    )
    .await
    .expect("no binding, no pass needed");
    assert_eq!(token_balance(&mut ctx, pass).await, 1, "nothing is burned without a binding");
}

#[tokio::test]
async fn binding_needs_a_fixed_supply_no_larger_than_the_player_limit() {
    let fixture = Fixture::new(vec![]);
    let mut ctx = fixture.start().await;
    let mint = create_mint(&mut ctx, Some(XFCHESS_PASS_PROGRAM_ID), 0).await;
    let organizer = fixture.authority.pubkey();

    let open = common::send(&mut ctx, fixture.bind_ix(&organizer, &mint), &[&fixture.authority])
        .await
        .expect_err("an organizer who can still mint");
    assert_eq!(
        common::custom_code(&open),
        code(GameErrorCode::PassMintAuthorityActive)
    );

    // The fixture allows 8 players; 9 passes would outnumber the seats.
    create_token_account(&mut ctx, &mint, &Pubkey::new_unique(), true, 9).await;
    revoke_mint_authority(&mut ctx, &mint).await;
    ctx.last_blockhash = ctx.get_new_latest_blockhash().await.expect("blockhash");
    let oversupplied =
        common::send(&mut ctx, fixture.bind_ix(&organizer, &mint), &[&fixture.authority])
            .await
            .expect_err("more passes than seats");
    assert_eq!(
        common::custom_code(&oversupplied),
        code(GameErrorCode::PassSupplyExceedsPlayers)
    );
    assert!(ctx
        .banks_client
        .get_account(binding_pda().0)
        .await
        .expect("lookup")
        .is_none());
}
