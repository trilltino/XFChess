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
        // Submit delegated-account writes directly to the owning ER validator: the router
        // and validator have different blockhash domains. ER_RPC_URL must match the
        // delegation record; use er_probe when changing endpoints.
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
