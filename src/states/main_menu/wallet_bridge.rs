use super::*;

pub fn wallet_connect_overlay_system(
    mut contexts: EguiContexts,
    mut poller: ResMut<WalletBridgePoller>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !poller.show_connect_overlay {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ctx = ctx.clone();

    let screen_rect = ctx.content_rect();

    egui::Area::new("wallet_dim_backdrop".into())
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .show(&ctx, |ui| {
            ui.painter().rect_filled(
                screen_rect,
                egui::CornerRadius::ZERO,
                egui::Color32::from_black_alpha(190),
            );
            ui.allocate_rect(screen_rect, egui::Sense::click_and_drag());
        });

    let overlay_frame = egui::Frame {
        fill: egui::Color32::from_rgba_unmultiplied(18, 18, 24, 255),
        inner_margin: egui::Margin::same(32),
        corner_radius: egui::CornerRadius::same(12),
        stroke: egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(100, 140, 255, 80),
        ),
        shadow: egui::Shadow {
            blur: 40,
            spread: 8,
            color: egui::Color32::from_black_alpha(200),
            offset: [0, 6],
        },
        ..egui::Frame::NONE
    };

    // Escape dismisses the overlay even if popup refocusing consumes a Cancel click.
    let mut cancelled = keyboard.just_pressed(KeyCode::Escape);
    let mut retry = false;
    let message = wallet_connect_overlay_message(&poller);
    let button_text = if poller.bridge_status_error.is_some() {
        "Retry"
    } else {
        "Cancel"
    };

    // Allocate the row at its content width so vertical_centered can center it.
    let button_row_width = if poller.bridge_status_error.is_some() {
        120.0 + 8.0 + 120.0
    } else {
        120.0
    };

    egui::Window::new("##wallet_connect_overlay")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([360.0, 230.0])
        .order(egui::Order::Foreground)
        .frame(overlay_frame)
        .show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Connect Wallet")
                        .size(20.0)
                        .color(egui::Color32::WHITE)
                        .strong()
                        .family(egui::FontFamily::Proportional),
                );
                ui.add_space(12.0);
                ui.spinner();
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(message)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(160, 170, 200))
                        .family(egui::FontFamily::Proportional),
                );
                ui.add_space(12.0);
                if let Some(err) = &poller.bridge_status_error {
                    ui.label(
                        egui::RichText::new(err)
                            .size(10.0)
                            .color(egui::Color32::from_rgb(220, 120, 120))
                            .family(egui::FontFamily::Proportional),
                    );
                    ui.add_space(10.0);
                }
                ui.allocate_ui(egui::vec2(button_row_width, 34.0), |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(
                                [120.0, 34.0],
                                egui::Button::new(
                                    egui::RichText::new(button_text)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(180, 180, 200)),
                                )
                                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14))
                                .corner_radius(6.0)
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                                )),
                            )
                            .clicked()
                        {
                            if poller.bridge_status_error.is_some() {
                                retry = true;
                            } else {
                                cancelled = true;
                            }
                        }
                        if poller.bridge_status_error.is_some()
                            && ui
                                .add_sized(
                                    [120.0, 34.0],
                                    egui::Button::new(
                                        egui::RichText::new("Cancel")
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(180, 180, 200)),
                                    )
                                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14))
                                    .corner_radius(6.0)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                                    )),
                                )
                                .clicked()
                        {
                            cancelled = true;
                        }
                    });
                });
            });
        });

    if cancelled {
        poller.show_connect_overlay = false;
    }
    if retry {
        poller.bridge_status_error = None;
        poller.timer = 5.0;
        poller.status_rx = None;
        poller.show_connect_overlay = true;
    }
}

pub(super) fn wallet_connect_overlay_message(poller: &WalletBridgePoller) -> &'static str {
    if poller.bridge_status_error.is_some() {
        "Cannot reach the wallet bridge. Verify the desktop app is running, then retry."
    } else {
        "Approve the connection in your browser wallet extension."
    }
}

#[derive(Default, Clone)]
pub struct WalletBridgeData {
    pub sol_balance: f64,
    pub usd_balance: Option<f64>,
    pub sol_usd_rate: f64,
    pub sol_gbp_rate: f64,
    pub balance_loaded: bool,
}

