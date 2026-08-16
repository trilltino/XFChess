//! Exhaustive instruction -> settlement-layer classification.
//!
//! `MAGICBLOCK.md`'s boundary invariant — "ER hot-path instructions write
//! only delegated accounts; instructions that write escrow, treasury,
//! profile, or player lamports must reject delegated games" — used to be
//! enforced only by each call site in `routes/main.rs` / `tasks/
//! settlement_worker.rs` independently picking `config.solana_rpc_url` or
//! `config.magic_router_rpc_url` by hand. That worked because every call
//! site happened to be correct, not because anything checked it —
//! `magicblock::routing::GAME_WRITES_ONLY_ROUTING_INVARIANT` on the program
//! side was (and still is) a doc-only comment, not a runtime check.
//!
//! `Instr`/`layer_of` turns the per-call-site choice into one exhaustive
//! match: adding a new Game-lifecycle instruction without classifying it
//! here is a compile error (no wildcard arm), not a silently-wrong RPC
//! endpoint discovered in production.
//!
//! Scope: only instructions that touch the delegatable `Game` PDA across its
//! lifecycle. Routes with no relationship to delegation (tournaments, ELO,
//! blinks, identity, admin) keep picking `solana_rpc_url` directly — routing
//! them through this enum would be generality for a case that doesn't exist,
//! since they never touch an account that could be ER-owned.

use super::rpc::make_rpc;
use crate::signing::config::SigningConfig;
use solana_client::rpc_client::RpcClient;

/// Every backend-submitted instruction that participates in the
/// delegate/undelegate lifecycle boundary documented in `MAGICBLOCK.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    CreateGame,
    JoinGame,
    CancelGame,
    FinalizeGame,
    DelegateGame,
    RecordMove,
    ScheduleTimeCheck,
    CancelTimeCheck,
    UndelegateGame,
    RequestForceUndelegate,
    ForceUndelegateAfterTimeout,
    RecoverStuckDelegation,
}

/// Which cluster an instruction must be submitted to. XFChess's own
/// invariant (see module docs) is that every instruction here is entirely
/// ER-hot-path or entirely base-layer, never both — so a per-instruction
/// classification is sufficient; no per-transaction account inspection is
/// needed unless that invariant ever stops holding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Base,
    Er,
}

/// Classifies `instr`. No wildcard arm on purpose — see module docs.
pub fn layer_of(instr: Instr) -> Layer {
    match instr {
        Instr::CreateGame
        | Instr::JoinGame
        | Instr::CancelGame
        | Instr::FinalizeGame
        | Instr::DelegateGame
        | Instr::RequestForceUndelegate
        | Instr::ForceUndelegateAfterTimeout
        | Instr::RecoverStuckDelegation => Layer::Base,
        Instr::RecordMove
        | Instr::ScheduleTimeCheck
        | Instr::CancelTimeCheck
        | Instr::UndelegateGame => Layer::Er,
    }
}

/// Builds an RPC client for `instr`'s classified layer — the single point
/// every Game-lifecycle handler should go through instead of picking
/// `config.solana_rpc_url` / `config.magic_router_rpc_url` by hand.
pub fn rpc_for(config: &SigningConfig, instr: Instr) -> RpcClient {
    let url = rpc_url_for(config, instr);
    match layer_of(instr) {
        Layer::Base => {
            tracing::info!("[ROUTING] {instr:?} -> Base layer {url}");
            make_rpc(&url)
        }
        Layer::Er => {
            tracing::info!("[ROUTING] {instr:?} -> Ephemeral Rollup {url}");
            make_rpc(&url)
        }
    }
}

