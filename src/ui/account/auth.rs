use crate::core::GameState;
use crate::states::main_menu::PlayerIdentity;
use bevy::prelude::*;
use bevy::tasks::Task;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use futures_lite::future;
use serde::Deserialize;

fn auth_base_url() -> String {
    std::env::var("SIGNING_SERVICE_URL")
        .or_else(|_| std::env::var("BACKEND_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string())
}

// --- Resources ---

#[derive(Resource, Default)]
pub struct AuthState {
    pub username: String,
    pub email: String,
    pub password: String,
    pub error: Option<String>,
    pub is_loading: bool,
    pub mode: AuthMode,
    pub token: Option<String>,
    pub show_modal: bool,
    pub wallet_connected: bool,
    pub wallet_pubkey: Option<String>,
    pub wallet_needs_registration: bool,
}

#[derive(Resource, Default)]
pub struct ProfileConsentState {
    pub show: bool,
}

#[derive(Default, PartialEq, Eq, Debug)]
pub enum AuthMode {
    #[default]
    Login,
    Register,
    WalletConnect,
    WalletRegister,
}

#[derive(Resource)]
pub struct AuthTask(Task<Result<AuthTaskResult, String>>);

#[derive(Debug)]
enum AuthTaskResult {
    Auth(AuthResponse),
    WalletCheck {
        pubkey: String,
        registered: bool,
        has_kyc: bool,
    },
}

// --- API Types ---

#[derive(Deserialize, Debug, Clone)]
pub struct AuthResponse {
    pub token: String,
    pub username: String,
    #[serde(default)]
    pub wallet: String,
}

// --- Plugin ---

pub struct AuthUiPlugin;

impl Plugin for AuthUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AuthState>()
            .init_resource::<ProfileConsentState>()
            .add_systems(OnEnter(GameState::Auth), setup_auth)
            .add_systems(
                EguiPrimaryContextPass,
                auth_ui_system.run_if(in_state(GameState::Auth)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                render_profile_consent_modal.run_if(in_state(GameState::Auth)),
            )
            .add_systems(Update, handle_auth_task);
    }
}

// --- Systems ---

fn setup_auth(mut auth_state: ResMut<AuthState>) {
    auth_state.error = None;
    auth_state.is_loading = false;
    auth_state.mode = AuthMode::Login;
}

fn auth_ui_system(
    mut contexts: EguiContexts,
    mut auth_state: ResMut<AuthState>,
    consent_state: Res<ProfileConsentState>,
    mut commands: Commands,
    mut frames: Local<usize>,
) {
    if consent_state.show {
        return;
    }
    if *frames < 5 {
        *frames += 1;
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Request repaint for smooth animation
    ctx.request_repaint();

    // Dark Theme Colors
    let _bg_dark = egui::Color32::from_rgb(10, 10, 10);
    let text_light = egui::Color32::from_rgb(240, 240, 240);
    let text_dim = egui::Color32::from_rgb(160, 160, 160);
    let accent_color = egui::Color32::from_rgb(220, 140, 60); // Burnt Orange/Gold accent
    let input_bg = egui::Color32::from_rgb(40, 40, 40);
    let input_border = egui::Color32::from_rgb(60, 60, 60);

    // Helper closure for lightening color
    let _lighten = |c: egui::Color32, amount: f32| -> egui::Color32 {
        let r = (c.r() as f32 * (1.0 + amount)).min(255.0) as u8;
        let g = (c.g() as f32 * (1.0 + amount)).min(255.0) as u8;
        let b = (c.b() as f32 * (1.0 + amount)).min(255.0) as u8;
        egui::Color32::from_rgb(r, g, b)
    };

    // Draw Background
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
        .show(ctx, |ui| {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                println!(
                    "Egui Pos: {:?}, Click: {}",
                    pos,
                    ui.input(|i| i.pointer.primary_down())
                );
            } else {
                println!("Egui Pos: None");
            }
            println!("App Loading State: {}", auth_state.is_loading);

            let _rect = ui.max_rect();

            ui.vertical_centered(|ui| {
                ui.add_space(50.0);

                // Animated Title
                render_floating_title(ui, text_light);

                ui.add_space(50.0);

                // Error Message
                if let Some(err) = &auth_state.error {
                    ui.label(
                        egui::RichText::new(err)
                            .size(16.0)
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                    ui.add_space(20.0);
                }

                // Authentication Form Container
                let container_width = 360.0;

                ui.scope(|ui| {
                    // Customize Visuals for Inputs
                    let style = ui.style_mut();

                    // TextEdit background color
                    style.visuals.extreme_bg_color = input_bg;

                    // Border/Stroke for inputs
                    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, input_border);
                    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, accent_color);
                    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5, accent_color);

                    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
                    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
                    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(8);

                    style.visuals.selection.bg_fill = accent_color;

                    style.visuals.selection.bg_fill = accent_color;

                    let input_height = 45.0;
                    let font_size = 18.0;

                    // Show username field for Register or WalletRegister modes
                    if auth_state.mode == AuthMode::Register
                        || auth_state.mode == AuthMode::WalletRegister
                    {
                        ui.add_sized(
                            [container_width, input_height],
                            egui::TextEdit::singleline(&mut auth_state.username)
                                .hint_text(
                                    egui::RichText::new("Username")
                                        .size(font_size)
                                        .color(text_dim),
                                )
                                .font(egui::FontId::proportional(font_size))
                                .text_color(text_light)
                                .margin(egui::Margin::symmetric(10, 10)),
                        );
                        ui.add_space(15.0);
                    }

                    ui.add_sized(
                        [container_width, input_height],
                        egui::TextEdit::singleline(&mut auth_state.email)
                            .hint_text(egui::RichText::new("Email").size(font_size).color(text_dim))
                            .font(egui::FontId::proportional(font_size))
                            .text_color(text_light)
                            .margin(egui::Margin::symmetric(10, 10)),
                    );
                    ui.add_space(15.0);

                    ui.add_sized(
                        [container_width, input_height],
                        egui::TextEdit::singleline(&mut auth_state.password)
                            .password(true)
                            .hint_text(
                                egui::RichText::new("Password")
                                    .size(font_size)
                                    .color(text_dim),
                            )
                            .font(egui::FontId::proportional(font_size))
                            .text_color(text_light)
                            .margin(egui::Margin::symmetric(10, 10)),
                    );
                });

                ui.add_space(30.0);

                if auth_state.is_loading {
                    ui.spinner();
                } else {
                    // Action Button
                    let button_text = match auth_state.mode {
                        AuthMode::Login => "LOGIN",
                        AuthMode::Register => "CREATE ACCOUNT",
                        AuthMode::WalletRegister => "REGISTER WALLET",
                        AuthMode::WalletConnect => "CONNECT WALLET",
                    };

                    if ui
                        .add_sized(
                            [200.0, 50.0],
                            egui::Button::new(
                                egui::RichText::new(button_text)
                                    .size(20.0)
                                    .strong()
                                    .color(egui::Color32::BLACK),
                            )
                            .fill(accent_color)
                            .corner_radius(egui::CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        match auth_state.mode {
                            AuthMode::Login | AuthMode::Register => {
                                perform_auth(&mut auth_state, &mut commands);
                            }
                            AuthMode::WalletRegister => {
                                perform_wallet_register(&mut auth_state, &mut commands);
                            }
                            AuthMode::WalletConnect => {
                                perform_wallet_connect(&mut auth_state, &mut commands);
                            }
                        }
                    }

                    ui.add_space(20.0);

                    ui.add_space(10.0);

                    // Toggle Button (Secondary Action)
                    let toggle_text = match auth_state.mode {
                        AuthMode::Login => "CREATE NEW ACCOUNT",
                        AuthMode::Register => "BACK TO LOGIN",
                        AuthMode::WalletRegister => "BACK TO LOGIN",
                        AuthMode::WalletConnect => "BACK TO LOGIN",
                    };

                    if ui
                        .add_sized(
                            [200.0, 40.0],
                            egui::Button::new(
                                egui::RichText::new(toggle_text)
                                    .size(14.0)
                                    .strong()
                                    .color(text_dim),
                            )
                            .fill(egui::Color32::from_rgb(30, 30, 35))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)))
                            .corner_radius(egui::CornerRadius::same(8)),
                        )
                        .clicked()
                    {
                        auth_state.mode = AuthMode::Login;
                        auth_state.error = None;
                        auth_state.wallet_needs_registration = false;
                    }

                    ui.add_space(10.0);

                    // Login with Wallet (New Option) - only show in Login mode
                    if auth_state.mode == AuthMode::Login {
                        if ui
                            .add_sized(
                                [200.0, 40.0],
                                egui::Button::new(
                                    egui::RichText::new("LOGIN WITH WALLET")
                                        .strong()
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(50, 50, 60))
                                .corner_radius(8.0),
                            )
                            .clicked()
                        {
                            perform_wallet_connect(&mut auth_state, &mut commands);
                        }
                    }
                }
            });
        });
}

