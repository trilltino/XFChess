use super::*;

pub enum AvatarEntry {
    Loading,
    PendingBytes(Vec<u8>),
    Loaded(egui::TextureHandle),
    Failed,
}

pub(super) const MAX_AVATAR_CACHE_ENTRIES: usize = 200;

#[derive(Resource)]
pub struct AvatarCache {
    pub entries: HashMap<String, AvatarEntry>,
    pub(super) insertion_order: VecDeque<String>,
    pub(super) rx: Receiver<(String, Vec<u8>)>,
    pub tx: Sender<(String, Vec<u8>)>,
}

impl Default for AvatarCache {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
            rx,
            tx,
        }
    }
}

impl AvatarCache {
    pub fn fetch_if_absent(&mut self, name: &str) {
        if self.entries.contains_key(name) {
            return;
        }
        if self.entries.len() >= MAX_AVATAR_CACHE_ENTRIES {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(name.to_string(), AvatarEntry::Loading);
        self.insertion_order.push_back(name.to_string());
        let tx = self.tx.clone();
        let key = name.to_string();
        let base = crate::multiplayer::network::vps::vps_base();
        crate::core::net_task::spawn_detached("game_ui", move || {
            let url = format!("{}/api/players/{}/avatar", base, key);
            let result = reqwest::blocking::get(&url).and_then(|r| r.bytes().map(|b| b.to_vec()));
            match result {
                Ok(bytes) if !bytes.is_empty() => {
                    let _ = tx.send((key, bytes));
                }
                _ => {
                    let _ = tx.send((key, Vec::new()));
                }
            }
        });
    }

    pub fn drain_channel(&mut self) {
        while let Ok((name, bytes)) = self.rx.try_recv() {
            if bytes.is_empty() {
                self.entries.insert(name, AvatarEntry::Failed);
            } else {
                self.entries.insert(name, AvatarEntry::PendingBytes(bytes));
            }
        }
    }

    pub fn flush_pending(&mut self, ctx: &egui::Context) {
        for entry in self.entries.values_mut() {
            if let AvatarEntry::PendingBytes(bytes) = entry {
                match image::load_from_memory(bytes) {
                    Ok(dyn_img) => {
                        let rgba = dyn_img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let color_img = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &rgba,
                        );
                        let tex = ctx.load_texture(
                            "player_avatar",
                            color_img,
                            egui::TextureOptions::LINEAR,
                        );
                        *entry = AvatarEntry::Loaded(tex);
                    }
                    Err(_) => {
                        *entry = AvatarEntry::Failed;
                    }
                }
            }
        }
    }
}

#[cfg(feature = "solana")]
pub(super) fn resolve_online_player_name(
    player_identity: Option<&crate::states::main_menu::PlayerIdentity>,
    fallback_name: Option<&str>,
) -> String {
    player_identity
        .and_then(|id| id.username.as_ref())
        .filter(|name| !name.trim().is_empty())
        .cloned()
        .or_else(|| fallback_name.map(str::to_string))
        .unwrap_or_default()
}

#[cfg(feature = "solana")]
pub(super) fn resolve_opponent_display_name(
    p2p_display_name: Option<&str>,
    vps_username: &str,
) -> String {
    p2p_display_name
        .filter(|n| !n.is_empty())
        .unwrap_or(vps_username)
        .to_string()
}

