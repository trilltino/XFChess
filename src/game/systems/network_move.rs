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

            // Validate the complete UCI move. A destination alone cannot tell
            // whether a promotion is required, forbidden, or has a legal type.
            let Some(uci) = ChessEngine::move_uci(event.from, event.to, event.promotion)
                .filter(|uci| engine.is_move_legal_by_uci(uci))
            else {
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
            };

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
                        // Visual only: stop drawing a move the peer disagrees
                        // with; pieces snap to the board until the resync lands.
                        commands.queue(crate::motion::retire_board_visuals);
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

/// How long a draw prompt stays on screen. A peer that never hears the answer
/// would otherwise keep it forever. Only the prompt and button lapse: whether our
/// own offer can still be accepted depends on the position, not the clock.
pub const DRAW_OFFER_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// Plies played in the engine's current position (0 before the first move).
pub fn position_ply(engine: &ChessEngine) -> u32 {
    crate::multiplayer::systems::fen_ply(engine.current_fen())
        .and_then(|ply| u32::try_from(ply).ok())
        .unwrap_or(0)
}

/// A draw offer as the chess rules (and the on-chain program) treat it: made in a
/// position, standing through the offerer's own next move, and lapsing once the
/// opponent moves instead of accepting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Standing {
    ply: u32,
    by_white: bool,
}

impl Standing {
    fn new(ply: u32, offerer: &str) -> Self {
        Self {
            ply,
            by_white: offerer == "white",
        }
    }

    /// The first position in which the opponent has moved since the offer.
    fn lapses_at(self) -> u32 {
        // Even plies have white to move.
        let offerer_moves_next = self.ply.is_multiple_of(2) == self.by_white;
        if offerer_moves_next {
            self.ply + 2
        } else {
            self.ply + 1
        }
    }

    fn stands_at(self, ply: u32) -> bool {
        ply < self.lapses_at()
    }
}

#[derive(Resource, Default)]
pub struct PendingDrawOffer {
    /// The opponent's offer, waiting for our answer.
    pub from_player: Option<String>,
    from: Option<Standing>,
    received_at: Option<std::time::Instant>,
    /// Our own offer, waiting for their answer (shows the button as sent).
    sent_at: Option<std::time::Instant>,
    /// Our own offer. An acceptance is only honoured for it, so nobody can be
    /// drawn out of a game they did not offer to draw, and it survives the prompt
    /// timing out: an answer that reaches us late is still real.
    offered: Option<Standing>,
}

impl PendingDrawOffer {
    pub fn receive(&mut self, player: String, ply: u32) {
        // The same offer arrives over gossip and Braid; keep the later position.
        let ply = match self.from {
            Some(known) if self.from_player.is_some() => ply.max(known.ply),
            _ => ply,
        };
        self.from = Some(Standing::new(ply, &player));
        self.from_player = Some(player);
        self.received_at = Some(std::time::Instant::now());
    }

    pub fn mark_sent(&mut self, ply: u32, offerer: &str) {
        self.sent_at = Some(std::time::Instant::now());
        self.offered = Some(Standing::new(ply, offerer));
    }

    pub fn is_sent(&self) -> bool {
        self.sent_at.is_some()
    }

    pub fn is_pending(&self) -> bool {
        self.from_player.is_some() || self.sent_at.is_some()
    }

