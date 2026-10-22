use super::rpc::make_rpc;
use crate::signing::config::SigningConfig;
use parking_lot::Mutex;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

type ErCacheKey = (String, String, String, u64);
struct CachedEr {
    rpc: Arc<RpcClient>,
    at: Instant,
}
static ER_LOCATIONS: LazyLock<Mutex<std::collections::HashMap<ErCacheKey, CachedEr>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

fn er_cache_key(config: &SigningConfig, game_id: u64) -> ErCacheKey {
    (
        config.magic_router_rpc_url.clone(),
        config.er_rpc_url.clone(),
        config.program_id.clone(),
        game_id,
    )
}

pub fn invalidate_er_for_game(config: &SigningConfig, game_id: u64) {
    ER_LOCATIONS.lock().remove(&er_cache_key(config, game_id));
}

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
    ClaimTimeout,
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
        | Instr::UndelegateGame
        | Instr::ClaimTimeout => Layer::Er,
    }
}

pub fn rpc_for(
    config: &SigningConfig,
    instr: Instr,
) -> solana_client::client_error::Result<Arc<RpcClient>> {
    cached_rpc(&rpc_url_for(config, instr))
}

fn cached_rpc(url: &str) -> solana_client::client_error::Result<Arc<RpcClient>> {
    static CLIENTS: LazyLock<Mutex<std::collections::HashMap<String, Arc<RpcClient>>>> =
        LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(client) = CLIENTS.lock().get(url).cloned() {
        return Ok(client);
    }
    let client = Arc::new(make_rpc(url)?);
    let mut clients = CLIENTS.lock();
    if clients.len() >= 256 {
        clients.clear();
    }
    Ok(clients.entry(url.to_owned()).or_insert(client).clone())
}

/// Resolve the validator that owns this game's live account. A router is a
/// discovery endpoint, not a substitute for the validator's blockhash domain.
pub async fn er_rpc_for_game(
    config: &SigningConfig,
    game_id: u64,
) -> Result<Arc<RpcClient>, String> {
    let key = er_cache_key(config, game_id);
    let cached = {
        let mut locations = ER_LOCATIONS.lock();
        if locations.len() >= 4096 {
            locations.retain(|_, value| value.at.elapsed() < Duration::from_secs(30));
        }
        locations
            .get(&key)
            .filter(|value| value.at.elapsed() < Duration::from_secs(30))
            .map(|value| value.rpc.clone())
    };
    if let Some(rpc) = cached {
        return Ok(rpc);
    }
    let config = config.clone();
    tokio::task::spawn_blocking(move || {
        let program: Pubkey = config
            .program_id
            .parse()
            .map_err(|e| format!("Bad program ID: {e}"))?;
        let game =
            Pubkey::find_program_address(&[super::GAME_SEED, &game_id.to_le_bytes()], &program).0;
        let router = cached_rpc(&config.magic_router_rpc_url).map_err(|error| error.to_string())?;
        let status: serde_json::Value = router
            .send(
                solana_client::rpc_request::RpcRequest::Custom {
                    method: "getDelegationStatus",
                },
                serde_json::json!([game.to_string()]),
            )
            .map_err(|e| format!("Could not locate game's ER validator: {e}"))?;
        if status["isDelegated"].as_bool() != Some(true)
            || status["delegationRecord"]["owner"].as_str() != Some(config.program_id.as_str())
        {
            return Err(
                "Game has no matching live delegation; wait for base-layer confirmation".into(),
            );
        }
        let endpoint = status["fqdn"]
            .as_str()
            .ok_or("Delegation response has no validator endpoint")?;
        validate_er_endpoint(endpoint, &config.er_rpc_url)?;
        let rpc = cached_rpc(endpoint).map_err(|error| error.to_string())?;
        let mut locations = ER_LOCATIONS.lock();
        if locations.len() >= 4096 {
            locations.clear();
        }
        locations.insert(
            key,
            CachedEr {
                rpc: rpc.clone(),
                at: Instant::now(),
            },
        );
        Ok(rpc)
    })
    .await
    .map_err(|e| format!("ER discovery task failed: {e}"))?
}

fn validate_er_endpoint(endpoint: &str, configured: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(endpoint).map_err(|_| "Invalid ER endpoint")?;
    let trusted_config = reqwest::Url::parse(configured).ok();
    let official = url
        .host_str()
        .is_some_and(|host| host.ends_with(".magicblock.app"));
    let configured_origin = trusted_config
        .as_ref()
        .is_some_and(|other| other.origin() == url.origin());
    if url.scheme() != "https"
        || (!official && !configured_origin)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (official && url.port_or_known_default() != Some(443))
    {
        return Err("Untrusted ER validator endpoint".into());
    }
    Ok(())
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
    fn validator_discovery_rejects_untrusted_destinations() {
        let configured = "https://devnet-eu.magicblock.app/";
        assert!(validate_er_endpoint("https://devnet-as.magicblock.app/", configured).is_ok());
        for endpoint in [
            "http://devnet-as.magicblock.app/",
            "https://127.0.0.1/",
            "https://magicblock.app.attacker.example/",
            "https://user@devnet-as.magicblock.app/",
            "https://devnet-as.magicblock.app:8443/",
            "https://devnet-as.magicblock.app/?key=untrusted",
        ] {
            assert!(
                validate_er_endpoint(endpoint, configured).is_err(),
                "{endpoint}"
            );
        }
    }

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
            Instr::ClaimTimeout,
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

    #[test]
    fn routed_clients_are_reused_by_full_endpoint() -> anyhow::Result<()> {
        let mut cfg = test_config();
        cfg.solana_rpc_url = "http://127.0.0.1:1/base?key=a".into();
        cfg.er_rpc_url = "http://127.0.0.1:1/er?key=a".into();
        let base = rpc_for(&cfg, Instr::CreateGame)?;
        assert!(Arc::ptr_eq(&base, &rpc_for(&cfg, Instr::JoinGame)?));
        let er = rpc_for(&cfg, Instr::RecordMove)?;
        assert!(Arc::ptr_eq(&er, &rpc_for(&cfg, Instr::ScheduleTimeCheck)?));
        assert!(!Arc::ptr_eq(&base, &er));
        cfg.er_rpc_url = "http://127.0.0.1:1/er?key=b".into();
        assert!(!Arc::ptr_eq(&er, &rpc_for(&cfg, Instr::RecordMove)?));
        Ok(())
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