fn render_floating_title(ui: &mut egui::Ui, color: egui::Color32) {
    let title = "XFCHESS";
    let font_size = 72.0;

    // Safety check for time access
    let time = ui.input(|i| i.time);

    let char_width_approx = font_size * 0.6;
    let total_width = title.len() as f32 * char_width_approx;

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(total_width, font_size + 40.0), // Extra height for wave
        egui::Sense::hover(),
    );

    let painter = ui.painter();
    let font_id = egui::FontId::proportional(font_size);

    // Center the title horizontally in the allocated rect
    // But since we are in `vertical_centered`, the rect is already centered if we allocated correctly?
    // allocate_exact_size doesn't auto-center an explicit width, so compute the
    // start x manually and draw relative to rect.min.
    let mut x_offset = 0.0;
    let actual_width_estimate = title.len() as f32 * (font_size * 0.55);
    let start_x = rect.center().x - (actual_width_estimate / 2.0);

    for (i, ch) in title.chars().enumerate() {
        let phase = i as f64 * 0.5;
        let wave = (time * 2.0 + phase).sin() as f32 * 8.0;

        let pos = egui::pos2(start_x + x_offset, rect.min.y + 20.0 - wave);

        let text_layout = painter.layout_no_wrap(ch.to_string(), font_id.clone(), color);
        let width = text_layout.size().x;

        painter.galley(pos, text_layout, color);
        x_offset += width;
    }
}