pub(crate) fn resolve_player_names(
    params: &crate::ui::system_params::game_ui::GameUIParams,
    #[cfg_attr(not(feature = "solana"), allow(unused_variables))] local_color: PieceColor,
    is_spectating: bool,
) -> (String, String, String, String) {
    let white_name: String;
    let white_elo: String;
    let black_name: String;
    let black_elo: String;

    if is_spectating {
        let w = params.spectator_mode.white_player.as_ref();
        let b = params.spectator_mode.black_player.as_ref();
        white_name = w
            .map(|p| p.username.clone())
            .unwrap_or_else(|| "White".to_string());
        white_elo = w.map(|p| format!("{}", p.rating)).unwrap_or_default();
        black_name = b
            .map(|p| p.username.clone())
            .unwrap_or_else(|| "Black".to_string());
        black_elo = b.map(|p| format!("{}", p.rating)).unwrap_or_default();
    } else if *params.game_mode == crate::core::GameMode::SinglePlayer {
        let ai_color = params.ai_params.ai_config.mode.ai_color();
        let human_color = match ai_color {
            PieceColor::White => PieceColor::Black,
            PieceColor::Black => PieceColor::White,
        };
        let ai_name = "Computer".to_string();
        let ai_elo = params
            .ai_params
            .ai_config
            .difficulty
            .elo_label()
            .to_string();
        let human_name = params
            .player_identity
            .as_ref()
            .filter(|id| id.username.is_some())
            .map(|id| id.display_name().to_string())
            .unwrap_or_else(|| match human_color {
                PieceColor::White => "White".to_string(),
                PieceColor::Black => "Black".to_string(),
            });
        let human_elo = params
            .player_identity
            .as_ref()
            .map(|id| id.hud_elo_label())
            .unwrap_or_default();
        match ai_color {
            PieceColor::White => {
                white_name = ai_name;
                white_elo = ai_elo;
                black_name = human_name;
                black_elo = human_elo;
            }
            PieceColor::Black => {
                white_name = human_name;
                white_elo = human_elo;
                black_name = ai_name;
                black_elo = ai_elo;
            }
        }
    } else {
        #[cfg(feature = "solana")]
        {
            if let (Some(ref profile), Some(ref comp)) = (
                params.solana_profile.as_ref(),
                params.competitive_match.as_ref(),
            ) {
                // Prefer the signed-in local identity while the async profile name is pending.
                let local_name = resolve_online_player_name(
                    params.player_identity.as_ref().map(|id| id.as_ref()),
                    Some(profile.username.as_str()),
                );
                // Use the handshake display name while the opponent's async profile lookup is pending.
                let opponent_name = resolve_opponent_display_name(
                    params
                        .p2p_conn
                        .as_ref()
                        .and_then(|c| c.opponent_display_name.as_deref()),
                    comp.opponent_username.as_str(),
                );
                let is_white_local = local_color == PieceColor::White;
                if is_white_local {
                    white_name = local_name;
                    white_elo = format!("{}", profile.elo);
                    black_name = opponent_name;
                    black_elo = format!("{}", comp.opponent_elo);
                } else {
                    black_name = local_name;
                    black_elo = format!("{}", profile.elo);
                    white_name = opponent_name;
                    white_elo = format!("{}", comp.opponent_elo);
                }
            } else {
                white_name = params.players.player_1.name.clone();
                white_elo = String::new();
                black_name = params.players.player_2.name.clone();
                black_elo = String::new();
            }
        }
        #[cfg(not(feature = "solana"))]
        {
            white_name = params.players.player_1.name.clone();
            white_elo = String::new();
            black_name = params.players.player_2.name.clone();
            black_elo = String::new();
        }
    }

    (white_name, white_elo, black_name, black_elo)
}

pub(crate) fn render_compact_user_row(
    ui: &mut egui::Ui,
    name: &str,
    _elo: &str,
    online: Option<bool>,
) {
    let display_name = if name.is_empty() { "Player" } else { name };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        // Online indicator dot via unicode — no allocate/painter needed
        if let Some(is_online) = online {
            let (dot, col) = if is_online {
                ("●", egui::Color32::from_rgb(80, 200, 80))
            } else {
                ("●", egui::Color32::from_gray(70))
            };
            ui.label(egui::RichText::new(dot).size(9.0).color(col));
        }
        // Identicon badge via Frame — avoids allocate+painter cursor bug
        let ic_color = identicon_color(display_name);
        let initial = display_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .next()
            .unwrap_or('?');
        egui::Frame::NONE
            .fill(ic_color)
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(5, 2))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(initial.to_string())
                        .size(11.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
            });
        // Name
        ui.label(
            egui::RichText::new(display_name)
                .size(13.0)
                .color(UiColors::TEXT_PRIMARY)
                .strong(),
        );
    });
}

