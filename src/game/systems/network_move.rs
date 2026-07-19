use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, Piece, PieceType};
use crate::game::events::{NetworkMoveEvent, RemoteMoveApplied, ResignEvent};
use crate::game::resources::{
    CapturedPieces, CurrentTurn, GameOverState, GameSounds, MoveHistory, PendingTurnAdvance,
    Selection,
};
use crate::game::systems::shared::{execute_move, CapturedTarget, MoveContext};
use crate::multiplayer::network::online_game_session::OnlineGameSession;
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::OnlineNetworkState;
use bevy::prelude::*;

/// Handle network move events by executing them on the local board
pub fn handle_network_moves(
    mut events: MessageReader<NetworkMoveEvent>,
    mut commands: Commands,
    mut pieces_query: Query<(Entity, &mut Piece, &mut HasMoved)>,
    mut selection: ResMut<Selection>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut move_history: ResMut<MoveHistory>,
    mut captured_pieces: ResMut<CapturedPieces>,
    mut engine: ResMut<ChessEngine>,
    game_sounds: Option<Res<GameSounds>>,
    current_turn: Res<CurrentTurn>,
    mut remote_applied: MessageWriter<RemoteMoveApplied>,
    network_state: Option<Res<OnlineNetworkState>>,
    session: Option<Res<OnlineGameSession>>,
) {
    for event in events.read() {
        info!(
            "[NETWORK_MOVE] Processing move: {:?} -> {:?}",
            event.from, event.to
        );

        // 1. Find Source Entity and Piece Data
        let source_data = pieces_query
            .iter()
            .find(|(_, piece, _)| piece.x == event.from.0 && piece.y == event.from.1)
            .map(|(e, p, _)| (e, *p));

        if let Some((entity, piece)) = source_data {
            // 2. Validate turn: remote player must be the side to move
            if piece.color != engine.current_turn {
                warn!(
                    "[NETWORK_MOVE] Rejected move: it's {:?}'s turn but {:?} tried to move",
                    engine.current_turn, piece.color
                );
                continue;
            }

            // 3. Validate move legality using the engine
            let legal_dests = engine.get_legal_moves_for_square(event.from, piece.color);
            if !legal_dests.iter().any(|d| *d == event.to) {
                warn!(
                    "[NETWORK_MOVE] Rejected illegal move {:?} -> {:?} for {:?}",
                    event.from, event.to, piece.color
                );
                continue;
            }

            // 4. Find Potential Capture
            let capture_data = pieces_query
                .iter()
                .find(|(_, p, _)| p.x == event.to.0 && p.y == event.to.1)
                .map(|(e, p, _)| (e, *p));

            let capture_target = if let Some((cap_entity, cap_piece)) = capture_data {
                if cap_piece.color != piece.color {
                    Some(CapturedTarget {
                        entity: cap_entity,
                        piece_type: cap_piece.piece_type,
                        color: cap_piece.color,
                    })
                } else {
                    warn!("[NETWORK_MOVE] Attempted move to occupied square of same color!");
                    None
                }
            } else {
                None
            };

            // 5. Determine first-move status before execute_move borrows the query
            let was_first_move = if let Ok((_, _, has_moved)) = pieces_query.get(entity) {
                !has_moved.moved
            } else {
                false
            };

            // 6. Map Promotion Piece
            let promotion_type = event.promotion.and_then(PieceType::from_char);

            // 7. Execute Move
            let ctx = MoveContext {
                origin: "network_move",
                entity,
                piece,
                target: event.to,
                capture: capture_target,
                promotion: promotion_type,
                was_first_move,
                remote: true,
                move_sound: game_sounds.as_ref().map(|s| s.move_piece.clone()),
                capture_sound: game_sounds.as_ref().map(|s| s.capture_piece.clone()),
                game_id: None, // Remote moves don't need game_id for rollup submission
            };

            execute_move(
                &ctx,
                &mut commands,
                &mut pending_turn,
                &mut move_history,
                &mut captured_pieces,
                &mut engine,
                &mut pieces_query,
                None, // No MoveMadeEvent writer — avoid local echo
                None, // BoardStateSync — network moves don't broadcast
                &current_turn,
            );

            // Emit RemoteMoveApplied so the rollup can record the opponent's move on-chain.
            // The engine is fully updated by execute_move above, so current_fen() is correct.
            {
                let from_col = (b'a' + event.from.0) as char;
                let from_row = event.from.1 + 1;
                let to_col = (b'a' + event.to.0) as char;
                let to_row = event.to.1 + 1;
                let mut uci = format!("{}{}{}{}", from_col, from_row, to_col, to_row);
                if let Some(promo) = event.promotion {
                    let promo_char = match PieceType::from_char(promo) {
                        Some(crate::game::components::PieceType::Queen) => 'q',
                        Some(crate::game::components::PieceType::Rook) => 'r',
                        Some(crate::game::components::PieceType::Bishop) => 'b',
                        Some(crate::game::components::PieceType::Knight) => 'n',
                        _ => 'q',
                    };
                    uci.push(promo_char);
                }
                let fen_after = engine.current_fen().to_string();

                // FEN desync check: compare local result against what the remote reported.
                if let Some(ref expected) = event.expected_fen {
                    if fen_after != *expected {
                        warn!(
                            "[NET] FEN desync after move {} — local: {} | remote: {}",
                            uci, fen_after, expected
                        );
                        // Ask the opponent to resend the authoritative FEN.
                        if let (Some(ns), Some(sess)) = (&network_state, &session) {
                            let game_id = sess.game_id.parse::<u64>().unwrap_or(0);
                            if let Some(tx) = &ns.message_sender {
                                let _ = tx.send(NetworkMessage::ResyncRequest { game_id });
                            }
                        }
                    }
                }

                remote_applied.write(RemoteMoveApplied {
                    uci,
                    next_fen: fen_after,
                });
            }

            // 6. Update Selection (Clear if we moved selected piece)
            if let Some(selected_entity) = selection.selected_entity {
                if selected_entity == entity {
                    selection.selected_entity = None;
                    commands
                        .entity(entity)
                        .remove::<crate::game::components::SelectedPiece>();
                }
            }
        } else {
            warn!("[NETWORK_MOVE] Source piece not found at {:?}", event.from);
        }
    }
}