pub fn perform_auth(auth_state: &mut ResMut<AuthState>, commands: &mut Commands) {
    auth_state.is_loading = true;
    auth_state.error = None;

    let thread_pool = bevy::tasks::AsyncComputeTaskPool::get();
    let base_url = auth_base_url();
    let email = auth_state.email.clone();
    let password = auth_state.password.clone();
    let username = auth_state.username.clone();
    let is_register = auth_state.mode == AuthMode::Register;

    let task = thread_pool.spawn(async move {
        let client = reqwest::blocking::Client::new();
        let url = if is_register {
            format!("{}/api/auth/register-email", base_url)
        } else {
            format!("{}/api/auth/login-email", base_url)
        };

        let body = if is_register {
            serde_json::json!({
                "email": email,
                "password": password,
                "username": username,
            })
        } else {
            serde_json::json!({
                "email": email,
                "password": password,
            })
        };

        let res = client.post(&url).json(&body).send();

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let ct = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if ct.contains("application/json") {
                        response
                            .json::<AuthResponse>()
                            .map_err(|e| e.to_string())
                            .map(|r| AuthTaskResult::Auth(r))
                    } else {
                        let text = response.text().unwrap_or_default();
                        Err(format!("Auth returned non-JSON response: {}", text))
                    }
                } else {
                    let status = response.status();
                    let text = response.text().unwrap_or_default();
                    Err(format!("Auth failed ({}): {}", status, text))
                }
            }
            Err(e) => Err(e.to_string()),
        }
    });

    commands.insert_resource(AuthTask(task));
}

