use crate::engine::board_state::ChessEngine;
use crate::game::components::{HasMoved, Piece, PieceType};
use crate::game::events::{MoveMadeEvent, NetworkMoveEvent, RemoteMoveApplied, ResignEvent};
use crate::game::resources::{
    CapturedPieces, CurrentTurn, GameOverState, GameSounds, MoveHistory, PendingTurnAdvance,
    Selection,
};
use crate::game::systems::shared::{execute_move, CapturedTarget, MoveContext};
use crate::multiplayer::network::online_game_session::OnlineGameSession;
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::types::CausalChainState;
use crate::multiplayer::OnlineNetworkState;
use bevy::prelude::*;

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
    mut move_events: MessageWriter<MoveMadeEvent>,
    network_state: Option<Res<OnlineNetworkState>>,
    session: Option<Res<OnlineGameSession>>,
    mut causal: Option<ResMut<CausalChainState>>,
    mut results: MessageWriter<crate::game::events::NetworkMoveResult>,
) {
    for event in events.read() {
        info!(
            "[NETWORK_MOVE] Processing move: {:?} -> {:?}",
            event.from, event.to
        );

        let source_data = pieces_query
            .iter()
            .find(|(_, piece, _)| piece.x == event.from.0 && piece.y == event.from.1)
            .map(|(e, p, _)| (e, *p));

        if let Some((entity, piece)) = source_data {
            let release_pending = |causal: &mut Option<ResMut<CausalChainState>>,
                                   version: &Option<String>,
                                   applied: bool| {
                if let (Some(causal), Some(version)) = (causal.as_mut(), version) {
                    let game_id = session
                        .as_ref()
                        .map(|session| {
                            crate::multiplayer::network::online_game_session::numeric_game_id(
                                &session.game_id,
                            )
                        })
                        .unwrap_or_default();
                    if let Some(pending) = causal.pending_versions.get_mut(&game_id) {
                        pending.remove(version);
                    }
                    if applied {
                        causal
                            .applied_versions
                            .entry(game_id)
                            .or_default()
                            .insert(version.clone());
                    }
                }
            };
            // 2. Validate turn: remote player must be the side to move
            if piece.color != engine.current_turn {
                warn!(
                    "[NETWORK_MOVE] Rejected move: it's {:?}'s turn but {:?} tried to move",
                    engine.current_turn, piece.color
                );
                release_pending(&mut causal, &event.dedup_version, false);
                results.write(crate::game::events::NetworkMoveResult {
                    version: event.dedup_version.clone(),
                    fen: None,
                });
                continue;
            }

            let legal_dests = engine.get_legal_moves_for_square(event.from, piece.color);
            if !legal_dests.contains(&event.to) {
                warn!(
                    "[NETWORK_MOVE] Rejected illegal move {:?} -> {:?} for {:?}",
                    event.from, event.to, piece.color
                );
                release_pending(&mut causal, &event.dedup_version, false);
                results.write(crate::game::events::NetworkMoveResult {
                    version: event.dedup_version.clone(),
                    fen: None,
                });
                continue;
            }

            let capture_data = pieces_query
                .iter()
                .find(|(_, p, _)| p.x == event.to.0 && p.y == event.to.1)
                .map(|(e, p, _)| (e, *p));

            let capture_target = match capture_data {
                // Unreachable while the board entities match the engine; if they
                // diverge, the move must be rejected, not applied as a non-capture.
                Some((_, cap_piece)) if cap_piece.color == piece.color => {
                    warn!("[NETWORK_MOVE] Attempted move to occupied square of same color!");
                    release_pending(&mut causal, &event.dedup_version, false);
                    results.write(crate::game::events::NetworkMoveResult {
                        version: event.dedup_version.clone(),
                        fen: None,
                    });
                    continue;
                }
                Some((cap_entity, cap_piece)) => Some(CapturedTarget {
                    entity: cap_entity,
                    piece_type: cap_piece.piece_type,
                    color: cap_piece.color,
                }),
                None => None,
            };

            let was_first_move = if let Ok((_, _, has_moved)) = pieces_query.get(entity) {
                !has_moved.moved
            } else {
                false
            };

            let promotion_type = event.promotion.and_then(PieceType::from_char);

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

            if !execute_move(
                &ctx,
                &mut commands,
                &mut pending_turn,
                &mut move_history,
                &mut captured_pieces,
                &mut engine,
                &mut pieces_query,
                Some(&mut move_events),
                None, // BoardStateSync — network moves don't broadcast
                &current_turn,
            ) {
                release_pending(&mut causal, &event.dedup_version, false);
                results.write(crate::game::events::NetworkMoveResult {
                    version: event.dedup_version.clone(),
                    fen: None,
                });
                continue;
            }

            // Record the remote move after execute_move updates the engine’s FEN.
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
                let matches_expected = event
                    .expected_fen
                    .as_ref()
                    .is_none_or(|expected| expected == &fen_after);
                release_pending(&mut causal, &event.dedup_version, matches_expected);
                results.write(crate::game::events::NetworkMoveResult {
                    version: event.dedup_version.clone(),
                    fen: matches_expected.then(|| fen_after.clone()),
                });

                // FEN desync check: compare local result against what the remote reported.
                if let Some(ref expected) = event.expected_fen {
                    if fen_after != *expected {
                        warn!(
                            "[NET] FEN desync after move {} — local: {} | remote: {}",
                            uci, fen_after, expected
                        );
                        // Ask the opponent to resend the authoritative FEN.
                        if let (Some(ns), Some(sess)) = (&network_state, &session) {
                            let game_id =
                                crate::multiplayer::network::online_game_session::numeric_game_id(
                                    &sess.game_id,
                                );
                            if let Some(tx) = &ns.message_sender {
                                let _ = tx.send(NetworkMessage::ResyncRequest { game_id });
                            }
                        }
                    }
                }

                if matches_expected {
                    remote_applied.write(RemoteMoveApplied {
                        uci,
                        next_fen: fen_after,
                    });
                }
            }

            if let Some(selected_entity) = selection.selected_entity {
                if selected_entity == entity {
                    selection.selected_entity = None;
                    commands
                        .entity(entity)
                        .remove::<crate::game::components::SelectedPiece>();
                }
            }
        } else {
            results.write(crate::game::events::NetworkMoveResult {
                version: event.dedup_version.clone(),
                fen: None,
            });
            warn!("[NETWORK_MOVE] Source piece not found at {:?}", event.from);
            if let (Some(causal), Some(version)) = (causal.as_mut(), &event.dedup_version) {
                let game_id = session
                    .as_ref()
                    .map(|session| {
                        crate::multiplayer::network::online_game_session::numeric_game_id(
                            &session.game_id,
                        )
                    })
                    .unwrap_or_default();
                causal
                    .pending_versions
                    .entry(game_id)
                    .or_default()
                    .remove(version);
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct PendingDrawOffer {
    pub from_player: Option<String>,
}

#[cfg(test)]
mod application_tests {
    use super::*;
    use crate::rendering::pieces::PieceColor;

    fn game() -> App {
        let mut app = App::new();
        let mut engine = ChessEngine::default();
        engine.rebuild_legal_move_cache();
        app.insert_resource(engine);
        app.init_resource::<Selection>();
        app.init_resource::<PendingTurnAdvance>();
        app.init_resource::<MoveHistory>();
        app.init_resource::<CapturedPieces>();
        app.init_resource::<CurrentTurn>();
        app.insert_resource(OnlineGameSession {
            game_id: "1".into(),
            ..Default::default()
        });
        app.init_resource::<CausalChainState>();
        app.add_message::<NetworkMoveEvent>();
        app.add_message::<MoveMadeEvent>();
        app.add_message::<RemoteMoveApplied>();
        app.add_message::<crate::game::events::NetworkMoveResult>();
        for color in [PieceColor::White, PieceColor::Black] {
            let rank = if color == PieceColor::White { 0 } else { 7 };
            let pawn_rank = if color == PieceColor::White { 1 } else { 6 };
            for (file, kind) in [
                PieceType::Rook,
                PieceType::Knight,
                PieceType::Bishop,
                PieceType::Queen,
                PieceType::King,
                PieceType::Bishop,
                PieceType::Knight,
                PieceType::Rook,
            ]
            .into_iter()
            .enumerate()
            {
                app.world_mut().spawn((
                    Piece::new(color, kind, file as u8, rank),
                    HasMoved::default(),
                ));
                app.world_mut().spawn((
                    Piece::new(color, PieceType::Pawn, file as u8, pawn_rank),
                    HasMoved::default(),
                ));
            }
        }
        app.add_systems(Update, handle_network_moves);
        app
    }

    #[test]
    fn accepted_move_produces_application_acknowledgment() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut app = game();
        app.world_mut().write_message(NetworkMoveEvent {
            from: (4, 1),
            to: (4, 3),
            promotion: None,
            expected_fen: None,
            dedup_version: Some("v1".into()),
        });
        app.update();
        let results = app
            .world()
            .resource::<Messages<crate::game::events::NetworkMoveResult>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        let result = cursor.read(results).next().ok_or("missing test value")?;
        assert_eq!(result.version.as_deref(), Some("v1"));
        assert_eq!(
            result.fen.as_deref(),
            Some(app.world().resource::<ChessEngine>().current_fen())
        );
        assert!(app.world().resource::<CausalChainState>().applied_versions[&1].contains("v1"));

        Ok(())
    }

    #[test]
    fn illegal_turn_and_position_mismatch_never_acknowledge_a_move(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for (from, to, expected_fen) in [
            ((4, 1), (4, 4), None),
            ((4, 6), (4, 4), None),
            ((4, 2), (4, 3), None),
            ((4, 1), (4, 3), Some("wrong-position".to_owned())),
        ] {
            let mut app = game();
            app.world_mut().write_message(NetworkMoveEvent {
                from,
                to,
                promotion: None,
                expected_fen,
                dedup_version: Some("rejected".into()),
            });
            app.update();
            let results = app
                .world()
                .resource::<Messages<crate::game::events::NetworkMoveResult>>();
            let mut cursor = bevy::ecs::message::MessageCursor::default();
            assert!(cursor
                .read(results)
                .next()
                .ok_or("missing test value")?
                .fen
                .is_none());
            assert!(!app
                .world()
                .resource::<CausalChainState>()
                .applied_versions
                .get(&1)
                .is_some_and(|versions| versions.contains("rejected")));
            assert!(app
                .world()
                .resource::<Messages<RemoteMoveApplied>>()
                .is_empty());
        }

        Ok(())
    }

    #[test]
    fn move_onto_own_piece_is_rejected_once_when_board_and_engine_diverge() {
        let mut app = game();
        // The engine believes e4 is empty, but a stale white entity sits there.
        app.world_mut().spawn((
            Piece::new(PieceColor::White, PieceType::Knight, 4, 3),
            HasMoved::default(),
        ));
        let fen_before = app
            .world()
            .resource::<ChessEngine>()
            .current_fen()
            .to_string();
        app.world_mut().write_message(NetworkMoveEvent {
            from: (4, 1),
            to: (4, 3),
            promotion: None,
            expected_fen: None,
            dedup_version: Some("diverged".into()),
        });
        app.update();

        let results = app
            .world()
            .resource::<Messages<crate::game::events::NetworkMoveResult>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        let all: Vec<_> = cursor.read(results).collect();
        assert_eq!(all.len(), 1, "one result per received move");
        assert!(all[0].fen.is_none());
        assert_eq!(
            app.world().resource::<ChessEngine>().current_fen(),
            fen_before,
            "a rejected move must not advance the position"
        );
        assert!(app.world().resource::<Messages<MoveMadeEvent>>().is_empty());
    }
}

#[derive(Resource, Default)]
pub struct PendingRematchOffer {
    pub from_player: Option<String>,
}

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

pub fn handle_draw_response_events(
    mut events: MessageReader<crate::game::events::DrawResponseEvent>,
    mut pending: ResMut<PendingDrawOffer>,
    mut game_over: ResMut<GameOverState>,
    #[cfg(feature = "solana")] solana_wallet: Option<
        Res<crate::multiplayer::solana::addon::SolanaWallet>,
    >,
    #[cfg(feature = "solana")] solana_sync: Option<
        Res<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
) {
    for ev in events.read() {
        if ev.accepted {
            info!(
                "[DRAW] Draw accepted by {} (remote={})",
                ev.player, ev.remote
            );
            *game_over = GameOverState::Stalemate; // Stalemate is the closest "draw" variant

            // Only the accepting client can sign the on-chain transaction;
            // the remote echo must not submit it again.
            #[cfg(feature = "solana")]
            if !ev.remote {
                if let (Some(wallet_pubkey), Some(game_id), Some(rpc_url)) = (
                    solana_wallet.as_ref().and_then(|w| w.pubkey),
                    solana_sync.as_ref().and_then(|s| s.game_id),
                    solana_sync.as_ref().map(|s| s.rpc_url.clone()),
                ) {
                    crate::multiplayer::solana::lobby::spawn_accept_draw(
                        rpc_url,
                        wallet_pubkey,
                        game_id,
                    );
                }
            }
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