pub fn identicon_color(name: &str) -> egui::Color32 {
    let mut hash: u32 = 2166136261;
    for b in name.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    // Map to 8 hue buckets (HSL-like, pre-defined colors)
    let colors = [
        egui::Color32::from_rgb(70, 130, 200),  // blue
        egui::Color32::from_rgb(60, 160, 100),  // green
        egui::Color32::from_rgb(200, 90, 60),   // red-orange
        egui::Color32::from_rgb(140, 80, 200),  // purple
        egui::Color32::from_rgb(200, 150, 40),  // amber
        egui::Color32::from_rgb(40, 170, 170),  // teal
        egui::Color32::from_rgb(200, 70, 140),  // pink
        egui::Color32::from_rgb(100, 100, 120), // slate
    ];
    colors[(hash as usize) % colors.len()]
}

pub(crate) fn render_captured_pieces_tray(
    ui: &mut egui::Ui,
    pieces: &[crate::rendering::pieces::PieceType],
    is_dark: bool,
) {
    use crate::rendering::pieces::PieceType;
    if pieces.is_empty() {
        return;
    }

    let (sym_color, _bg) = if is_dark {
        (
            egui::Color32::from_gray(30),
            egui::Color32::from_rgba_unmultiplied(230, 220, 195, 60),
        )
    } else {
        (
            egui::Color32::from_gray(225),
            egui::Color32::from_rgba_unmultiplied(50, 50, 60, 60),
        )
    };

    let mut sorted = pieces.to_vec();
    let order = |p: &PieceType| match p {
        PieceType::Queen => 0,
        PieceType::Rook => 1,
        PieceType::Bishop => 2,
        PieceType::Knight => 3,
        PieceType::Pawn => 4,
        PieceType::King => 5,
    };
    sorted.sort_by_key(order);

    // Keep captures on one stable row so adding a captured piece cannot grow
    // the player bar and resize the 2D board underneath it.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for pt in &sorted {
            let sym = if is_dark {
                match pt {
                    PieceType::Queen => "♛",
                    PieceType::Rook => "♜",
                    PieceType::Bishop => "♝",
                    PieceType::Knight => "♞",
                    PieceType::Pawn => "♟",
                    PieceType::King => "♚",
                }
            } else {
                match pt {
                    PieceType::Queen => "♕",
                    PieceType::Rook => "♖",
                    PieceType::Bishop => "♗",
                    PieceType::Knight => "♘",
                    PieceType::Pawn => "♙",
                    PieceType::King => "♔",
                }
            };
            ui.label(egui::RichText::new(sym).size(17.0).color(sym_color));
        }
    });
}

pub fn avatar_fetch_system(
    players: Res<crate::game::resources::Players>,
    mut avatar_cache: ResMut<AvatarCache>,
) {
    avatar_cache.fetch_if_absent(&players.player_1.name);
    avatar_cache.fetch_if_absent(&players.player_2.name);
    avatar_cache.drain_channel();
}

#[cfg(all(test, feature = "solana"))]
mod tests {
    use super::*;
    use crate::states::main_menu::PlayerIdentity;

    #[test]
    fn prefers_signed_in_username_for_local_hud_name() {
        let identity = PlayerIdentity {
            username: Some("Alice".to_string()),
            ..Default::default()
        };

        assert_eq!(
            resolve_online_player_name(Some(&identity), Some("Bob")),
            "Alice"
        );
    }

    #[test]
    fn falls_back_to_profile_name_when_identity_has_no_username() {
        let identity = PlayerIdentity::default();

        assert_eq!(
            resolve_online_player_name(Some(&identity), Some("Bob")),
            "Bob"
        );
    }

    #[test]
    fn prefers_p2p_handshake_name_when_vps_fetch_hasnt_resolved_yet() {
        assert_eq!(resolve_opponent_display_name(Some("Tinog"), ""), "Tinog");
    }

    #[test]
    fn falls_back_to_vps_username_when_p2p_name_unavailable() {
        assert_eq!(resolve_opponent_display_name(None, "Tinog"), "Tinog");
    }

    #[test]
    fn falls_back_to_vps_username_when_p2p_name_is_empty_string() {
        assert_eq!(resolve_opponent_display_name(Some(""), "Tinog"), "Tinog");
    }

    #[test]
    fn prefers_p2p_name_over_a_populated_vps_username_too() {
        // Not just an empty-vs-nonempty tiebreak — P2P is the more current,
        // same-frame source, so it wins even when the VPS value is also set.
        assert_eq!(
            resolve_opponent_display_name(Some("Tinog"), "StaleName"),
            "Tinog"
        );
    }
}