#[derive(Debug, Default, Clone)]
pub struct BridgeMeResp {
    pub username: String,
    pub elo: u32,
    pub country: String,
    pub can_wager: bool,
    pub has_onchain_profile: bool,
    pub jwt_token: String,
    pub lichess_blitz: u32,
    pub lichess_verified: bool,
}

#[derive(Resource, Default)]
pub struct WalletBridgePoller {
    pub status_rx: Option<
        crossbeam_channel::Receiver<
            Result<(Option<String>, Option<String>, Option<String>), String>,
        >,
    >,
    pub bridge_status_error: Option<String>,
    pub balance_rx: Option<crossbeam_channel::Receiver<(f64, f64, f64)>>,
    pub me_rx: Option<crossbeam_channel::Receiver<Result<BridgeMeResp, String>>>,
    pub timer: f32,
    pub profile_retry_timer: f32,
    pub known_pubkey: Option<String>,
    pub wallet_provider: Option<String>,
    pub data: std::sync::Arc<parking_lot::Mutex<WalletBridgeData>>,
    pub enabled: bool,
    pub show_connect_overlay: bool,
}

pub(super) fn poll_wallet_bridge(
    mut poller: ResMut<WalletBridgePoller>,
    mut player_identity: ResMut<PlayerIdentity>,
    time: Res<Time>,
) {
    if !poller.enabled {
        return;
    }

    if let Some(ref rx) = poller.me_rx {
        match rx.try_recv() {
            Ok(Ok(me)) => {
                poller.me_rx = None;
                if !me.username.is_empty() {
                    player_identity.username = Some(me.username);
                }
                if me.elo > 0 {
                    player_identity.elo = Some(me.elo);
                }
                if !me.country.is_empty() {
                    player_identity.country = Some(me.country);
                }
                if !me.jwt_token.is_empty() {
                    player_identity.jwt_token = Some(me.jwt_token);
                }
                player_identity.can_wager = me.can_wager;
                player_identity.has_onchain_profile = me.has_onchain_profile;
                // Keep Lichess ELO separate from the on-chain rating; zero means unlinked.
                player_identity.lichess_elo = if me.lichess_blitz > 0 {
                    Some(me.lichess_blitz / 100)
                } else {
                    None
                };
                player_identity.lichess_verified = me.lichess_verified;
                info!(
                    "[WalletBridge] /auth/me: {} ELO {} can_wager={} onchain={}",
                    player_identity.username.as_deref().unwrap_or("?"),
                    player_identity.elo.unwrap_or(0),
                    player_identity.can_wager,
                    player_identity.has_onchain_profile
                );
            }
            Ok(Err(e)) => {
                // JWT unavailable — fall back to raw VPS profile endpoint
                info!(
                    "[WalletBridge] /auth/me unavailable ({}), falling back to VPS",
                    e
                );
                poller.me_rx = None;
                poller.profile_retry_timer = 0.0;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                poller.me_rx = None;
            }
        }
    }

    if let Some(ref rx) = player_identity.pending_profile_rx {
        match rx.try_recv() {
            Ok(Ok(profile)) => {
                if !profile.username.is_empty() {
                    player_identity.username = Some(profile.username.clone());
                }
                if profile.elo > 0 {
                    player_identity.elo = Some(profile.elo);
                }
                if !profile.country.is_empty() {
                    player_identity.country = Some(profile.country);
                }
                info!(
                    "[WalletBridge] VPS profile fallback: {} ELO {}",
                    profile.username, profile.elo
                );
                player_identity.pending_profile_rx = None;
            }
            Ok(Err(e)) => {
                info!("[WalletBridge] VPS profile unavailable ({}), will retry", e);
                player_identity.pending_profile_rx = None;
                poller.profile_retry_timer = 0.0;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                player_identity.pending_profile_rx = None;
            }
        }
    }

    if let Some(ref rx) = poller.status_rx {
        match rx.try_recv() {
            Ok(Ok((pubkey_opt, username_opt, provider_opt))) => {
                poller.status_rx = None;
                poller.bridge_status_error = None;
                poller.wallet_provider = provider_opt;
                if let Some(pk) = pubkey_opt {
                    poller.show_connect_overlay = false;
                    // Adopt bridge usernames after verified connect or confirmed rename, even
                    // when the local name is already set.
                    if player_identity.username.as_deref() != username_opt.as_deref() {
                        if let Some(ref uname) = username_opt {
                            if !uname.is_empty() {
                                info!("[WalletBridge] Username from bridge: {}", uname);
                                player_identity.username = Some(uname.clone());
                            }
                        }
                    }

                    let is_new_pubkey = poller.known_pubkey.as_deref() != Some(&pk);
                    if is_new_pubkey {
                        info!("[WalletBridge] New pubkey detected: {}", pk);
                        // Reset identity so we don't show stale data from previous wallet
                        if poller.known_pubkey.is_some() {
                            player_identity.username = None;
                            player_identity.elo = None;
                            player_identity.country = None;
                            player_identity.jwt_token = None;
                            player_identity.can_wager = false;
                            player_identity.has_onchain_profile = false;
                        }
                        poller.known_pubkey = Some(pk.clone());
                        player_identity.pubkey_str = Some(pk.clone());
                    }

                    // Primary: fetch JWT from bridge then call /auth/me
                    if poller.me_rx.is_none() && player_identity.jwt_token.is_none() {
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        poller.me_rx = Some(rx);
                        crate::core::net_task::spawn_detached("main_menu", move || {
                            let res = fetch_bridge_me();
                            let _ = tx.send(res);
                        });
                    }

                    // Fallback: kick off raw VPS profile fetch if JWT path is unavailable
                    if poller.me_rx.is_none()
                        && player_identity.pending_profile_rx.is_none()
                        && player_identity.jwt_token.is_none()
                        && (player_identity.username.is_none()
                            || username_opt.is_none()
                            || username_opt.as_deref() == Some(""))
                    {
                        let (tx, rx) = crossbeam_channel::bounded(1);
                        player_identity.pending_profile_rx = Some(rx);
                        let pk2 = pk.clone();
                        crate::core::net_task::spawn_detached("main_menu", move || {
                            let res =
                                crate::multiplayer::network::vps::identity::fetch_player_profile(
                                    &pk2,
                                );
                            let _ = tx.send(res);
                        });
                    }

                    // Trigger balance fetch on new pubkey or when Refresh was clicked (balance_rx is None)
                    if is_new_pubkey || poller.balance_rx.is_none() {
                        if is_new_pubkey {
                            // Fresh connection — show a loading state rather than a
                            // stale/default "$0.00" until this fetch actually lands.
                            poller.data.lock().balance_loaded = false;
                        }
                        let (btx, brx) = crossbeam_channel::bounded(1);
                        poller.balance_rx = Some(brx);
                        let data_arc = poller.data.clone();
                        crate::core::net_task::spawn_detached("main_menu", move || {
                            let (sol, usd_per_sol, gbp_per_sol) = fetch_sol_rates(&pk);
                            let _ = btx.send((sol, usd_per_sol, gbp_per_sol));
                            let mut d = data_arc.lock();
                            d.sol_balance = sol;
                            d.usd_balance = if usd_per_sol > 0.0 {
                                Some(sol * usd_per_sol)
                            } else {
                                None
                            };
                            d.sol_usd_rate = usd_per_sol;
                            d.sol_gbp_rate = gbp_per_sol;
                            d.balance_loaded = true;
                        });
                    }
                }
            }
            Ok(Err(err)) => {
                poller.status_rx = None;
                poller.bridge_status_error = Some(err);
                poller.show_connect_overlay = true;
            }
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(_) => {
                poller.status_rx = None;
            }
        }
    }

    if let Some(ref rx) = poller.balance_rx {
        if let Ok((sol, usd_per_sol, gbp_per_sol)) = rx.try_recv() {
            poller.balance_rx = None;
            let mut d = poller.data.lock();
            d.sol_balance = sol;
            d.usd_balance = if usd_per_sol > 0.0 {
                Some(sol * usd_per_sol)
            } else {
                None
            };
            d.balance_loaded = true;
            d.sol_usd_rate = usd_per_sol;
            d.sol_gbp_rate = gbp_per_sol;
        }
    }

    poller.timer += time.delta_secs();
    if poller.timer >= 5.0 && poller.status_rx.is_none() {
        poller.timer = 0.0;
        let (tx, rx) = crossbeam_channel::bounded(1);
        poller.status_rx = Some(rx);
        crate::core::net_task::spawn_detached("main_menu", move || {
            let result = fetch_bridge_status();
            let _ = tx.send(result);
        });
    }

    const PROFILE_RETRY_SECS: f32 = 10.0;
    if player_identity.username.is_none()
        && player_identity.pending_profile_rx.is_none()
        && poller.me_rx.is_none()
        && poller.known_pubkey.is_some()
    {
        poller.profile_retry_timer += time.delta_secs();
        if poller.profile_retry_timer >= PROFILE_RETRY_SECS {
            poller.profile_retry_timer = 0.0;
            // Try JWT path first; fall through to VPS if JWT still absent
            if player_identity.jwt_token.is_none() {
                let (tx, rx) = crossbeam_channel::bounded(1);
                poller.me_rx = Some(rx);
                crate::core::net_task::spawn_detached("main_menu", move || {
                    let _ = tx.send(fetch_bridge_me());
                });
            } else if let Some(pk) = poller.known_pubkey.clone() {
                let (tx, rx) = crossbeam_channel::bounded(1);
                player_identity.pending_profile_rx = Some(rx);
                crate::core::net_task::spawn_detached("main_menu", move || {
                    let res = crate::multiplayer::network::vps::identity::fetch_player_profile(&pk);
                    let _ = tx.send(res);
                });
            }
        }
    } else {
        poller.profile_retry_timer = 0.0;
    }
}

