use xfchess_client_compat::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::chain::{
    apply_compute_budget, instructions as ix, with_retry, CONTRACT_ER_SESSION_REIMBURSEMENT,
};

const CREATE_GAME_COST: u64 = 5_000;
const JOIN_GAME_COST: u64 = 5_000;
const DELEGATE_COST: u64 = 5_000;
const UNDELEGATE_COST: u64 = 5_000;
const RECORD_RESULT_COST: u64 = 5_000;
const SETTLEMENT_TX_FEE: u64 = 10_000;

pub(super) fn verify_fees_advanced(fees_advanced: u64, move_count: u64) -> anyhow::Result<()> {
    let expected = CREATE_GAME_COST
        + JOIN_GAME_COST
        + DELEGATE_COST
        + move_count * RECORD_RESULT_COST
        + UNDELEGATE_COST
        + CONTRACT_ER_SESSION_REIMBURSEMENT;
    println!(
        "   fees_advanced: {} lamports (expected {} lamports for {} moves)",
        fees_advanced, expected, move_count
    );
    if fees_advanced != expected {
        anyhow::bail!(
            "fee-accounting mismatch: Game.fees_advanced was {fees_advanced} lamports, \
             expected {expected} lamports (create {CREATE_GAME_COST} + join {JOIN_GAME_COST} + \
             delegate {DELEGATE_COST} + {move_count}*record {RECORD_RESULT_COST} + undelegate \
             {UNDELEGATE_COST} + ER session fee {CONTRACT_ER_SESSION_REIMBURSEMENT}) — mark_undelegated's \
             accrual may have regressed"
        );
    }
    Ok(())
}

pub(super) async fn verify_treasury_reimbursement(
    base_rpc: &RpcClient,
    treasury_vault: Pubkey,
    treasury_before: u64,
    fees_advanced: u64,
    wager_lamports: u64,
) -> anyhow::Result<()> {
    let treasury_after = with_retry(|| base_rpc.get_balance(&treasury_vault)).await?;
    let treasury_delta = treasury_after.saturating_sub(treasury_before);

    let pot = wager_lamports.saturating_mul(2);
    let remaining = pot.saturating_sub(SETTLEMENT_TX_FEE);
    let expected_reimbursement = fees_advanced.min(remaining);

    println!(
        "   treasury_vault: {} -> {} SOL ({:+} lamports, expected {} lamports)",
        treasury_before as f64 / LAMPORTS_PER_SOL as f64,
        treasury_after as f64 / LAMPORTS_PER_SOL as f64,
        treasury_delta,
        expected_reimbursement
    );
    if treasury_delta != expected_reimbursement {
        anyhow::bail!(
            "treasury reimbursement mismatch: treasury_vault balance increased by \
             {treasury_delta} lamports, expected {expected_reimbursement} lamports \
             (fees_advanced {fees_advanced} lamports, clamped by pot-minus-tx-fee \
             {remaining} lamports) — settle_finished_game's platform_reimbursement \
             may have regressed"
        );
    }
    Ok(())
}

pub(super) const TOURNAMENT_PRIZE_LAMPORTS: u64 = 3_000_000;

pub(super) async fn verify_winner_payout(
    base_rpc: &RpcClient,
    white: &Pubkey,
    black: &Pubkey,
    white_before: u64,
    black_before: u64,
) -> anyhow::Result<()> {
    let white_after = with_retry(|| base_rpc.get_balance(white)).await?;
    let black_after = with_retry(|| base_rpc.get_balance(black)).await?;
    let white_delta = white_after as i64 - white_before as i64;
    let black_delta = black_after as i64 - black_before as i64;

    println!("   Escrow payout (winner = Black, delivered Fool's Mate):");
    println!(
        "     White: {:.6} -> {:.6} SOL ({:+.6})",
        white_before as f64 / LAMPORTS_PER_SOL as f64,
        white_after as f64 / LAMPORTS_PER_SOL as f64,
        white_delta as f64 / LAMPORTS_PER_SOL as f64,
    );
    println!(
        "     Black: {:.6} -> {:.6} SOL ({:+.6})",
        black_before as f64 / LAMPORTS_PER_SOL as f64,
        black_after as f64 / LAMPORTS_PER_SOL as f64,
        black_delta as f64 / LAMPORTS_PER_SOL as f64,
    );

    if black_delta <= 0 || black_delta <= white_delta {
        anyhow::bail!(
            "payout verification failed: winner (black) balance delta {black_delta} \
             lamports was not greater than loser (white) delta {white_delta} lamports \
             — smart contract did not pay the winning pair"
        );
    }

    println!(
        "   Payout verified: winner (black) received {:.6} SOL more than white from escrow.",
        (black_delta - white_delta) as f64 / LAMPORTS_PER_SOL as f64
    );
    Ok(())
}

pub(super) async fn verify_tournament_payout(
    logger: &mut crate::cost::measurements::MeasurementLog,
    base_rpc: &RpcClient,
    program_id: Pubkey,
    cranker: &Keypair,
    tournament_id: u64,
    places: &[Option<Pubkey>],
    prize_shares_bps: &[u16],
) -> anyhow::Result<()> {
    let placed: Vec<(Pubkey, u16)> = places
        .iter()
        .zip(prize_shares_bps.iter())
        .filter_map(|(p, &bps)| p.map(|pk| (pk, bps)))
        .collect();
    anyhow::ensure!(!placed.is_empty(), "no placed players to verify payout for");

    let mut balances_before = Vec::with_capacity(placed.len());
    for (pk, _) in &placed {
        balances_before.push(with_retry(|| base_rpc.get_balance(pk)).await?);
    }

    let winners: Vec<Pubkey> = placed.iter().map(|(pk, _)| *pk).collect();
    let mut dist_ixs = vec![ix::distribute_tournament_prizes_ix(
        program_id,
        cranker.pubkey(),
        tournament_id,
        &winners,
    )?];
    apply_compute_budget(&mut dist_ixs, 200_000, 10_000, 262_144);
    let blockhash = base_rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(
        &dist_ixs,
        Some(&cranker.pubkey()),
        &[cranker],
        blockhash,
    );
    let sig = with_retry(|| base_rpc.send_sdk_and_confirm_transaction(&tx)).await?;
    logger.log(
        crate::cost::measurements::ExecutionLayer::Solana,
        "tournament_payout",
        "distribute_tournament_prizes",
        true,
        Some(sig.to_string()),
    );

    println!("   Tournament payout ({} funded place(s)):", placed.len());
    for (i, (pk, bps)) in placed.iter().enumerate() {
        let place_no = i + 1;
        let expected = (TOURNAMENT_PRIZE_LAMPORTS as u128 * *bps as u128 / 10_000) as u64;
        let after = with_retry(|| base_rpc.get_balance(pk)).await?;
        let delta = after as i64 - balances_before[i] as i64;
        println!(
            "     Place {place_no}: {pk} — {:.6} SOL (expected {:.6})",
            delta as f64 / LAMPORTS_PER_SOL as f64,
            expected as f64 / LAMPORTS_PER_SOL as f64,
        );
        if delta != expected as i64 {
            anyhow::bail!(
                "payout verification failed: place {place_no} ({pk}) balance delta {delta} \
                 lamports != expected {expected} lamports — smart contract did not pay this \
                 place correctly"
            );
        }
    }
    println!(
        "   Payout verified: all {} funded place(s) received their exact prize share.",
        placed.len()
    );
    Ok(())
}
