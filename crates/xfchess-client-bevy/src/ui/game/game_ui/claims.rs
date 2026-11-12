//! Controls for the FIDE draw claims (threefold repetition, fifty moves).
//!
//! Shown only to the side to move in a live game whose earlier positions can be
//! proven. A claim made before a move is committed is the digital form of FIDE
//! 9.4: hovering or selecting a piece never forfeits it, and a move chosen for a
//! prospective claim is only *reviewed* until the player declares it.

use super::*;
use crate::game::systems::draw_claims::{ClaimStatus, DrawClaimRequest};

/// Intended moves worth listing, so the panel stays small.
const MAX_LISTED_MOVES: usize = 6;

pub(crate) fn render_claim_controls(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
    is_online: bool,
    is_waiting: bool,
) {
    if params.spectator_mode.active
        || !is_waiting
        || params.game_state.game_over.is_game_over()
        || !params.game_state.engine.history_proven()
    {
        return;
    }
    let my_turn = match params.ai_params.ai_config.mode {
        crate::game::ai::resource::AiMode::VsAI { ai_color } => {
            params.current_turn.color != ai_color
        }
        _ => true,
    };
    let seat_ok = !is_online
        || params
            .p2p_conn
            .as_ref()
            .and_then(|c| c.player_color)
            .is_none_or(|color| color == params.current_turn.color);
    if !my_turn || !seat_ok {
        return;
    }

    let current = params.game_state.engine.claim_availability();
    let record = params.game_state.engine.match_record();
    let near = record.halfmoves() >= 99 || record.occurrences() >= 2;
    // Moves that would create the claimable position, for a prospective claim.
    let prospective: Vec<String> = if near {
        record
            .game()
            .legal_moves()
            .into_iter()
            .filter(|uci| {
                params
                    .game_state
                    .engine
                    .claim_availability_after(uci)
                    .is_some_and(|c| c.repetition || c.fifty_moves)
            })
            .take(MAX_LISTED_MOVES)
            .collect()
    } else {
        Vec::new()
    };
    if !current.repetition && !current.fifty_moves && prospective.is_empty() {
        show_status(ui, &params.claim_ui.status);
        return;
    }

    ui.add_space(8.0);
    let amber = egui::Color32::from_rgb(210, 180, 90);
    if current.repetition
        && StyledButton::outline_pill(ui, "CLAIM DRAW: REPETITION", amber, true, true).clicked()
    {
        params.claim_writer.write(DrawClaimRequest {
            repetition: true,
            intended: None,
        });
    }
    if current.fifty_moves
        && StyledButton::outline_pill(ui, "CLAIM DRAW: 50 MOVES", amber, true, true).clicked()
    {
        params.claim_writer.write(DrawClaimRequest {
            repetition: false,
            intended: None,
        });
    }
    if !prospective.is_empty() && !current.repetition && !current.fifty_moves {
        ui.label(
            egui::RichText::new("Claim by declaring a move")
                .size(11.0)
                .color(UiColors::TEXT_TERTIARY),
        );
        for uci in &prospective {
            let selected = params.claim_ui.selected.as_deref() == Some(uci.as_str());
            if ui.selectable_label(selected, uci).clicked() {
                params.claim_ui.selected = Some(uci.clone());
            }
        }
        let chosen = params.claim_ui.selected.clone();
        if let Some(uci) = chosen {
            let after = params.game_state.engine.claim_availability_after(&uci);
            let repetition = after.is_some_and(|c| c.repetition);
            let label = format!("DECLARE {uci} AND CLAIM");
            if StyledButton::outline_pill(ui, &label, amber, true, after.is_some()).clicked() {
                // Irrevocable: there is no cancel once the claim is filed.
                params.claim_writer.write(DrawClaimRequest {
                    repetition,
                    intended: Some(uci),
                });
            }
        }
    }
    ui.label(
        egui::RichText::new(
            "A correct claim ends the game without your opponent's agreement. \
             Reaching 100 half-moves alone does not end it.",
        )
        .size(10.0)
        .color(UiColors::TEXT_TERTIARY),
    );
    show_status(ui, &params.claim_ui.status);
}

fn show_status(ui: &mut egui::Ui, status: &ClaimStatus) {
    let (text, color) = match status {
        ClaimStatus::Idle => return,
        ClaimStatus::Submitting => (
            "Formal claim submitted: clocks paused".to_string(),
            UiColors::TEXT_SECONDARY,
        ),
        ClaimStatus::Rejected(reason) => {
            (format!("Not a claim: {reason}"), UiColors::TEXT_TERTIARY)
        }
        ClaimStatus::Penalized { declared_move } => (
            match declared_move {
                Some(uci) => {
                    format!("Incorrect claim: opponent awarded two minutes. You must play {uci}.")
                }
                None => "Incorrect claim: opponent awarded two minutes.".to_string(),
            },
            UiColors::DANGER,
        ),
    };
    ui.label(egui::RichText::new(text).size(11.0).color(color));
}