#[cfg(feature = "solana")]
pub(super) fn sync_bridge_pubkey_to_solana(
    poller: Res<WalletBridgePoller>,
    mut solana_state: Option<
        ResMut<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
    mut solana_wallet: Option<ResMut<crate::multiplayer::solana::addon::SolanaWallet>>,
) {
    use crate::multiplayer::solana::integration::state::{ProfileStatus, DEVNET_RPC_URL};

    let Some(ref mut state) = solana_state else {
        return;
    };

    // Always keep balance in sync from the bridge poller (it has fresher data).
    {
        let data = poller.data.lock();
        if data.sol_balance > 0.0 {
            state.balance = data.sol_balance;
        }
        if let Some(usd) = data.usd_balance {
            state.cached_usd_balance = Some(usd);
        }
    }

    // Embedded-wallet status gates global-session setup without extension cluster selection.
    state.wallet_is_embedded = poller.wallet_provider.as_deref() == Some("privy");

    // Invalidate all wallet-derived caches when switching wallets so signers,
    // participant identities, and PDAs are recomputed.
    if let Some(current) = state.wallet_pubkey {
        if let Some(ref pubkey_str) = poller.known_pubkey {
            if let Ok(new_pubkey) = pubkey_str.parse::<solana_sdk::pubkey::Pubkey>() {
                if new_pubkey != current {
                    info!(
                        "[WalletBridge] Wallet switched {} -> {} — clearing cached identity state",
                        current, new_pubkey
                    );
                    state.wallet_pubkey = None;
                    if let Some(wallet) = solana_wallet.as_mut() {
                        wallet.pubkey = None;
                        wallet.keypair = None;
                    }
                    state.session_keypair = None;
                    state.global_session_keypair = None;
                    state.global_session_active = false;
                    state.global_session_setup_in_progress = false;
                    state.global_session_unavailable_reason = None;
                    state.opponent_pubkey = None;
                    state.profile_status = ProfileStatus::Unknown;
                    state.checking_profile = false;
                    state.pending_profile_check = None;
                    state.cached_elo = 0;
                    state.cached_display_name = None;
                    state.handshake_completed = false;
                }
            }
        }
    }

    // Only set pubkey once — avoid overwriting if already set by other path.
    if state.wallet_pubkey.is_some() {
        mirror_bridge_pubkey_to_wallet(&poller, state.wallet_pubkey, solana_wallet.as_deref_mut());
        return;
    }
    let Some(ref pubkey_str) = poller.known_pubkey else {
        return;
    };

    if let Ok(pubkey) = pubkey_str.parse::<solana_sdk::pubkey::Pubkey>() {
        info!(
            "[WalletBridge] Syncing pubkey {} → SolanaIntegrationState",
            pubkey
        );
        state.wallet_pubkey = Some(pubkey);
        if state.rpc_client.is_none() {
            state.rpc_client = Some(solana_client::rpc_client::RpcClient::new(
                DEVNET_RPC_URL.to_string(),
            ));
        }
        mirror_bridge_pubkey_to_wallet(&poller, state.wallet_pubkey, solana_wallet.as_deref_mut());
    }
}