/// Same as [`rpc_for`] but returns the URL string rather than a constructed
/// client — for call sites that build the client on a `spawn_blocking`
/// thread and only need the URL to move across it.
pub fn rpc_url_for(config: &SigningConfig, instr: Instr) -> String {
    match layer_of(instr) {
        Layer::Base => config.solana_rpc_url.clone(),
        // ER writes go straight to the ER validator, NOT the Magic Router.
        //
        // The router serves `getLatestBlockhash` from its own chain, but
        // forwards any transaction that touches a *delegated* account to the
        // ER validator that owns that delegation — a different chain, with a
        // different blockhash domain. The transaction then fails validation
        // with `-32003 ... Blockhash not found`, no matter how fresh the
        // blockhash was or how many times it is refetched.
        //
        // Reproduced deterministically by `bin/er_probe` (kept in-tree so this
        // can be re-checked if MagicBlock changes the router's behaviour):
        //
        //   router      + no delegated account -> accepted
        //   ER validator+ no delegated account -> accepted
        //   router      + delegated account    -> "Blockhash not found"
        //   ER validator+ delegated account    -> accepted
        //
        // That last pair is the whole bug: it made every `record_move`,
        // `undelegate_game` and `schedule_time_check` fail for the entire life
        // of this integration — no chess move was ever recorded on the ER.
        //
        // Caveat: this pins ER writes to `ER_RPC_URL` (default devnet-eu).
        // `DelegateConfig.validator` is `None`, so MagicBlock chooses the
        // validator; today it lands on devnet-eu, which is what makes this
        // correct. If a game is ever delegated to a different validator, this
        // URL must follow it (read the delegation record) or writes will fail
        // the same way. Use `er_probe` to confirm before changing endpoints.
        Layer::Er => config.er_rpc_url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_layer_instructions_route_to_base() {
        for i in [
            Instr::CreateGame,
            Instr::JoinGame,
            Instr::CancelGame,
            Instr::FinalizeGame,
            Instr::DelegateGame,
            Instr::RequestForceUndelegate,
            Instr::ForceUndelegateAfterTimeout,
            Instr::RecoverStuckDelegation,
        ] {
            assert_eq!(layer_of(i), Layer::Base, "{i:?} should route to base");
        }
    }

    #[test]
    fn er_hot_path_instructions_route_to_er() {
        for i in [
            Instr::RecordMove,
            Instr::ScheduleTimeCheck,
            Instr::CancelTimeCheck,
            Instr::UndelegateGame,
        ] {
            assert_eq!(layer_of(i), Layer::Er, "{i:?} should route to ER");
        }
    }

    #[test]
    fn rpc_for_matches_layer_of() {
        let cfg = SigningConfig {
            solana_rpc_url: "https://base.example".to_string(),
            // ER-layer instructions route to the ER validator directly, not
            // the Magic Router — see this module's `rpc_url_for` doc comment.
            er_rpc_url: "https://er.example".to_string(),
            ..test_config()
        };
        assert_eq!(
            rpc_url_for(&cfg, Instr::DelegateGame),
            "https://base.example"
        );
        assert_eq!(rpc_url_for(&cfg, Instr::RecordMove), "https://er.example");
    }

    /// Minimal `SigningConfig` for the struct-update-syntax test above —
    /// only the RPC fields matter here, everything else is a throwaway.
    fn test_config() -> SigningConfig {
        SigningConfig {
            port: 0,
            solana_rpc_url: String::new(),
            solana_mainnet_rpc_url: None,
            er_rpc_url: String::new(),
            magic_router_rpc_url: String::new(),
            program_id: String::new(),
            jwt_secret: String::new(),
            identity_encryption_key: String::new(),
            identity_salt: String::new(),
            fee_payer_keys: Vec::new(),
            vps_authority_key: None,
            kyc_authority_key: None,
            link_authority_key: None,
            treasury_authority_pubkey: "9jpjASzudVvpbgw5G7zCf7o6EvCw4ejRVcEN1aBLq4Kd".to_string(),
            admin_token: None,
            tournament_fee_recipient: String::new(),
            usdc_mint_pubkey: String::new(),
            lichess_client_id: String::new(),
            allowed_origins: vec![],
        }
    }
}
