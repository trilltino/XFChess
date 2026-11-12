//! Pay-first policy (decided 2026-10-26): wagered 1v1 payouts settle on-chain
//! before, and independently of, anti-cheat analysis. Analysis is a post-hoc
//! signal for moderation; it never holds, claws back or reorders a payout.
//!
//! These are source-level invariants, not behaviour tests: they keep the
//! decision visible, and a change to it has to edit this file on purpose.

const FINALIZE_HANDLER: &str = include_str!("../src/signing/routes/main.rs");
const SETTLEMENT_WORKER: &str = include_str!("../src/tasks/settlement_worker.rs");
const ANTICHEAT_ENQUEUE: &str = include_str!("../src/signing/anticheat_enqueue.rs");
const ANTICHEAT_WORKER: &str = include_str!("../src/tasks/anticheat_worker.rs");
const ANTICHEAT_ROUTES: &str = include_str!("../src/signing/routes/anticheat.rs");

/// Calls that move funds or submit transactions.
const MONEY_MARKERS: &[&str] = &[
    "sign_and_submit",
    "finalize_game_ix",
    "finalize_on_chain",
    "withdraw_treasury",
    "money_actions",
    "refund",
    "claim_prize",
];

fn index_of(haystack: &str, needle: &str) -> Option<usize> {
    haystack.find(needle)
}

fn function_body<'a>(source: &'a str, signature: &str) -> Option<&'a str> {
    let start = source.find(signature)?;
    let rest = &source[start..];
    // The next top-level item bounds the body.
    let end = rest[signature.len()..]
        .find("\npub async fn ")
        .or_else(|| rest[signature.len()..].find("\nasync fn "))
        .or_else(|| rest[signature.len()..].find("\nfn "))
        .map_or(rest.len(), |offset| offset + signature.len());
    Some(&rest[..end])
}

#[test]
fn anticheat_modules_cannot_reach_money_paths() {
    for (name, source) in [
        ("anticheat_enqueue.rs", ANTICHEAT_ENQUEUE),
        ("anticheat_worker.rs", ANTICHEAT_WORKER),
        ("routes/anticheat.rs", ANTICHEAT_ROUTES),
    ] {
        for marker in MONEY_MARKERS {
            assert!(
                !source.contains(marker),
                "{name} references `{marker}`: anti-cheat must not hold, claw back or trigger a \
                 payout (pay-first policy). If this is intended, change the policy and this test together."
            );
        }
    }
}

#[test]
fn finalize_handler_submits_payout_before_analysis_is_queued() {
    let Some(body) = function_body(FINALIZE_HANDLER, "pub async fn finalize_game(") else {
        panic!("finalize_game handler not found");
    };
    let (Some(submit), Some(record)) = (
        index_of(body, "sign_and_submit"),
        index_of(body, "record_settled_game"),
    ) else {
        panic!("finalize_game must both submit the payout and record the settled game");
    };
    assert!(
        submit < record,
        "the payout transaction must be submitted before analysis is queued"
    );
    assert!(
        !body.contains("anticheat"),
        "finalize_game must not consult anti-cheat state before paying out"
    );
}

#[test]
fn record_settled_game_runs_after_settlement_and_only_queues_analysis() {
    let Some(body) = function_body(SETTLEMENT_WORKER, "pub async fn record_settled_game(") else {
        panic!("record_settled_game not found");
    };
    for marker in ["sign_and_submit", "finalize_game_ix", "finalize_on_chain"] {
        assert!(
            !body.contains(marker),
            "record_settled_game runs after settlement and must not submit `{marker}`"
        );
    }
    assert!(
        body.contains("enqueue_game_analysis"),
        "settled games must still be queued for analysis"
    );
}