/// The pubkey `SolanaWallet` should take from the bridge, if it does not hold it yet.
///
/// Code throughout the game reads `SolanaWallet.pubkey` to decide whether a wallet is
/// connected, while the bridge reports into `SolanaIntegrationState`. A bridge wallet that
/// only reached the second left readers such as the start-of-game handshake with nothing.
/// Only a pubkey the bridge currently reports is mirrored, so a logout (which clears the
/// bridge's key) is never undone.
#[cfg(feature = "solana")]
fn bridge_pubkey_for_wallet(
    bridge: Option<&str>,
    integration: Option<solana_sdk::pubkey::Pubkey>,
    wallet: Option<solana_sdk::pubkey::Pubkey>,
) -> Option<solana_sdk::pubkey::Pubkey> {
    let bridged = bridge?.parse::<solana_sdk::pubkey::Pubkey>().ok()?;
    (integration == Some(bridged) && wallet != Some(bridged)).then_some(bridged)
}

#[cfg(feature = "solana")]
fn mirror_bridge_pubkey_to_wallet(
    poller: &WalletBridgePoller,
    integration: Option<solana_sdk::pubkey::Pubkey>,
    wallet: Option<&mut crate::multiplayer::solana::addon::SolanaWallet>,
) {
    use solana_sdk::signer::Signer;
    let Some(wallet) = wallet else {
        return;
    };
    let Some(pubkey) =
        bridge_pubkey_for_wallet(poller.known_pubkey.as_deref(), integration, wallet.pubkey)
    else {
        return;
    };
    info!("[WalletBridge] Syncing pubkey {pubkey} → SolanaWallet");
    wallet.pubkey = Some(pubkey);
    // A local keypair for some other wallet must not outlive the pubkey it belonged to.
    if wallet
        .keypair
        .as_ref()
        .is_some_and(|keypair| keypair.pubkey() != pubkey)
    {
        wallet.keypair = None;
    }
}