pub fn perform_wallet_connect(auth_state: &mut ResMut<AuthState>, commands: &mut Commands) {
    auth_state.is_loading = true;
    auth_state.error = None;

    // Trigger the Tauri wallet popup to open
    #[cfg(feature = "solana")]
    crate::multiplayer::solana::tauri_signer::open_wallet_browser();
    #[cfg(not(feature = "solana"))]
    bevy::prelude::info!("Solana feature disabled. Cannot open wallet browser.");

    let thread_pool = bevy::tasks::AsyncComputeTaskPool::get();
    let base_url = auth_base_url();

    // Use a plain thread so the blocking wallet poll doesn't occupy an async executor slot.
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        // Poll for wallet pubkey (up to 15 s, 500 ms steps)
        #[cfg_attr(not(feature = "solana"), allow(unused_mut))]
        let mut pubkey: Option<String> = None;
        for _attempt in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            #[cfg(feature = "solana")]
            {
                pubkey = crate::multiplayer::solana::integration::systems::query_wallet_pubkey_from_tauri();
            }
            if pubkey.is_some() {
                break;
            }
        }

        let pubkey = match pubkey {
            Some(pk) => pk,
            None => {
                let _ = tx.send(Err(
                    "Wallet not connected. Please connect your wallet in the popup window."
                        .to_string(),
                ));
                return;
            }
        };

        // Check registration status (5 s timeout)
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        let is_registered = http
            .get(format!("{}/api/auth/check-wallet/{}", base_url, pubkey))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        // Use the shared helper — checks can_wager (the authoritative gate) with a proper timeout
        let can_wager = crate::multiplayer::network::vps::identity::get_user_status(&pubkey)
            .map(|s| s.can_wager)
            .unwrap_or(false);

        let _ = tx.send(Ok(AuthTaskResult::WalletCheck {
            pubkey,
            registered: is_registered,
            has_kyc: can_wager,
        }));
    });

    // Wrap the channel in an async task so the rest of the auth pipeline (AuthTask polling) still works
    let task = thread_pool.spawn(async move {
        rx.recv()
            .unwrap_or_else(|_| Err("Wallet check thread dropped unexpectedly".to_string()))
    });

    commands.insert_resource(AuthTask(task));
}

pub fn perform_wallet_register(auth_state: &mut ResMut<AuthState>, commands: &mut Commands) {
    auth_state.is_loading = true;
    auth_state.error = None;

    let thread_pool = bevy::tasks::AsyncComputeTaskPool::get();
    let base_url = auth_base_url();
    let username = auth_state.username.clone();
    let email = auth_state.email.clone();
    let pubkey = auth_state.wallet_pubkey.clone().unwrap_or_default();

    let task = thread_pool.spawn(async move {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let _msg = format!("xfchess:register:{}", timestamp);

        #[cfg(feature = "solana")]
        let sig_result = crate::multiplayer::solana::tauri_signer::sign_message_via_tauri(&_msg);
        #[cfg(not(feature = "solana"))]
        let sig_result: Result<Vec<u8>, String> = Err("Solana feature disabled".to_string());

        let signature = match sig_result {
            Ok(sig) => sig,
            Err(e) => return Err(format!("Wallet sign failed: {}", e)),
        };

        let body = serde_json::json!({
            "wallet": pubkey,
            "signature": bs58::encode(&signature).into_string(),
            "timestamp": timestamp,
            "username": username,
            "email": email,
        });

        let client = reqwest::blocking::Client::new();
        let url = format!("{}/api/auth/register", base_url);
        let res = client.post(&url).json(&body).send();

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let ct = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if ct.contains("application/json") {
                        response
                            .json::<AuthResponse>()
                            .map_err(|e| e.to_string())
                            .map(|r| AuthTaskResult::Auth(r))
                    } else {
                        let text = response
                            .text()
                            .unwrap_or_else(|e| format!("[failed to read response: {}]", e));
                        Err(format!("Register returned non-JSON response: {}", text))
                    }
                } else {
                    let status = response.status();
                    let text = response
                        .text()
                        .unwrap_or_else(|e| format!("[failed to read response: {}]", e));
                    Err(format!("Register failed ({}): {}", status, text))
                }
            }
            Err(e) => Err(e.to_string()),
        }
    });

    commands.insert_resource(AuthTask(task));
}

