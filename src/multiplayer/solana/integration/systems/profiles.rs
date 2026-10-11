use super::*;

pub fn fetch_user_status_async(
    mut solana_wallet: Option<ResMut<crate::multiplayer::solana::addon::SolanaWallet>>,
    time: Res<Time>,
    mut poll_timer: Local<f32>,
    mut rx: Local<Option<crossbeam_channel::Receiver<Option<UserStatus>>>>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
) {
    // Drain any pending result first
    if let Some(ref receiver) = *rx {
        match receiver.try_recv() {
            Ok(Some(status)) => {
                if let Some(ref mut w) = solana_wallet {
                    info!(
                        "[USER_STATUS] profile={} email={} kyc={} can_wager={}",
                        status.has_profile, status.has_email, status.has_kyc, status.can_wager
                    );
                    w.user_status = Some(status);
                }
                *rx = None;
            }
            Ok(None) => {
                *rx = None;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                *rx = None;
            }
        }
    }

    // Kick off a new fetch every 30 seconds when connected and idle
    *poll_timer -= time.delta_secs();
    if *poll_timer > 0.0 || rx.is_some() {
        return;
    }
    *poll_timer = 30.0;

    let pubkey = match solana_wallet.as_ref().and_then(|w| w.pubkey) {
        Some(pk) => pk.to_string(),
        None => return,
    };

    let (tx, new_rx) = crossbeam_channel::bounded::<Option<UserStatus>>(1);
    *rx = Some(new_rx);
    tokio_runtime.0.spawn(async move {
        let result = crate::multiplayer::vps_client::get_user_status_async(pubkey)
            .await
            .ok();
        let _ = tx.send(result);
    });
}

pub fn sync_player_profiles(
    mut competitive: ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    mut profile: ResMut<crate::multiplayer::solana::addon::SolanaProfile>,
    solana_state: Res<SolanaIntegrationState>,
    tokio_runtime: Res<crate::multiplayer::TokioRuntime>,
    mut own_rx: Local<
        Option<
            crossbeam_channel::Receiver<Option<crate::multiplayer::network::vps::PlayerProfile>>,
        >,
    >,
    mut opp_rx: Local<
        Option<
            crossbeam_channel::Receiver<Option<crate::multiplayer::network::vps::PlayerProfile>>,
        >,
    >,
    mut last_game_id: Local<Option<u64>>,
    mut last_opp_pk: Local<Option<Pubkey>>,
) {
    // Trigger fetches
    if competitive.active {
        if competitive.game_id != *last_game_id || competitive.opponent_pubkey != *last_opp_pk {
            *last_game_id = competitive.game_id;
            *last_opp_pk = competitive.opponent_pubkey;

            if let Some(pk) = solana_state.wallet_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                // Run blocking profile fetches on spawn_blocking, outside the async runtime.
                tokio_runtime.0.spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        crate::multiplayer::network::vps::fetch_player_profile(&pk_str).ok()
                    })
                    .await
                    .unwrap_or(None);
                    let _ = tx.send(result);
                });
                *own_rx = Some(rx);
            }

            if let Some(pk) = competitive.opponent_pubkey {
                let (tx, rx) = crossbeam_channel::bounded(1);
                let pk_str = pk.to_string();
                tokio_runtime.0.spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        crate::multiplayer::network::vps::fetch_player_profile(&pk_str).ok()
                    })
                    .await
                    .unwrap_or(None);
                    let _ = tx.send(result);
                });
                *opp_rx = Some(rx);
            }
        }
    } else {
        *last_game_id = None;
        *last_opp_pk = None;
    }

    // Poll results
    if let Some(ref rx) = *own_rx {
        if let Ok(res) = rx.try_recv() {
            if let Some(p) = res {
                profile.elo = p.elo;
                profile.username = p.username;
                profile.country = p.country;
                info!(
                    "[PROFILES] Updated own profile: {} ({} ELO)",
                    profile.username, profile.elo
                );
            }
            *own_rx = None;
        }
    }

    if let Some(ref rx) = *opp_rx {
        if let Ok(res) = rx.try_recv() {
            if let Some(p) = res {
                competitive.opponent_elo = p.elo;
                competitive.opponent_username = p.username;
                competitive.opponent_country = p.country;
                info!(
                    "[PROFILES] Updated opponent profile: {} ({} ELO)",
                    competitive.opponent_username, competitive.opponent_elo
                );
            }
            *opp_rx = None;
        }
    }
}