#[cfg(all(test, feature = "solana"))]
mod bridge_wallet_tests {
    use super::bridge_pubkey_for_wallet;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn a_bridge_wallet_missing_from_the_wallet_resource_is_mirrored() {
        let key = Pubkey::new_unique();
        assert_eq!(
            bridge_pubkey_for_wallet(Some(&key.to_string()), Some(key), None),
            Some(key)
        );
    }

    #[test]
    fn nothing_changes_when_the_wallet_resource_already_matches() {
        let key = Pubkey::new_unique();
        assert_eq!(
            bridge_pubkey_for_wallet(Some(&key.to_string()), Some(key), Some(key)),
            None
        );
    }

    #[test]
    fn a_switched_wallet_replaces_the_stale_one() {
        let (old, new) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert_eq!(
            bridge_pubkey_for_wallet(Some(&new.to_string()), Some(new), Some(old)),
            Some(new)
        );
    }

    #[test]
    fn a_logout_is_not_undone() {
        // The bridge key is cleared on logout; leftover integration state alone must not
        // put the wallet back.
        let key = Pubkey::new_unique();
        assert_eq!(bridge_pubkey_for_wallet(None, Some(key), None), None);
    }

    #[test]
    fn a_bridge_key_the_integration_state_does_not_hold_is_not_mirrored() {
        let (bridged, other) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert_eq!(
            bridge_pubkey_for_wallet(Some(&bridged.to_string()), Some(other), None),
            None
        );
        assert_eq!(
            bridge_pubkey_for_wallet(Some("not-a-key"), None, None),
            None
        );
    }
}