/// Tracks whether there is a pending incoming draw offer waiting for the local player to respond.
#[derive(Resource, Default)]
pub struct PendingDrawOffer {
    /// Set to the offering player's name when a draw offer is received over the network.
    pub from_player: Option<String>,
}

/// Tracks whether there is a pending incoming rematch offer waiting for the local player to respond.
#[derive(Resource, Default)]
pub struct PendingRematchOffer {
    /// Set to the offering player's name when a rematch offer is received over the network.
    pub from_player: Option<String>,
}

/// Watch for remote [`DrawOfferEvent`]s and store them so the UI can display a banner.
pub fn watch_draw_offers(
    mut events: MessageReader<crate::game::events::DrawOfferEvent>,
    mut pending: ResMut<PendingDrawOffer>,
) {
    for ev in events.read() {
        if ev.remote {
            info!("[DRAW] Received draw offer from {}", ev.player);
            pending.from_player = Some(ev.player.clone());
        }
    }
}

/// Apply an accepted draw by setting game-over state; clear state on decline.
pub fn handle_draw_response_events(
    mut events: MessageReader<crate::game::events::DrawResponseEvent>,
    mut pending: ResMut<PendingDrawOffer>,
    mut game_over: ResMut<GameOverState>,
) {
    for ev in events.read() {
        if ev.accepted {
            info!(
                "[DRAW] Draw accepted by {} (remote={})",
                ev.player, ev.remote
            );
            *game_over = GameOverState::Stalemate; // Stalemate is the closest "draw" variant
        } else {
            info!(
                "[DRAW] Draw declined by {} (remote={})",
                ev.player, ev.remote
            );
        }
        // Either way, clear the pending offer.
        pending.from_player = None;
    }
}

/// Watch for remote [`RematchOfferEvent`]s and store them so the UI can display a banner.
pub fn watch_rematch_offers(
    mut events: MessageReader<crate::game::events::RematchOfferEvent>,
    mut pending: ResMut<PendingRematchOffer>,
) {
    for ev in events.read() {
        if ev.remote {
            info!("[REMATCH] Received rematch offer from {}", ev.player);
            pending.from_player = Some(ev.player.clone());
        }
    }
}

pub fn handle_resign_events(
    mut events: MessageReader<ResignEvent>,
    mut game_over: ResMut<GameOverState>,
) {
    for event in events.read() {
        *game_over = match event.winner.as_str() {
            "white" => GameOverState::WhiteWonByResignation,
            "black" => GameOverState::BlackWonByResignation,
            winner => {
                warn!(
                    "[RESIGN] Unknown winner '{}', ignoring resign event",
                    winner
                );
                continue;
            }
        };

        info!(
            "[RESIGN] Applied {} resignation result (remote={})",
            event.winner, event.remote
        );
    }
}

/// Resolves `FlagTimeoutEvent` into a terminal `GameOverState`.
///
/// If no move has been played yet (the 30-second first-move grace period
/// expired, or an opponent disconnected before making any move) the game is
/// aborted with no winner. Otherwise the flagged player's clock ran out
/// mid-game and their opponent wins on time — this mirrors what
/// `update_game_timer` already sets locally, so a remote `FlagTimeout`
/// arriving here just confirms the same result on the other peer.
pub fn handle_flag_timeout_events(
    mut events: MessageReader<crate::game::events::FlagTimeoutEvent>,
    mut game_over: ResMut<GameOverState>,
    move_history: Res<MoveHistory>,
) {
    for event in events.read() {
        if game_over.is_game_over() {
            continue;
        }

        *game_over = if move_history.is_empty() {
            GameOverState::Aborted
        } else {
            match event.flagged_player.as_str() {
                "white" => GameOverState::BlackWonByTime,
                "black" => GameOverState::WhiteWonByTime,
                flagged => {
                    warn!(
                        "[FLAG] Unknown flagged_player '{}', ignoring flag timeout event",
                        flagged
                    );
                    continue;
                }
            }
        };

        info!(
            "[FLAG] Applied flag timeout for '{}' (remote={}) -> {:?}",
            event.flagged_player, event.remote, *game_over
        );
    }
}
