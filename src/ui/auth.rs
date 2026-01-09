use crate::core::GameState;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use futures_lite::future;
use serde::{Deserialize, Serialize};

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
}

#[derive(Default, PartialEq, Eq)]
pub enum AuthMode {
    #[default]
    Login,
    Register,
}

#[derive(Resource)]
pub struct AuthTask(Task<Result<AuthResponse, String>>);

// --- API Types ---

#[derive(Serialize)]
struct LoginRequest<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct RegisterRequest<'a> {
    username: &'a str,
    email: &'a str,
    password: &'a str,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

// --- Plugin ---

pub struct AuthUiPlugin;

impl Plugin for AuthUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AuthState>()
            .add_systems(OnEnter(GameState::Auth), setup_auth)
            .add_systems(
                EguiPrimaryContextPass,
                auth_ui_system.run_if(in_state(GameState::Auth)),
            )
            .add_systems(Update, handle_auth_task.run_if(in_state(GameState::Auth)));
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
    mut commands: Commands,
    mut frames: Local<usize>,
) {
    if *frames < 5 {
        *frames += 1;
        return;
    }
    let ctx = contexts.ctx_mut();

    // Safety check for context
    if ctx.is_err() {
        return;
    }
    let ctx = ctx.unwrap();

    // Request repaint for smooth animation
    ctx.request_repaint();

    // Dark Theme Colors
    let _bg_dark = egui::Color32::from_rgb(10, 10, 10);
    // let bg_gradient_top = egui::Color32::from_rgb(20, 20, 20);
    // let bg_gradient_bottom = egui::Color32::from_rgb(5, 5, 5);
    let text_light = egui::Color32::from_rgb(240, 240, 240);
    let text_dim = egui::Color32::from_rgb(160, 160, 160);
    let accent_color = egui::Color32::from_rgb(220, 140, 60); // Burnt Orange/Gold accent
    let input_bg = egui::Color32::from_rgb(40, 40, 40);
    let input_border = egui::Color32::from_rgb(60, 60, 60);

    // Helper closure for lightening color
    let lighten = |c: egui::Color32, amount: f32| -> egui::Color32 {
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

            let rect = ui.max_rect();
            // draw_dark_gradient(ui, rect); // Commented out to test interaction

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

                    if auth_state.mode == AuthMode::Register {
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
                        perform_auth(&mut auth_state, &mut commands);
                    }

                    ui.add_space(20.0);

                    ui.add_space(10.0);

                    // Toggle Button (Secondary Action)
                    let toggle_text = match auth_state.mode {
                        AuthMode::Login => "CREATE NEW ACCOUNT",
                        AuthMode::Register => "BACK TO LOGIN",
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
                        auth_state.mode = match auth_state.mode {
                            AuthMode::Login => AuthMode::Register,
                            AuthMode::Register => AuthMode::Login,
                        };
                        auth_state.error = None;
                    }
                }
            });
        });
}

#[allow(dead_code)] // May be used when gradient background is re-enabled
fn draw_dark_gradient(ui: &mut egui::Ui, rect: egui::Rect) {
    let top_color = egui::Color32::from_rgb(25, 25, 30);
    let bottom_color = egui::Color32::from_rgb(5, 5, 8);

    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.min, top_color);
    mesh.colored_vertex(egui::pos2(rect.max.x, rect.min.y), top_color);
    mesh.colored_vertex(rect.max, bottom_color);
    mesh.colored_vertex(egui::pos2(rect.min.x, rect.max.y), bottom_color);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(mesh);
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
    // Actually, allocate_exact_size reserves space but doesn't auto-center if the width is explicit.
    // The previous layout `vertical_centered` aligns the *center* of widgets.
    // We need to calculate start x.

    // Let's just draw relative to rect.min
    let mut x_offset = 0.0;
    // Center it manually
    let actual_width_estimate = title.len() as f32 * (font_size * 0.55); // tighter estimate
    let start_x = rect.center().x - (actual_width_estimate / 2.0);

    for (i, ch) in title.chars().enumerate() {
        let phase = i as f64 * 0.5;
        let wave = (time * 2.0 + phase).sin() as f32 * 8.0;

        let pos = egui::pos2(start_x + x_offset, rect.min.y + 20.0 - wave);

        let text_layout = painter.layout_no_wrap(ch.to_string(), font_id.clone(), color);
        let width = text_layout.size().x;

        painter.galley(pos, text_layout, egui::Color32::BLACK); // optional shadow if we wanted

        x_offset += width;
    }
}

fn perform_auth(auth_state: &mut ResMut<AuthState>, commands: &mut Commands) {
    auth_state.is_loading = true;
    auth_state.error = None;

    let thread_pool = AsyncComputeTaskPool::get();
    let email = auth_state.email.clone();
    let password = auth_state.password.clone();
    let username = auth_state.username.clone();
    let mode = auth_state.mode == AuthMode::Register;

    // Use blocking HTTP client to avoid Tokio runtime requirement
    let task = thread_pool.spawn(async move {
        // Run blocking code in a separate thread
        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let base_url = "http://localhost:3000";
            let url = if mode {
                format!("{}/auth/register", base_url)
            } else {
                format!("{}/auth/login", base_url)
            };

            let res = if mode {
                let body = serde_json::json!({
                    "username": username,
                    "email": email,
                    "password": password,
                });
                client.post(&url).json(&body).send()
            } else {
                let body = serde_json::json!({
                    "email": email,
                    "password": password,
                });
                client.post(&url).json(&body).send()
            };

            match res {
                Ok(response) => {
                    if response.status().is_success() {
                        if mode {
                            // After registration, auto-login
                            let login_body = serde_json::json!({
                                "email": email,
                                "password": password,
                            });
                            let login_url = format!("{}/auth/login", base_url);
                            let login_res = client.post(&login_url).json(&login_body).send();
                            match login_res {
                                Ok(l_res) => {
                                    if l_res.status().is_success() {
                                        l_res.json::<AuthResponse>().map_err(|e| e.to_string())
                                    } else {
                                        Err("Registration successful, but auto-login failed."
                                            .to_string())
                                    }
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        } else {
                            response.json::<AuthResponse>().map_err(|e| e.to_string())
                        }
                    } else {
                        let status = response.status();
                        let text = response.text().unwrap_or_default();
                        Err(format!("Request failed ({}): {}", status, text))
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        })
        .join()
        .unwrap_or_else(|_| Err("Thread panicked".to_string()))
    });

    commands.insert_resource(AuthTask(task));
}

fn handle_auth_task(
    mut commands: Commands,
    auth_task: Option<ResMut<AuthTask>>,
    mut auth_state: ResMut<AuthState>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if let Some(mut task) = auth_task {
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            auth_state.is_loading = false;
            match result {
                Ok(response) => {
                    auth_state.token = Some(response.token);
                    info!("Authenticated as: {}", response.username);
                    next_state.set(GameState::MainMenu);
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
            "user_id": "user123",
            "username": "testuser"
        }"#;

        let response: AuthResponse = serde_json::from_str(json).expect("Should deserialize");

        assert_eq!(response.token, "jwt_token_here");
        assert_eq!(response.user_id, "user123");
        assert_eq!(response.username, "testuser");
    }
}
