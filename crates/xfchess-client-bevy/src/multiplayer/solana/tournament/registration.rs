use super::*;

pub fn spawn_register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    password: Option<String>,
) -> oneshot::Receiver<Result<usize, String>> {
    let (tx, rx) = oneshot::channel();
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    crate::core::net_task::spawn_detached("tournament", move || {
        let wallet = wallet_pubkey.to_string();
        crate::multiplayer::network::vps::emit_client_event(
            crate::multiplayer::network::vps::ClientEvent::new("tournament_registration_started")
                .wallet(&wallet)
                .game_id(tournament_id),
        );
        let result = register_tournament(tournament_id, wallet_pubkey, &rpc_url)
            .map_err(|e| format!("on-chain register_player failed: {e}"))
            .and_then(|signature| {
                crate::multiplayer::network::vps::confirm_join_with_retry(
                    tournament_id,
                    &wallet,
                    1200,
                    &signature,
                    password.as_deref(),
                )
                .map(|slot| slot as usize)
            });
        let _ = tx.send(result);
    });
    rx
}

#[derive(Deserialize)]
pub(super) struct RegistrationInfo {
    pub(super) program_id: String,
    pub(super) max_players: u16,
    pub(super) host_treasury: String,
}

pub(super) fn fetch_registration_info(
    rpc_base: &str,
    tournament_id: u64,
) -> Result<RegistrationInfo, String> {
    let resp = crate::multiplayer::network::vps::client_fast()?
        .get(format!(
            "{}/api/tournament/{}/registration-info",
            rpc_base, tournament_id
        ))
        .send()
        .map_err(|e| format!("fetch registration-info: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("fetch registration-info: HTTP {}", resp.status()));
    }
    resp.json::<RegistrationInfo>()
        .map_err(|e| format!("parse registration-info: {e}"))
}

pub fn register_tournament(
    tournament_id: u64,
    wallet_pubkey: Pubkey,
    rpc_url: &str,
) -> Result<String, String> {
    use crate::multiplayer::network::vps::vps_base;
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
    use crate::multiplayer::solana::tournament_session::build_register_player_ix;
    use std::str::FromStr;

    let info = fetch_registration_info(&vps_base(), tournament_id)?;
    let program_id =
        Pubkey::from_str(&info.program_id).map_err(|e| format!("bad program_id: {e}"))?;
    let host_treasury =
        Pubkey::from_str(&info.host_treasury).map_err(|e| format!("bad host_treasury: {e}"))?;

    // Use the same ELO as the off-chain roster join.
    const DEFAULT_ELO: u32 = 1200;

    let ix = build_register_player_ix(
        &program_id,
        tournament_id,
        info.max_players,
        &wallet_pubkey,
        &host_treasury,
        DEFAULT_ELO,
    );

    let sig = sign_and_send_via_tauri(
        rpc_url,
        wallet_pubkey,
        &[ix],
        &[],
        "Registering for tournament",
    )
    .map_err(|e| format!("register_player tx: {e}"))?;

    info!(
        "[TOURNAMENT] register_player confirmed for tournament {} ({})",
        tournament_id, sig
    );
    Ok(sig.to_string())
}
