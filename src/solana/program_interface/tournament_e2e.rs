//! End-to-end tournament driver for devnet.
//!
//! Runs a complete single-elimination tournament against the deployed
//! `xfchess-game` program: initialize → escrow → shards → prize funding →
//! player profiles → registration → start → bracket matches → results →
//! winner advancement, using ephemeral player keypairs funded from the
//! admin wallet. Match results are recorded by the tournament authority
//! (higher seed wins), mirroring how the backend records results — the
//! goal is to exercise the tournament instruction surface, not chess.
//!
//! Used by the `tournament_data_gen` and `tournament_real_test` bins.

use anyhow::{bail, Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

use super::instructions::{
    advance_winner_ix, bracket_position, fund_sol_prize_ix, init_profile_ix, initialize_escrow_ix,
    initialize_match_ix, initialize_shards_ix, initialize_tournament_ix, record_match_result_ix,
    register_player_ix, start_tournament_ix, PROGRAM_ID,
};

/// One confirmed on-chain step of the run.
#[derive(Debug, Clone)]
pub struct StepLog {
    pub step: String,
    pub signature: String,
}

/// Outcome of a full tournament run.
#[derive(Debug)]
pub struct TournamentRunSummary {
    pub tournament_id: u64,
    pub player_count: u16,
    pub champion: Pubkey,
    pub players: Vec<(String, Pubkey)>,
    pub steps: Vec<StepLog>,
}

/// Lamports funded to each ephemeral player: profile + username rent,
/// entry fee, and transaction fees.
const PLAYER_FUNDING_LAMPORTS: u64 = 20_000_000; // 0.02 SOL
/// Entry fee per player (operator revenue, refundable until start).
const ENTRY_FEE_LAMPORTS: u64 = 1_000_000; // 0.001 SOL
/// Guaranteed SOL prize locked in escrow before registration opens.
const PRIZE_LAMPORTS: u64 = 5_000_000; // 0.005 SOL

/// Runs a complete `player_count`-player tournament on `rpc_url`.
///
/// `admin` must be the program's `vps_authority` (tournament authority);
/// it also acts as `host_treasury`. Supports 2..=64 players (power of 2).
pub fn run_tournament(
    rpc_url: &str,
    admin: &Keypair,
    player_count: u16,
) -> Result<TournamentRunSummary> {
    if !player_count.is_power_of_two() || !(2..=64).contains(&player_count) {
        bail!("player_count must be a power of 2 between 2 and 64, got {player_count}");
    }

    let program_id: Pubkey = PROGRAM_ID.parse().context("bad PROGRAM_ID")?;
    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let mut steps: Vec<StepLog> = Vec::new();

    let admin_balance = rpc.get_balance(&admin.pubkey())?;
    let needed = PLAYER_FUNDING_LAMPORTS * player_count as u64 + PRIZE_LAMPORTS + 20_000_000;
    if admin_balance < needed {
        bail!(
            "admin {} has {} lamports, needs at least {} — fund it with devnet SOL",
            admin.pubkey(),
            admin_balance,
            needed
        );
    }

    let tournament_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    println!(
        "Tournament {tournament_id}: {player_count} players, program {program_id}, admin {}",
        admin.pubkey()
    );

    // ── Players: ephemeral keypairs, highest ELO first ──────────────────────
    let players: Vec<(String, Keypair, u32)> = (0..player_count)
        .map(|i| (format!("P{}", i + 1), Keypair::new(), 2800 - 50 * i as u32))
        .collect();

    // Fund the players (chunked — a single tx can't hold 64 transfers).
    {
        let ixs: Vec<Instruction> = players
            .iter()
            .map(|(_, kp, _)| {
                system_instruction::transfer(&admin.pubkey(), &kp.pubkey(), PLAYER_FUNDING_LAMPORTS)
            })
            .collect();
        for chunk in ixs.chunks(10) {
            let sig = send(&rpc, chunk, admin, &[])?;
            push_step(&mut steps, "Players funded", &sig);
        }
    }

    // ── Tournament setup: initialize + escrow + shards, then prize ──────────
    {
        let init = initialize_tournament_ix(
            program_id,
            admin.pubkey(),
            tournament_id,
            &format!("E2E Cup {player_count}p"),
            ENTRY_FEE_LAMPORTS,
            player_count,
            admin.pubkey(), // host_treasury
            600,            // 10 min base time
            5,              // 5s increment
        )?;
        let escrow = initialize_escrow_ix(program_id, admin.pubkey(), tournament_id)?;
        let shards = initialize_shards_ix(program_id, admin.pubkey(), tournament_id, player_count)?;
        if std::env::var("XFCHESS_E2E_DEBUG").is_ok() {
            let hex: String = init.data.iter().map(|b| format!("{b:02x}")).collect();
            eprintln!(
                "[debug] initialize_tournament data ({} bytes): {hex}",
                init.data.len()
            );
        }
        let sig = send(&rpc, &[init, escrow, shards], admin, &[])?;
        push_step(&mut steps, "Tournament created (init+escrow+shards)", &sig);

        let fund = fund_sol_prize_ix(program_id, admin.pubkey(), tournament_id, PRIZE_LAMPORTS)?;
        let sig = send(&rpc, &[fund], admin, &[])?;
        push_step(&mut steps, "Prize funded", &sig);
    }

    // ── Profiles + registration (one tx per player, player signs) ──────────
    for (i, (name, kp, elo)) in players.iter().enumerate() {
        let username = format!("e2e{}{}", i, tournament_id % 1_000_000);
        let profile = init_profile_ix(
            program_id,
            kp.pubkey(),
            username,
            "US".to_string(),
            100_000_000, // 1973-03-03 — the on-chain gate needs DOB > 0 AND 18+ years old
        )?;
        let register = register_player_ix(
            program_id,
            kp.pubkey(),
            tournament_id,
            player_count,
            admin.pubkey(), // host_treasury
            *elo,
        )?;
        let sig = send(&rpc, &[profile, register], kp, &[])?;
        push_step(&mut steps, &format!("{name} registered (ELO {elo})"), &sig);
        println!("  {name} = {} (ELO {elo})", kp.pubkey());
    }

    // ── Start: locks registration, seeds by ELO, sweeps entry fees ─────────
    {
        let ix = start_tournament_ix(
            program_id,
            admin.pubkey(),
            tournament_id,
            player_count,
            admin.pubkey(),
        )?;
        let sig = send(&rpc, &[ix], admin, &[])?;
        push_step(&mut steps, "Tournament started", &sig);
    }

    // ── Bracket: seeding is ELO-descending = our player order ──────────────
    // Round-1 match i pairs seed i vs seed N-1-i (same layout as the backend
    // store and on-chain final_match_index = total_matches - 1).
    let total_matches = player_count - 1;
    let seeded: Vec<Pubkey> = players.iter().map(|(_, kp, _)| kp.pubkey()).collect();
    let round1 = (player_count / 2) as usize;

    {
        let ixs: Vec<Instruction> = (0..total_matches)
            .map(|i| {
                let (round, next, slot) = bracket_position(player_count, i);
                let (white, black) = if (i as usize) < round1 {
                    (
                        Some(seeded[i as usize]),
                        Some(seeded[player_count as usize - 1 - i as usize]),
                    )
                } else {
                    (None, None) // filled by advance_winner
                };
                initialize_match_ix(
                    program_id,
                    admin.pubkey(),
                    tournament_id,
                    i,
                    round,
                    white,
                    black,
                    next,
                    slot,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        // 2/4-player brackets fit one tx; chunk for larger sizes.
        for chunk in ixs.chunks(6) {
            let sig = send(&rpc, chunk, admin, &[])?;
            push_step(&mut steps, "Bracket matches initialized", &sig);
        }
    }

    // ── Play the bracket: authority records results, higher seed wins ──────
    // Track each match's (white, black) as it fills.
    let mut slots: Vec<(Option<Pubkey>, Option<Pubkey>)> = (0..total_matches)
        .map(|i| {
            if (i as usize) < round1 {
                (
                    Some(seeded[i as usize]),
                    Some(seeded[player_count as usize - 1 - i as usize]),
                )
            } else {
                (None, None)
            }
        })
        .collect();

    let mut champion = seeded[0];
    for i in 0..total_matches {
        let (Some(white), Some(black)) = slots[i as usize] else {
            bail!("match {i} has an unfilled slot — bracket advancement failed");
        };
        // Higher seed = earlier position in `seeded`.
        let (winner, loser) =
            if seeded.iter().position(|p| *p == white) < seeded.iter().position(|p| *p == black) {
                (white, black)
            } else {
                (black, white)
            };

        let record =
            record_match_result_ix(program_id, admin.pubkey(), tournament_id, i, winner, loser)?;
        let (_, next, slot) = bracket_position(player_count, i);
        let mut ixs = vec![record];
        if let Some(next_idx) = next {
            ixs.push(advance_winner_ix(
                program_id,
                admin.pubkey(),
                tournament_id,
                i,
                next_idx,
            )?);
            if slot == 0 {
                slots[next_idx as usize].0 = Some(winner);
            } else {
                slots[next_idx as usize].1 = Some(winner);
            }
        } else {
            champion = winner;
        }
        let sig = send(&rpc, &ixs, admin, &[])?;
        push_step(&mut steps, &format!("Match {i} result: {winner}"), &sig);
    }

    println!("Champion: {champion}");
    Ok(TournamentRunSummary {
        tournament_id,
        player_count,
        champion,
        players: players
            .iter()
            .map(|(name, kp, _)| (name.clone(), kp.pubkey()))
            .collect(),
        steps,
    })
}

/// Signs with `payer` (fee payer) plus `extra` signers and confirms.
fn send(
    rpc: &RpcClient,
    ixs: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> Result<String> {
    let blockhash = rpc.get_latest_blockhash()?;
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(ixs, Some(&payer.pubkey()), &signers, blockhash);
    let sig = rpc
        .send_and_confirm_transaction(&tx)
        .map_err(|e| anyhow::anyhow!("transaction failed: {e}"))?;
    Ok(sig.to_string())
}

fn push_step(steps: &mut Vec<StepLog>, step: &str, sig: &str) {
    println!("  [ok] {step}: {sig}");
    steps.push(StepLog {
        step: step.to_string(),
        signature: sig.to_string(),
    });
}

/// Loads a JSON keypair file (solana-keygen format).
pub fn load_keypair(path: &str) -> Result<Keypair> {
    let data = std::fs::read(path).with_context(|| format!("reading keypair {path}"))?;
    let bytes: Vec<u8> = serde_json::from_slice(&data)?;
    Keypair::try_from(bytes.as_slice()).map_err(|e| anyhow::anyhow!("bad keypair {path}: {e}"))
}
