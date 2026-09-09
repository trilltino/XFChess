use super::rpc::make_rpc;
use crate::signing::config::SigningConfig;
use solana_client::rpc_client::RpcClient;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Base,
    Er,
}

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
