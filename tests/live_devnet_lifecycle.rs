//! Opt-in live devnet cancellation drill using the client instruction builders.
//! Uses a throwaway wallet and returns remaining funds to XFCHESS_LIVE_FUNDER.
//! Run cargo test --features solana --test live_devnet_lifecycle -- --ignored --nocapture.
#![cfg(feature = "solana")]

use anyhow::Context;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use xfchess_app::solana::instructions::{cancel_game_ix, create_game_ix, PROGRAM_ID};

const RPC: &str = "https://api.devnet.solana.com";
const WAGER: u64 = 2_000_000; // 0.002 SOL (program minimum is 0.001)
const FUND: u64 = 12_000_000;

fn send(
    rpc: &RpcClient,
    ix: solana_sdk::instruction::Instruction,
    payer: &Keypair,
) -> anyhow::Result<String> {
    let bh = rpc.get_latest_blockhash().context("blockhash")?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], bh);
    Ok(rpc
        .send_and_confirm_transaction(&tx)
        .context("transaction confirmed")?
        .to_string())
}

#[test]
#[ignore = "live devnet: spends ~0.01 SOL; set XFCHESS_LIVE_FUNDER and pass --ignored"]
fn creator_cancels_unjoined_wagered_game_and_is_refunded() -> anyhow::Result<()> {
    let funder_path = std::env::var("XFCHESS_LIVE_FUNDER")
        .context("set XFCHESS_LIVE_FUNDER to a funded keypair file")?;
    let funder = read_keypair_file(&funder_path)
        .map_err(|error| anyhow::anyhow!("read funder keypair: {error}"))?;
    let rpc = RpcClient::new_with_commitment(RPC.to_string(), CommitmentConfig::confirmed());
    let program_id: Pubkey = PROGRAM_ID.parse().context(concat!(file!(), ":", line!()))?;

    let creator = Keypair::new();
    let fund_ix =
        solana_system_interface::instruction::transfer(&funder.pubkey(), &creator.pubkey(), FUND);
    send(&rpc, fund_ix, &funder)?;

    let game_id = 9_100_000_000
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context(concat!(file!(), ":", line!()))?
            .as_secs();
    let escrow = Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &program_id).0;
    let game_pda = Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

    let create = create_game_ix(
        program_id,
        creator.pubkey(),
        creator.pubkey(),
        game_id,
        WAGER,
        0, // MatchType::Free
        0,
        300,
        0,
    )
    .context(concat!(file!(), ":", line!()))?;
    let create_sig = send(&rpc, create, &creator)?;
    assert_eq!(
        rpc.get_balance(&escrow)
            .context(concat!(file!(), ":", line!()))?,
        WAGER,
        "stake escrowed"
    );
    let before_cancel =
        rpc.get_balance(&creator.pubkey())
            .context(concat!(file!(), ":", line!()))?;
    println!("game {game_id}: created {create_sig}");

    // The unjoined game’s black account is still Pubkey::default().
    let cancel = cancel_game_ix(
        program_id,
        creator.pubkey(),
        creator.pubkey(),
        Pubkey::default(),
        game_id,
    )
    .context(concat!(file!(), ":", line!()))?;
    let cancel_sig = send(&rpc, cancel, &creator)?;
    println!("game {game_id}: cancelled {cancel_sig}");

    let after_cancel =
        rpc.get_balance(&creator.pubkey())
            .context(concat!(file!(), ":", line!()))?;
    assert_eq!(
        rpc.get_balance(&escrow)
            .context(concat!(file!(), ":", line!()))?,
        0,
        "escrow emptied"
    );
    // Refund arrives in the same transaction; only the tx fee is spent.
    let fee = rpc
        .get_fee_for_message(
            &Transaction::new_with_payer(
                &[cancel_game_ix(
                    program_id,
                    creator.pubkey(),
                    creator.pubkey(),
                    Pubkey::default(),
                    game_id,
                )
                .context(concat!(file!(), ":", line!()))?],
                Some(&creator.pubkey()),
            )
            .message,
        )
        .context("fetch cancellation transaction fee")?;
    assert_eq!(
        after_cancel + fee,
        before_cancel + WAGER,
        "creator refunded the full stake"
    );

    let data = rpc.get_account_data(&game_pda).context("game account")?;
    let game =
        xfchess_app::multiplayer::solana::wager_recovery::decode_game(&data).context("decode")?;
    assert_eq!(game.status, xfchess_game::state::GameStatus::Cancelled);
    assert_eq!(game.result, xfchess_game::state::GameResult::None);

    let left = rpc
        .get_balance(&creator.pubkey())
        .context(concat!(file!(), ":", line!()))?;
    let back = left.saturating_sub(5_000);
    if back > 0 {
        let ret = solana_system_interface::instruction::transfer(
            &creator.pubkey(),
            &funder.pubkey(),
            back,
        );
        send(&rpc, ret, &creator)?;
    }
    Ok(())
}
