use super::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerIdentity {
    pub username: Option<String>,
    pub elo: Option<u32>,
    pub country: Option<String>,
    pub jwt_token: Option<String>,
    pub can_wager: bool,
    pub pubkey_str: Option<String>,
    pub has_onchain_profile: bool,
    pub pending_profile_rx: Option<
        crossbeam_channel::Receiver<
            Result<crate::multiplayer::network::vps::identity::PlayerProfile, String>,
        >,
    >,
    pub is_guest: bool,
    pub lichess_elo: Option<u32>,
    pub lichess_verified: bool,
}

impl PlayerIdentity {
    pub fn from_env() -> Self {
        let username = std::env::var("XFCHESS_USERNAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Self {
            username,
            ..Default::default()
        }
    }

    pub fn display_name(&self) -> &str {
        self.username.as_deref().unwrap_or("Guest")
    }

    pub fn display_elo(&self) -> String {
        self.elo
            .map(|e| e.to_string())
            .unwrap_or_else(|| "—".to_string())
    }

    pub fn hud_elo_label(&self) -> String {
        if self.is_guest {
            return String::new();
        }
        let mut parts = Vec::new();
        if let Some(e) = self.elo {
            parts.push(e.to_string());
        }
        if let Some(le) = self.lichess_elo {
            parts.push(format!("{le} (Lichess)"));
        }
        parts.join(" · ")
    }
}

#[cfg(feature = "solana")]
pub(super) fn sync_player_identity_from_wallet(
    mut player_identity: ResMut<PlayerIdentity>,
    solana_state: Option<
        Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
) {
    let Some(ref solana_state) = solana_state else {
        return;
    };
    let Some(pubkey) = solana_state.wallet_pubkey else {
        return;
    };

    // Check if we already have a pending fetch
    if let Some(ref rx) = player_identity.pending_profile_rx {
        match rx.try_recv() {
            Ok(Ok(profile)) => {
                player_identity.username = Some(profile.username.clone());
                player_identity.elo = Some(profile.elo);
                player_identity.country = Some(profile.country);
                info!(
                    "[PROFILE] Fetched profile for {} — ELO {}",
                    profile.username, profile.elo
                );
                player_identity.pending_profile_rx = None;
            }
            Ok(Err(e)) => {
                warn!("[PROFILE] Failed to fetch player profile: {}", e);
                player_identity.pending_profile_rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                player_identity.pending_profile_rx = None;
            }
        }
        return;
    }

    // If we already have ELO cached, don't refetch
    if player_identity.elo.is_some() {
        return;
    }

    let pk = pubkey.to_string();
    let (tx, rx) = crossbeam_channel::bounded(1);
    player_identity.pending_profile_rx = Some(rx);

    crate::core::net_task::spawn_detached("main_menu", move || {
        let res = crate::multiplayer::network::vps::identity::fetch_player_profile(&pk);
        let _ = tx.send(res);
    });
}

#[cfg(not(feature = "solana"))]
pub(super) fn sync_player_identity_from_wallet(_: ResMut<PlayerIdentity>) {}