pub fn perform_wallet_login(auth_state: &mut ResMut<AuthState>, commands: &mut Commands) {
    auth_state.is_loading = true;
    auth_state.error = None;

    let thread_pool = bevy::tasks::AsyncComputeTaskPool::get();
    let base_url = auth_base_url();
    let pubkey = auth_state.wallet_pubkey.clone().unwrap_or_default();

    let task = thread_pool.spawn(async move {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let _msg = format!("xfchess:login:{}", timestamp);

        #[cfg(feature = "solana")]
        let sig_result = crate::multiplayer::solana::tauri_signer::sign_message_via_tauri(&_msg);
        #[cfg(not(feature = "solana"))]
        let sig_result: Result<Vec<u8>, String> = Err("Solana feature disabled".to_string());

        let signature = match sig_result {
            Ok(sig) => sig,
            Err(e) => return Err(format!("Wallet sign failed: {}", e)),
        };

        let body = serde_json::json!({
            "wallet": pubkey,
            "signature": bs58::encode(&signature).into_string(),
            "timestamp": timestamp,
        });

        let client = reqwest::blocking::Client::new();
        let url = format!("{}/api/auth/login", base_url);
        let res = client.post(&url).json(&body).send();

        match res {
            Ok(response) => {
                if response.status().is_success() {
                    let ct = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    if ct.contains("application/json") {
                        response
                            .json::<AuthResponse>()
                            .map_err(|e| e.to_string())
                            .map(|r| AuthTaskResult::Auth(r))
                    } else {
                        let text = response.text().unwrap_or_default();
                        Err(format!("Login returned non-JSON response: {}", text))
                    }
                } else {
                    let status = response.status();
                    let text = response.text().unwrap_or_default();
                    Err(format!("Login failed ({}): {}", status, text))
                }
            }
            Err(e) => Err(e.to_string()),
        }
    });

    commands.insert_resource(AuthTask(task));
}