    /// Whether an acceptance made in position `ply` answers an offer of ours that
    /// still stands there.
    pub fn accepts(&self, ply: Option<u32>) -> bool {
        self.offered
            .is_some_and(|offer| ply.is_none_or(|ply| ply >= offer.ply && offer.stands_at(ply)))
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Drop what the opponent's move has overtaken. A move by the offerer leaves
    /// their offer standing for the reply.
    fn lapse_for_position(&mut self, ply: u32) {
        if self.from.is_some_and(|offer| !offer.stands_at(ply)) {
            self.from = None;
            self.from_player = None;
            self.received_at = None;
        }
        if self.offered.is_some_and(|offer| !offer.stands_at(ply)) {
            self.offered = None;
            self.sent_at = None;
        }
    }

    fn expire(&mut self, now: std::time::Instant) {
        let lapsed = |at: Option<std::time::Instant>| {
            at.is_some_and(|at| now.saturating_duration_since(at) >= DRAW_OFFER_TTL)
        };
        if lapsed(self.received_at) {
            self.from = None;
            self.from_player = None;
            self.received_at = None;
        }
        if lapsed(self.sent_at) {
            self.sent_at = None;
        }
    }
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
    fn promotion_suffixes_and_invalid_squares_are_rejected_before_mutation() {
        for (from, to, promotion) in [
            ((4, 1), (4, 3), Some('q')),
            ((6, 0), (5, 2), Some('n')),
            ((4, 1), (4, 3), Some('Q')),
            ((4, 1), (4, 3), Some('k')),
            ((4, 1), (4, 3), Some('é')),
            ((4, 1), (255, 255), None),
        ] {
            let mut app = game();
            let before = app
                .world()
                .resource::<ChessEngine>()
                .current_fen()
                .to_owned();
            app.world_mut().write_message(NetworkMoveEvent {
                from,
                to,
                promotion,
                expected_fen: Some(before.clone()),
                dedup_version: Some("invalid".into()),
            });
            app.update();
            assert_eq!(app.world().resource::<ChessEngine>().current_fen(), before);
            assert!(app.world().resource::<MoveHistory>().is_empty());
            assert!(!app.world().resource::<PendingTurnAdvance>().is_pending());
            assert!(app.world().resource::<Messages<MoveMadeEvent>>().is_empty());
            assert!(app
                .world()
                .resource::<Messages<RemoteMoveApplied>>()
                .is_empty());
            let mut results = bevy::ecs::message::MessageCursor::<
                crate::game::events::NetworkMoveResult,
            >::default();
            let results: Vec<_> = results
                .read(
                    app.world()
                        .resource::<Messages<crate::game::events::NetworkMoveResult>>(),
                )
                .collect();
            assert_eq!(results.len(), 1);
            assert!(results[0].fen.is_none());
            assert_eq!(
                app.world_mut()
                    .query::<&crate::motion::PieceMotion>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
    }

    #[test]
    fn last_rank_requires_a_legal_promotion_and_accepts_all_four_choices() -> Result<(), String> {
        let fen = "4k3/P7/8/8/8/8/8/4K3 w - - 0 17";
        for promotion in [
            None,
            Some('p'),
            Some('x'),
            Some('q'),
            Some('r'),
            Some('b'),
            Some('n'),
        ] {
            let mut app = game();
            let entities: Vec<_> = app
                .world_mut()
                .query::<(Entity, &Piece)>()
                .iter(app.world())
                .map(|(entity, _)| entity)
                .collect();
            for entity in entities {
                app.world_mut().despawn(entity);
            }
            app.world_mut()
                .resource_mut::<ChessEngine>()
                .set_from_fen(fen)?;
            for (color, kind, x, y) in [
                (PieceColor::White, PieceType::Pawn, 0, 6),
                (PieceColor::White, PieceType::King, 4, 0),
                (PieceColor::Black, PieceType::King, 4, 7),
            ] {
                app.world_mut()
                    .spawn((Piece::new(color, kind, x, y), HasMoved::default()));
            }
            app.world_mut().write_message(NetworkMoveEvent {
                from: (0, 6),
                to: (0, 7),
                promotion,
                expected_fen: None,
                dedup_version: Some("promotion".into()),
            });
            app.update();
            let valid = promotion.is_some_and(|kind| "qrbn".contains(kind));
            assert_eq!(
                app.world().resource::<MoveHistory>().len(),
                usize::from(valid)
            );
            let mut cursor = bevy::ecs::message::MessageCursor::<
                crate::game::events::NetworkMoveResult,
            >::default();
            let result = cursor
                .read(
                    app.world()
                        .resource::<Messages<crate::game::events::NetworkMoveResult>>(),
                )
                .next()
                .ok_or("missing application result")?;
            assert_eq!(result.fen.is_some(), valid);
            if !valid {
                assert_eq!(app.world().resource::<ChessEngine>().current_fen(), fen);
            }
        }
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

/// Set while a wagered game with a connected wallet is running: its draw offers and
/// acceptances are recorded on-chain first, by the chain layer, and announced after.
#[derive(Resource, Default)]
pub struct ChainDrawGate {
    pub active: bool,
}

/// Turn what the player asked for into the announced offer or answer. A declined
/// offer needs no chain record, and a game without a chain account has nothing to
/// wait for.
pub fn route_draw_requests(
    mut requests: MessageReader<crate::game::events::DrawActionRequest>,
    gate: Res<ChainDrawGate>,
    mut offers: MessageWriter<crate::game::events::DrawOfferEvent>,
    mut answers: MessageWriter<crate::game::events::DrawResponseEvent>,
) {
    use crate::game::events::DrawActionKind;
    for request in requests.read() {
        if gate.active && request.kind != DrawActionKind::Decline {
            continue; // the chain layer takes it first
        }
        match request.kind {
            DrawActionKind::Offer => {
                offers.write(crate::game::events::DrawOfferEvent {
                    player: request.player.clone(),
                    remote: false,
                    ply: None,
                });
            }
            DrawActionKind::Accept | DrawActionKind::Decline => {
                answers.write(crate::game::events::DrawResponseEvent {
                    player: request.player.clone(),
                    accepted: request.kind == DrawActionKind::Accept,
                    remote: false,
                    ply: None,
                });
            }
        }
    }
}

/// A resignation takes effect at once unless a wagered game must record it first.
pub fn route_resign_requests(
    mut requests: MessageReader<crate::game::events::ResignRequest>,
    gate: Res<ChainDrawGate>,
    mut resigns: MessageWriter<ResignEvent>,
) {
    for request in requests.read() {
        if gate.active {
            continue; // the chain layer records it first
        }
        resigns.write(ResignEvent {
            winner: request.winner.clone(),
            remote: false,
        });
    }
}

pub fn watch_draw_offers(
    mut events: MessageReader<crate::game::events::DrawOfferEvent>,
    mut pending: ResMut<PendingDrawOffer>,
    engine: Res<ChessEngine>,
) {
    for ev in events.read() {
        if ev.remote {
            info!("[DRAW] Received draw offer from {}", ev.player);
            pending.receive(
                ev.player.clone(),
                ev.ply.unwrap_or_else(|| position_ply(&engine)),
            );
        } else {
            pending.mark_sent(position_ply(&engine), &ev.player);
        }
    }
}

/// Retire offers that can no longer be answered: the position moved on (playing
/// on declines), the game ended, or the prompt timed out. Draw offers and answers
/// are durable on the Braid log, so a dropped link neither loses nor revives
/// them; only the gossip-only rematch prompt is dropped with the link.
pub fn expire_draw_offers(
    mut pending: ResMut<PendingDrawOffer>,
    mut rematch: ResMut<PendingRematchOffer>,
    game_over: Res<GameOverState>,
    engine: Res<ChessEngine>,
    p2p: Option<Res<crate::multiplayer::network::p2p::P2PConnectionState>>,
    mut last_link: Local<Option<crate::multiplayer::network::p2p::P2PConnectionStatus>>,
) {
    use crate::multiplayer::network::p2p::P2PConnectionStatus;

    if game_over.is_game_over() {
        pending.clear();
    } else if pending.is_pending() || pending.offered.is_some() {
        pending.lapse_for_position(position_ply(&engine));
    }

    if let Some(p2p) = p2p {
        let broken = |s: &P2PConnectionStatus| {
            matches!(
                s,
                P2PConnectionStatus::Disconnected | P2PConnectionStatus::Error(_)
            )
        };
        let previous = last_link.replace(p2p.status.clone());
        if previous.as_ref().is_some_and(|prev| *prev != p2p.status)
            && (broken(&p2p.status) || previous.as_ref().is_some_and(broken))
        {
            rematch.from_player = None;
        }
    }

    pending.expire(std::time::Instant::now());
}

pub fn handle_draw_response_events(
    mut events: MessageReader<crate::game::events::DrawResponseEvent>,
    mut pending: ResMut<PendingDrawOffer>,
    mut game_over: ResMut<GameOverState>,
) {
    for ev in events.read() {
        // The opponent can only accept a draw we offered, in the position we offered it.
        if ev.accepted && ev.remote && !pending.accepts(ev.ply) {
            warn!(
                "[DRAW] Ignored acceptance from {} (position {:?}): no matching offer of ours",
                ev.player, ev.ply
            );
            continue;
        }
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
        pending.clear();
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

#[cfg(test)]
mod draw_offer_tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_received_offer_lapses_after_the_ttl() {
        let mut pending = PendingDrawOffer::default();
        pending.receive("white".into(), 10);
        pending.expire(Instant::now() + DRAW_OFFER_TTL - Duration::from_secs(1));
        assert!(pending.from_player.is_some());
        pending.expire(Instant::now() + DRAW_OFFER_TTL + Duration::from_secs(1));
        assert!(pending.from_player.is_none());
    }

    #[test]
    fn a_sent_offer_lapses_and_frees_the_button() {
        let mut pending = PendingDrawOffer::default();
        pending.mark_sent(10, "white");
        assert!(pending.is_sent() && pending.is_pending());
        pending.expire(Instant::now() + DRAW_OFFER_TTL + Duration::from_secs(1));
        assert!(!pending.is_pending());
    }

    #[test]
    fn clear_drops_both_directions() {
        let mut pending = PendingDrawOffer::default();
        pending.receive("black".into(), 10);
        pending.mark_sent(10, "black");
        pending.clear();
        assert!(!pending.is_pending());
        assert!(!pending.accepts(None));
    }

    #[test]
    fn an_offer_made_on_the_offerers_turn_stands_through_their_move() {
        // White to move at ply 10 offers, then moves: black's reply may still accept.
        let mut pending = PendingDrawOffer::default();
        pending.receive("white".into(), 10);
        pending.lapse_for_position(10);
        pending.lapse_for_position(11);
        assert!(
            pending.from_player.is_some(),
            "the offerer's own move keeps it"
        );
        pending.lapse_for_position(12);
        assert!(
            pending.from_player.is_none(),
            "black moved instead of accepting"
        );
    }

    #[test]
    fn an_offer_made_on_the_opponents_turn_lapses_when_they_move() {
        // Black to move at ply 11; white offers and black plays on.
        let mut pending = PendingDrawOffer::default();
        pending.receive("white".into(), 11);
        pending.lapse_for_position(11);
        assert!(pending.from_player.is_some());
        pending.lapse_for_position(12);
        assert!(pending.from_player.is_none());
    }

    #[test]
    fn a_board_still_catching_up_does_not_cancel_an_offer() {
        // The offer can arrive before the move it follows has been applied.
        let mut pending = PendingDrawOffer::default();
        pending.receive("white".into(), 10);
        pending.lapse_for_position(9);
        assert!(pending.from_player.is_some());
    }

    #[test]
    fn the_same_offer_over_two_transports_keeps_the_later_position() {
        let mut pending = PendingDrawOffer::default();
        pending.receive("white".into(), 9); // gossip, ahead of the board
        pending.receive("white".into(), 10); // Braid, with the real position
        pending.lapse_for_position(11);
        assert!(pending.from_player.is_some());
    }

    #[test]
    fn only_an_offer_of_ours_that_still_stands_can_be_accepted() {
        let mut pending = PendingDrawOffer::default();
        assert!(!pending.accepts(Some(10)), "nothing was offered");
        pending.mark_sent(10, "white");
        assert!(pending.accepts(Some(10)));
        assert!(pending.accepts(Some(11)), "after our own move");
        assert!(pending.accepts(None), "gossip does not say which position");
        assert!(!pending.accepts(Some(8)), "before it was made");
        assert!(!pending.accepts(Some(12)), "after the opponent moved on");
        // The prompt timing out must not make a late acceptance unreal.
        pending.expire(Instant::now() + DRAW_OFFER_TTL + Duration::from_secs(1));
        assert!(!pending.is_sent());
        assert!(pending.accepts(Some(11)));
        // The opponent's move withdraws the offer.
        pending.lapse_for_position(12);
        assert!(!pending.accepts(Some(11)));
    }
}

#[cfg(test)]
mod draw_acceptance_tests {
    use super::*;
    use crate::game::events::{DrawOfferEvent, DrawResponseEvent};

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<PendingDrawOffer>();
        app.init_resource::<GameOverState>();
        app.add_message::<DrawResponseEvent>();
        app.add_message::<DrawOfferEvent>();
        app.add_systems(Update, handle_draw_response_events);
        app
    }

    fn accept(app: &mut App, ply: Option<u32>) {
        app.world_mut().write_message(DrawResponseEvent {
            player: "black".into(),
            accepted: true,
            remote: true,
            ply,
        });
        app.update();
    }

    #[test]
    fn an_acceptance_of_an_offer_nobody_made_does_not_end_the_game() {
        let mut app = app();
        accept(&mut app, Some(10));
        assert_eq!(
            *app.world().resource::<GameOverState>(),
            GameOverState::Playing
        );
    }

    #[test]
    fn an_acceptance_in_another_position_does_not_end_the_game() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<PendingDrawOffer>()
            .mark_sent(10, "white");
        accept(&mut app, Some(8));
        assert_eq!(
            *app.world().resource::<GameOverState>(),
            GameOverState::Playing
        );
    }

    #[test]
    fn an_acceptance_of_our_offer_ends_the_game_even_after_a_long_offline_spell() {
        let mut app = app();
        {
            let mut pending = app.world_mut().resource_mut::<PendingDrawOffer>();
            pending.mark_sent(10, "white");
            // The prompt timed out while we were away; the offer itself is still ours.
            pending.expire(std::time::Instant::now() + DRAW_OFFER_TTL * 10);
        }
        accept(&mut app, Some(10));
        assert_eq!(
            *app.world().resource::<GameOverState>(),
            GameOverState::Stalemate
        );
        assert!(!app.world().resource::<PendingDrawOffer>().is_pending());
    }
}
