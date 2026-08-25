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
    /// GDPR consent checkbox — the backend rejects `/identity/register`
    /// outright (`400 GDPR consent is required`) without this being `true`.
    pub consent_kyc: bool,
    /// In-flight submission result, polled by `poll_compliance_submission`.
    /// `Ok(())` = the backend accepted the registration; `Err` carries either
    /// a network failure or the backend's rejection reason (verbatim HTTP
    /// status/body) so a real failure is visible instead of assumed away.
    tx_rx: Option<tokio::sync::oneshot::Receiver<Result<(), String>>>,
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
            consent_kyc: false,
            tx_rx: None,
        }
    }
}

pub struct CompliancePlugin;

impl Plugin for CompliancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ComplianceState>().add_systems(
            Update,
            (poll_compliance_submission, draw_compliance_modal).chain(),
        );
    }
}

/// Drains the in-flight submission's result, if any landed this frame.
/// Separated from `draw_compliance_modal` (rather than polled inline) so the
/// UI closure below only ever reads a already-resolved `SubmissionStatus`,
/// matching the `lobby.tx_rx` poll-then-draw pattern already used for
/// `create_game`/`join_game` elsewhere in this codebase.
fn poll_compliance_submission(mut state: ResMut<ComplianceState>) {
    let Some(rx) = state.tx_rx.as_mut() else {
        return;
    };
    match rx.try_recv() {
        Ok(Ok(())) => {
            state.status = SubmissionStatus::Success;
            state.tx_rx = None;
        }
        Ok(Err(e)) => {
            state.status = SubmissionStatus::Error(e);
            state.tx_rx = None;
        }
        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            state.status =
                SubmissionStatus::Error("Submission task ended without a result".to_string());
            state.tx_rx = None;
        }
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
                            // Matches the four jurisdictions CLAUDE.md's legal
                            // review actually covers (UK/Brazil/Germany/
                            // Canada) — the previous list offered "United
                            // States" (uncovered by that review) and omitted
                            // Germany (which the review does cover).
                            ui.selectable_value(&mut state.country, "United Kingdom".to_string(), "United Kingdom");
                            ui.selectable_value(&mut state.country, "Brazil".to_string(), "Brazil");
                            ui.selectable_value(&mut state.country, "Germany".to_string(), "Germany");
                            ui.selectable_value(&mut state.country, "Canada".to_string(), "Canada");
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
                        "Germany" => "Steuer-ID (11-digit)",
                        "Canada" => "Social Insurance Number (SIN)",
                        _ => "National Tax ID",
                    };
                    ui.label(egui::RichText::new(tax_label).strong());
                    ui.text_edit_singleline(&mut state.tax_id);
                    ui.label(egui::RichText::new("Used strictly once to generate an anonymous blind-index.").small().color(egui::Color32::LIGHT_GRAY));
                });

                ui.add_space(10.0);
                ui.checkbox(
                    &mut state.consent_kyc,
                    "I consent to the collection and processing of my personal data",
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("‹ Back").clicked() {
                        state.step = 1;
                        state.error_msg = None;
                    }

                    if ui.button("Submit Securely ").clicked() {
                        if state.tax_id.is_empty() {
                            state.error_msg = Some("Tax ID cannot be blank".to_string());
                        } else if !state.consent_kyc {
                            state.error_msg = Some("Consent is required to continue".to_string());
                        } else {
                            state.error_msg = None;
                            submit_identity(&mut state);
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

/// Signs `register_identity:{pubkey}:{timestamp}` with the connected wallet
/// and POSTs the full registration payload to `/identity/register`,
/// threading the real result back through `state.tx_rx`.
///
/// Replaces what used to be here: a hardcoded placeholder signature
/// (`sig.verify` on the backend fails on that unconditionally → 401), no
/// `consent_kyc` field at all (→ 400 "GDPR consent is required" even before
/// the signature is checked), a fire-and-forget request whose result was
/// never read, and `state.status` set to `Success` synchronously — before
/// the request had even been sent, let alone answered. The visible symptom
/// was "Verification complete!" on every submission regardless of what the
/// backend actually did with it — see the CARF/KYC audit this fixes.
fn submit_identity(state: &mut ComplianceState) {
    let pubkey = state
        .pubkey
        .clone()
        .unwrap_or_else(|| "11111111111111111111111111111111".to_string());
    let full_name = state.full_name.clone();
    let dob = state.dob.clone();
    let address = state.address.clone();
    let country = state.country.clone();
    let tax_id = state.tax_id.clone();
    let consent_kyc = state.consent_kyc;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let (tx, rx) = tokio::sync::oneshot::channel();
    state.tx_rx = Some(rx);
    state.status = SubmissionStatus::Submitting;

    bevy::tasks::IoTaskPool::get()
        .spawn(async move {
            let result = (|| -> Result<(), String> {
                let message = format!("register_identity:{pubkey}:{timestamp}");

                #[cfg(all(feature = "solana", not(target_os = "android")))]
                let signature_bytes = crate::multiplayer::solana::tauri_signer::sign_message_via_tauri(
                    &message,
                    "Identity verification",
                )?;
                // MWA bridge (plan §4/§5b) isn't built yet — this is the same
                // seam `sign_message_via_tauri` fills on desktop
                // (`signMessages` in MWA terms), not a separate design. Fail
                // honestly rather than fabricate a signature the backend
                // would reject anyway.
                #[cfg(any(target_os = "android", not(feature = "solana")))]
                let signature_bytes: Vec<u8> = {
                    return Err(
                        "Identity verification isn't available on Android yet — wallet message signing requires the Mobile Wallet Adapter bridge, not yet implemented.".to_string(),
                    );
                };

                let signature = bs58::encode(&signature_bytes).into_string();

                // Already-typed, already-correct client helper — same
                // struct the backend deserializes, same URL, same error
                // handling. It existed the whole time; this UI just never
                // called it, building its own raw untyped payload instead.
                let payload = crate::multiplayer::network::vps::IdentityPayload {
                    pubkey,
                    full_name,
                    dob,
                    address,
                    country,
                    tax_id,
                    signature,
                    timestamp,
                    consent_kyc,
                    consent_retention_years: 7,
                };
                crate::multiplayer::network::vps::register_identity(&payload)
            })();

            let _ = tx.send(result);
        })
        .detach();
}