pub(super) fn fetch_bridge_status(
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| format!("bridge client build: {e}"))?;
    let port = crate::multiplayer::network::vps::wallet_bridge_port();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/status"))
        .header("X-Game-Pid", std::process::id())
        .send()
        .map_err(|e| format!("bridge /status: {e}"))?;
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("bridge /status parse: {e}"))?;
    let pubkey = json["pubkey"].as_str().map(|s| s.to_string());
    let username = json["username"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    // Older bridges omit provider; disable embedded-wallet optimizations by default.
    let provider = json["provider"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Ok((pubkey, username, provider))
}

pub(super) fn fetch_bridge_me() -> Result<BridgeMeResp, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .map_err(|e| e.to_string())?;

    let port = crate::multiplayer::network::vps::wallet_bridge_port();

    let token_resp = client
        .get(format!("http://127.0.0.1:{port}/token"))
        .send()
        .map_err(|e| format!("bridge /token: {e}"))?;
    let token_json: serde_json::Value = token_resp
        .json()
        .map_err(|e| format!("bridge /token parse: {e}"))?;
    let jwt = token_json["token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no JWT in bridge /token".to_string())?
        .to_string();

    let base = crate::multiplayer::network::vps::vps_base();
    let me_resp = client
        .get(format!("{base}/api/auth/me"))
        .header("Authorization", format!("Bearer {}", jwt))
        .send()
        .map_err(|e| format!("/auth/me: {e}"))?;
    if !me_resp.status().is_success() {
        return Err(format!("/auth/me HTTP {}", me_resp.status()));
    }

    // /auth/me accepted the JWT — make it the auth token for all subsequent VPS
    // session/move calls (preferred per-user auth on the signing endpoints).
    crate::multiplayer::network::vps::set_auth_token(Some(jwt.clone()));

    let me: serde_json::Value = me_resp.json().map_err(|e| format!("/auth/me parse: {e}"))?;

    Ok(BridgeMeResp {
        username: me["username"].as_str().unwrap_or_default().to_string(),
        elo: me["elo"].as_u64().unwrap_or(0) as u32,
        country: me["country"].as_str().unwrap_or_default().to_string(),
        can_wager: me["can_wager"].as_bool().unwrap_or(false),
        has_onchain_profile: me["has_onchain_profile"].as_bool().unwrap_or(false),
        jwt_token: jwt,
        lichess_blitz: me["lichess_blitz"].as_u64().unwrap_or(0) as u32,
        lichess_verified: me["lichess_verified"].as_bool().unwrap_or(false),
    })
}

pub(super) fn fetch_sol_rates(pubkey: &str) -> (f64, f64, f64) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(_) => return (0.0, 0.0, 0.0),
    };

    let balance_url = format!(
        "{}/api/rpc/mainnet",
        crate::multiplayer::network::vps::vps_base()
    );
    let pubkey = pubkey.to_string();
    let balance_client = client.clone();
    let balance_handle = std::thread::spawn(move || {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "getBalance",
            "params": [pubkey, { "commitment": "confirmed" }]
        });
        balance_client
            .post(&balance_url)
            .json(&body)
            .send()
            .ok()
            .and_then(|r| r.json::<serde_json::Value>().ok())
            .and_then(|j| j["result"]["value"].as_u64())
            .map(|lamports| lamports as f64 / 1_000_000_000.0)
            .unwrap_or(0.0)
    });

    // Fetch conversion rates through vps_base so release builds use their configured backend.
    let rates_url = format!(
        "{}/api/rates/all",
        crate::multiplayer::network::vps::vps_base()
    );
    let rates_json = client
        .get(rates_url)
        .send()
        .ok()
        .and_then(|r| r.json::<serde_json::Value>().ok());

    let usd_per_sol = rates_json
        .as_ref()
        .and_then(|j| j["rates"]["usd"].as_f64())
        .or_else(|| {
            client
                .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
                .send()
                .ok()
                .and_then(|r| r.json::<serde_json::Value>().ok())
                .and_then(|j| j["solana"]["usd"].as_f64())
        })
        .unwrap_or(0.0);

    let gbp_per_sol = rates_json
        .and_then(|j| j["rates"]["gbp"].as_f64())
        .unwrap_or({
            if usd_per_sol > 0.0 {
                usd_per_sol * 0.787
            } else {
                0.0
            }
        });

    let sol = balance_handle.join().unwrap_or(0.0);

    (sol, usd_per_sol, gbp_per_sol)
}
