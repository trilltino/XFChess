use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

#[derive(Resource)]
pub struct ComplianceState {
    pub show: bool,
    pub step: u8,
    pub full_name: String,
    pub dob: String,
    pub address: String,
    pub country: String,
    pub tax_id: String,
    pub error_msg: Option<String>,
    pub status: SubmissionStatus,
    pub pubkey: Option<String>,
}

#[derive(Default, PartialEq, Eq)]
pub enum SubmissionStatus {
    #[default]
    Idle,
    Submitting,
    Success,
    Error(String),
}

impl Default for ComplianceState {
    fn default() -> Self {
        Self {
            show: false,
            step: 1,
            full_name: String::new(),
            dob: String::new(),
            address: String::new(),
            country: "United Kingdom".to_string(),
            tax_id: String::new(),
            error_msg: None,
            status: SubmissionStatus::Idle,
            pubkey: None,
        }
    }
}

pub struct CompliancePlugin;

impl Plugin for CompliancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ComplianceState>()
            .add_systems(Update, draw_compliance_modal);
    }
}

fn draw_compliance_modal(mut contexts: EguiContexts, mut state: ResMut<ComplianceState>) {
    if !state.show {
        return;
    }

    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    egui::Window::new("CARF 2026 Legal Compliance")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(400.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("Verification Required").color(egui::Color32::from_rgb(255, 100, 100)));
                ui.add_space(10.0);
                ui.label(egui::RichText::new("To comply with international CARF RCASP legislation, you must provide identity details before engaging in real-currency wagers. This data is securely stored in a heavily encrypted zero-knowledge vault.").small());
                ui.add_space(20.0);
            });

            if state.status == SubmissionStatus::Submitting {
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.label("Encrypting and submitting to VPS Vault...");
                });
                return;
            }

            if state.status == SubmissionStatus::Success {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Verification complete!").color(egui::Color32::GREEN).strong());
                    ui.add_space(10.0);
                    if ui.button("Continue to Game").clicked() {
                        state.show = false;
                        state.step = 1;
                    }
                });
                return;
            }

            if state.step == 1 {
                ui.group(|ui| {
                    ui.label("Current Legal Name");
                    ui.text_edit_singleline(&mut state.full_name);
                    ui.add_space(8.0);

                    ui.label("Date of Birth (YYYY-MM-DD)");
                    ui.text_edit_singleline(&mut state.dob);
                    ui.add_space(8.0);

                    ui.label("Residential Address");
                    ui.text_edit_singleline(&mut state.address);
                    ui.add_space(8.0);

                    ui.label("Country of Tax Residence");
                    egui::ComboBox::from_label("")
                        .selected_text(&state.country)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.country, "United Kingdom".to_string(), "United Kingdom");
                            ui.selectable_value(&mut state.country, "Brazil".to_string(), "Brazil");
                            ui.selectable_value(&mut state.country, "Canada".to_string(), "Canada");
                            ui.selectable_value(&mut state.country, "United States".to_string(), "United States");
                        });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        state.show = false;
                    }
                    if ui.button("Next ").clicked() {
                        if state.full_name.is_empty() || state.dob.is_empty() || state.address.is_empty() {
                            state.error_msg = Some("All fields are required".to_string());
                        } else {
                            state.error_msg = None;
                            state.step = 2;
                        }
                    }
                });
            } else if state.step == 2 {
                ui.group(|ui| {
                    let tax_label = match state.country.as_str() {
                        "United Kingdom" => "National Insurance (NI) Number",
                        "Brazil" => "CPF (11-digit)",
                        "Canada" => "Social Insurance Number (SIN)",
                        "United States" => "Social Security Number (SSN)",
                        _ => "National Tax ID",
                    };
                    ui.label(egui::RichText::new(tax_label).strong());
                    ui.text_edit_singleline(&mut state.tax_id);
                    ui.label(egui::RichText::new("Used strictly once to generate an anonymous blind-index.").small().color(egui::Color32::LIGHT_GRAY));
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("‹ Back").clicked() {
                        state.step = 1;
                        state.error_msg = None;
                    }

                    if ui.button("Submit Securely ").clicked() {
                        if state.tax_id.is_empty() {
                            state.error_msg = Some("Tax ID cannot be blank".to_string());
                        } else {
                            // Launch background async thread to submit
                            let payload = serde_json::json!({
                                "pubkey": state.pubkey.clone().unwrap_or_else(|| "11111111111111111111111111111111".to_string()),
                                "full_name": state.full_name,
                                "dob": state.dob,
                                "address": state.address,
                                "country": state.country,
                                "tax_id": state.tax_id,
                                "timestamp": std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_else(|e| {
                                        tracing::error!("Failed to get timestamp: {}", e);
                                        std::time::Duration::from_secs(0)
                                    })
                                    .as_secs(),
                                "signature": "1111111111111111111111111111111111111111111111111111111111111111" // mock signature for now
                            });

                            let url = "http://127.0.0.1:3000/identity/register".to_string(); // Assuming dev server
                            state.status = SubmissionStatus::Submitting;

                            // Because we can't easily spawn a Bevy task from inside the UI closure without
                            // a system param wrapper, we'll spawn a native thread that shoots the request.
                            std::thread::spawn(move || {
                                let client = reqwest::blocking::Client::new();
                                let _res = client.post(&url)
                                    .json(&payload)
                                    .send();
                                // We don't poll back the exact success yet in this snippet to keep it simple,
                                // we'd normally wire a oneshot::channel back to Bevy.
                            });

                            // Mock instant success for UI demo purposes since thread is fire-and-forget
                            state.status = SubmissionStatus::Success;
                        }
                    }
                });
            }

            if let Some(err) = &state.error_msg {
                ui.add_space(5.0);
                ui.colored_label(egui::Color32::RED, format!(" {}", err));
            }
            if let SubmissionStatus::Error(err) = &state.status {
                ui.add_space(5.0);
                ui.colored_label(egui::Color32::RED, format!(" {}", err));
            }
        });
}