fn handle_auth_task(
    mut commands: Commands,
    auth_task: Option<ResMut<AuthTask>>,
    mut auth_state: ResMut<AuthState>,
    mut consent_state: ResMut<ProfileConsentState>,
    _next_state: ResMut<NextState<GameState>>,
    _current_state: Res<State<GameState>>,
    player_identity: Option<ResMut<PlayerIdentity>>,
) {
    if let Some(mut task) = auth_task {
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            auth_state.is_loading = false;
            match result {
                Ok(AuthTaskResult::Auth(response)) => {
                    auth_state.token = Some(response.token);
                    auth_state.show_modal = false;
                    auth_state.error = None;
                    if let Some(mut identity) = player_identity {
                        identity.username = Some(response.username.clone());
                    }
                    info!("Authenticated as: {}", response.username);

                    // After successful auth, show the "Complete Profile" consent modal
                    consent_state.show = true;
                }
                Ok(AuthTaskResult::WalletCheck {
                    pubkey,
                    registered,
                    has_kyc,
                }) => {
                    auth_state.wallet_pubkey = Some(pubkey.clone());
                    auth_state.wallet_connected = true;
                    if registered {
                        if !has_kyc {
                            // Wallet registered but no KYC, prompt for KYC
                            auth_state.error = Some("Account exists but KYC verification required. Please complete KYC on the web site.".to_string());
                        } else {
                            // Wallet registered and KYC complete, proceed to login
                            perform_wallet_login(&mut auth_state, &mut commands);
                        }
                    } else {
                        // Wallet not registered, show registration form
                        auth_state.mode = AuthMode::WalletRegister;
                        auth_state.wallet_needs_registration = true;
                    }
                }
                Err(e) => {
                    auth_state.error = Some(e);
                }
            }
            commands.remove_resource::<AuthTask>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_default() {
        let state = AuthState::default();

        assert!(state.username.is_empty());
        assert!(state.email.is_empty());
        assert!(state.password.is_empty());
        assert!(state.error.is_none());
        assert!(!state.is_loading);
        assert_eq!(state.mode, AuthMode::Login);
        assert!(state.token.is_none());
    }

    #[test]
    fn test_auth_mode_default_is_login() {
        let mode = AuthMode::default();
        assert_eq!(mode, AuthMode::Login);
    }

    #[test]
    fn test_auth_mode_equality() {
        assert_eq!(AuthMode::Login, AuthMode::Login);
        assert_eq!(AuthMode::Register, AuthMode::Register);
        assert_ne!(AuthMode::Login, AuthMode::Register);
    }

    #[test]
    fn test_auth_state_can_store_credentials() {
        let mut state = AuthState::default();

        state.username = "testuser".to_string();
        state.email = "test@example.com".to_string();
        state.password = "securepass123".to_string();

        assert_eq!(state.username, "testuser");
        assert_eq!(state.email, "test@example.com");
        assert_eq!(state.password, "securepass123");
    }

    #[test]
    fn test_auth_state_loading_toggle() {
        let mut state = AuthState::default();

        assert!(!state.is_loading);
        state.is_loading = true;
        assert!(state.is_loading);
        state.is_loading = false;
        assert!(!state.is_loading);
    }

    #[test]
    fn test_auth_state_error_handling() {
        let mut state = AuthState::default();

        assert!(state.error.is_none());
        state.error = Some("Invalid credentials".to_string());
        assert_eq!(state.error, Some("Invalid credentials".to_string()));
        state.error = None;
        assert!(state.error.is_none());
    }

    #[test]
    fn test_auth_response_deserialization() {
        let json = r#"{
            "token": "jwt_token_here",
            "username": "testuser"
        }"#;

        let response: AuthResponse = match serde_json::from_str(json) {
            Ok(response) => response,
            Err(e) => panic!("Should deserialize: {}", e),
        };

        assert_eq!(response.token, "jwt_token_here");
        assert_eq!(response.username, "testuser");
    }
}

pub fn render_profile_consent_modal(
    mut contexts: EguiContexts,
    mut consent_state: ResMut<ProfileConsentState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if !consent_state.show {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Complete Profile")
        .id(egui::Id::new("profile_consent_modal"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([400.0, 250.0])
        .frame(egui::Frame::window(&ctx.style())
            .fill(egui::Color32::from_rgb(15, 15, 20))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 140, 60)))
            .corner_radius(12.0)
            .inner_margin(20.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.heading(egui::RichText::new("Verification Required")
                    .color(egui::Color32::from_rgb(220, 140, 60))
                    .size(24.0));
                
                ui.add_space(20.0);
                
                ui.label(egui::RichText::new("In order to participate in Wager matches KYC and a Solana Wallet is required do you wish to proceed?")
                    .size(14.0)
                    .color(egui::Color32::WHITE));
                
                ui.add_space(30.0);
                
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    
                    // Yes button
                    if ui.add_sized([160.0, 40.0], egui::Button::new(
                        egui::RichText::new("Yes").strong().size(16.0)
                    ).fill(egui::Color32::from_rgb(220, 140, 60)).corner_radius(8.0)).clicked() {
                        consent_state.show = false;
                        next_state.set(GameState::MainMenu);
                        // Open Tauri profile step instead of in-game modal
                        std::thread::spawn(|| {
                            let _ = reqwest::blocking::Client::new()
                                .post("http://127.0.0.1:7454/api/open-profile-step")
                                .send();
                        });
                    }
                    
                    ui.add_space(20.0);
                    
                    // No button with italic text
                    if ui.add_sized([160.0, 40.0], egui::Button::new(
                        egui::RichText::new("No, I want to play local-online").italics()
                            .size(13.0)
                    ).fill(egui::Color32::from_rgb(40, 40, 45)).corner_radius(8.0)).clicked() {
                        consent_state.show = false;
                        next_state.set(GameState::MainMenu);
                    }
                });
            });
        });
}
