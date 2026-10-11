use super::*;

pub(crate) fn render_moves_and_controls(
    ui: &mut egui::Ui,
    params: &mut crate::ui::system_params::game_ui::GameUIParams,
) {
    use crate::game::resources::TurnPhase;

    {
        let is_online = matches!(
            *params.game_mode,
            crate::core::GameMode::OnlineMultiplayer
                | crate::core::GameMode::MultiplayerCompetitive
        );

        StyledPanel::sidebar_row()
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("MOVES")
                        .size(12.0)
                        .strong()
                        .color(UiColors::TEXT_TERTIARY),
                );
            });

        // Fixed height — the list scrolls internally and never grows the
        // panel as more moves are played.
        const MOVE_LIST_HEIGHT: f32 = 260.0;

        StyledPanel::sidebar_card()
            .fill(UiColors::BG_MID)
            .inner_margin(egui::Margin::symmetric(0, 0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("game_moves_scroll")
                    .max_height(MOVE_LIST_HEIGHT)
                    .min_scrolled_height(MOVE_LIST_HEIGHT)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        StyledPanel::sidebar_row()
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                render_move_list_paired(ui, &params.move_history);
                            });
                    });
            });

        ui.add_space(10.0);

        StyledPanel::sidebar_row()
            .inner_margin(egui::Margin::symmetric(12, 0))
            .show(ui, |ui| {
                let is_game_over = params.game_state.game_over.is_game_over();
                let is_waiting = params.turn_ctx.phase == TurnPhase::WaitingForInput;

                if !is_game_over {
                    if is_online {
                        let draw_offered = params.pending_draw.from_player.is_some();
                        if StyledButton::outline_pill(
                            ui,
                            if draw_offered {
                                "DRAW SENT"
                            } else {
                                "OFFER DRAW"
                            },
                            egui::Color32::from_rgb(210, 180, 90),
                            true,
                            !draw_offered,
                        )
                        .clicked()
                        {
                            let player = match params.current_turn.color {
                                PieceColor::White => "white".to_string(),
                                PieceColor::Black => "black".to_string(),
                            };
                            params
                                .draw_writer
                                .write(crate::game::events::DrawOfferEvent {
                                    player,
                                    remote: false,
                                });

                            // Record competitive draw offers on chain; the P2P event updates only the opponent UI.
                            #[cfg(feature = "solana")]
                            if let (Some(wallet_pubkey), Some(game_id), Some(rpc_url)) = (
                                params.solana_wallet.as_ref().and_then(|w| w.pubkey),
                                params.solana_sync.as_ref().and_then(|s| s.game_id),
                                params.solana_sync.as_ref().map(|s| s.rpc_url.clone()),
                            ) {
                                crate::multiplayer::solana::lobby::spawn_offer_draw(
                                    rpc_url,
                                    wallet_pubkey,
                                    game_id,
                                );
                            }
                        }
                        ui.add_space(8.0);
                    }

                    if StyledButton::outline_pill(ui, "RESIGN", UiColors::DANGER, true, is_waiting)
                        .clicked()
                    {
                        let winner = match params.current_turn.color {
                            PieceColor::White => "black".to_string(),
                            PieceColor::Black => "white".to_string(),
                        };
                        params
                            .resign_writer
                            .write(crate::game::events::ResignEvent {
                                winner,
                                remote: false,
                            });
                    }
                }
            });
    }
}

pub(super) fn render_move_list_paired(
    ui: &mut egui::Ui,
    history: &crate::game::resources::history::MoveHistory,
) {
    if history.is_empty() {
        ui.label(
            egui::RichText::new("No moves yet")
                .size(12.0)
                .color(UiColors::TEXT_TERTIARY),
        );
        return;
    }
    let moves = &history.moves;
    let total = moves.len();
    egui::Grid::new("move_list_grid")
        .num_columns(3)
        .min_col_width(44.0)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for move_num in 1..=total.div_ceil(2) {
                let white_idx = (move_num - 1) * 2;
                let black_idx = white_idx + 1;
                ui.label(
                    egui::RichText::new(format!("{}.", move_num))
                        .size(13.0)
                        .color(UiColors::TEXT_TERTIARY),
                );
                if white_idx < total {
                    let mv = &moves[white_idx];
                    ui.label(
                        egui::RichText::new(move_notation(history, white_idx, mv))
                            .size(16.0)
                            .color(UiColors::TEXT_PRIMARY)
                            .strong(),
                    );
                } else {
                    ui.label("");
                }
                if black_idx < total {
                    let mv = &moves[black_idx];
                    ui.label(
                        egui::RichText::new(move_notation(history, black_idx, mv))
                            .size(16.0)
                            .color(UiColors::TEXT_SECONDARY)
                            .strong(),
                    );
                } else if white_idx < total {
                    ui.label(
                        egui::RichText::new("…")
                            .size(16.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
        });
}

pub(super) fn move_notation(
    history: &crate::game::resources::history::MoveHistory,
    index: usize,
    mv: &crate::game::components::MoveRecord,
) -> String {
    history
        .san_at(index)
        .map(str::to_string)
        .unwrap_or_else(|| format_move_algebraic(mv))
}

pub(super) fn format_move_algebraic(mv: &crate::game::components::MoveRecord) -> String {
    use crate::rendering::pieces::PieceType;

    // Piece letter (or empty for pawn)
    let piece_letter = match mv.piece_type {
        PieceType::King => "K",
        PieceType::Queen => "Q",
        PieceType::Rook => "R",
        PieceType::Bishop => "B",
        PieceType::Knight => "N",
        PieceType::Pawn => "",
    };

    // Destination square
    let from_file = (b'a' + mv.from.0) as char;
    let _from_rank = mv.from.1 + 1;
    let to_file = (b'a' + mv.to.0) as char;
    let to_rank = mv.to.1 + 1;

    let mut notation = String::new();

    if mv.is_castling {
        if mv.to.0 > mv.from.0 {
            notation = "O-O".to_string();
        } else {
            notation = "O-O-O".to_string();
        }
    } else {
        // Normal move
        if mv.piece_type == PieceType::Pawn && mv.captured.is_some() {
            // Pawn capture includes file
            notation.push(from_file);
        } else {
            notation.push_str(piece_letter);
        }

        // Capture symbol
        if mv.captured.is_some() {
            notation.push('x');
        }

        // Destination
        notation.push(to_file);
        notation.push_str(&to_rank.to_string());
    }

    // Check/Checkmate
    if mv.is_checkmate {
        notation.push('#');
    } else if mv.is_check {
        notation.push('+');
    }

    notation
}
