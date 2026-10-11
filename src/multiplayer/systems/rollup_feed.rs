use super::*;

#[cfg(feature = "solana")]
pub fn feed_local_moves_to_rollup(
    mut move_events: MessageReader<MoveMadeEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
) {
    // game_id != 0 identifies active on-chain games; VPS lobby games do not
    // set the direct-invite active_session field.
    if rollup_manager.game_id == 0 {
        return;
    }
    // Only the creator submits rollup moves. Two writers would assign overlapping
    // nonces; the joiner receives committed baselines independently.
    if !rollup_manager.is_creator {
        return;
    }

    for event in move_events.read() {
        if event.remote {
            continue;
        }

        let from_col = (b'a' + event.from.0) as char;
        let from_row = event.from.1 + 1;
        let to_col = (b'a' + event.to.0) as char;
        let to_row = event.to.1 + 1;

        let mut uci = format!("{}{}{}{}", from_col, from_row, to_col, to_row);

        if let Some(promo) = event.promotion {
            let promo_char = match promo {
                PieceType::Queen => 'q',
                PieceType::Rook => 'r',
                PieceType::Bishop => 'b',
                PieceType::Knight => 'n',
                _ => 'q',
            };
            uci.push(promo_char);
        }

        rollup_manager.add_local_move(uci, event.next_fen.clone());
    }
}

#[cfg(feature = "solana")]
pub fn feed_remote_moves_to_rollup(
    mut remote_events: MessageReader<crate::game::events::RemoteMoveApplied>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
) {
    if rollup_manager.game_id == 0 {
        return;
    }
    if !rollup_manager.is_creator {
        return;
    }

    for event in remote_events.read() {
        rollup_manager.add_local_move(event.uci.clone(), event.next_fen.clone());
    }
}

#[cfg(feature = "solana")]
pub fn handle_session_info_from_network(
    mut network_events: MessageReader<NetworkEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut session_key_manager: ResMut<
        crate::multiplayer::rollup::session_keys::HandshakeOrderingKeyManager,
    >,
    causal: Res<crate::multiplayer::types::CausalChainState>,
    mut solana_state: Option<
        ResMut<crate::multiplayer::solana::integration::state::SolanaIntegrationState>,
    >,
) {
    for event in network_events.read() {
        if let NetworkEvent::MessageReceived(NetworkMessage::SessionInfo {
            game_id,
            player_pubkey,
            session_pubkey,
            ..
        }) = event
        {
            if game_id != &rollup_manager.game_id {
                continue;
            }

            let claim_trusted = match causal.verified_wallets.get(game_id) {
                Some((white, black)) => {
                    let claimed = player_pubkey.to_string();
                    claimed == *white || claimed == *black
                }
                None => true,
            };

            if !claim_trusted {
                warn!(
                    "[SESSION] Ignored spoofed SessionInfo for game {}: claimed player_pubkey {} is not in verified wallet pair {:?}",
                    game_id,
                    player_pubkey,
                    causal.verified_wallets.get(game_id)
                );
                continue;
            }

            if let Some(ref mut state) = solana_state {
                state.opponent_pubkey = Some(*player_pubkey);
            }
            session_key_manager.set_game_id(*game_id);

            if let Some(our_pubkey) = session_key_manager.get_session_pubkey() {
                use solana_sdk::pubkey::Pubkey;
                let peer_key: Pubkey = *session_pubkey;
                let our_key: Pubkey = our_pubkey;
                let (white_key, black_key) = if our_key.to_string() < peer_key.to_string() {
                    (our_key, peer_key)
                } else {
                    (peer_key, our_key)
                };
                rollup_manager.set_session_keys(white_key, black_key);
            }
        }
    }
}

#[cfg(feature = "solana")]
pub fn finalize_game_on_end(
    mut game_end_events: MessageReader<GameEndedEvent>,
    mut rollup_manager: ResMut<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    mut rollup_events: MessageWriter<crate::multiplayer::rollup::manager::RollupEvent>,
) {
    // Only the host submits the final move batch; serializing two writers
    // would still permit duplicate submissions.
    for _event in game_end_events.read() {
        if !rollup_manager.is_creator {
            continue;
        }
        if rollup_manager.game_id == 0 {
            continue;
        }
        if let Some((moves, next_fens)) = rollup_manager.force_flush() {
            rollup_events.write(
                crate::multiplayer::rollup::manager::RollupEvent::GameEndBatch {
                    game_id: rollup_manager.game_id,
                    moves,
                    next_fens,
                },
            );
        }
    }
}

#[cfg(feature = "solana")]
pub fn emit_game_ended_event(
    game_over: Res<GameOverState>,
    mut game_ended_events: MessageWriter<GameEndedEvent>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut emitted: Local<bool>,
) {
    if !game_over.is_game_over() {
        *emitted = false;
        return;
    }
    if *emitted {
        return;
    }
    let game_id = match competitive.as_ref().and_then(|c| c.game_id) {
        Some(id) => id,
        None => return,
    };
    *emitted = true;

    let (winner, reason) = match *game_over {
        GameOverState::WhiteWon => (Some("white".to_string()), "checkmate"),
        GameOverState::WhiteWonByTime => (Some("white".to_string()), "timeout"),
        GameOverState::WhiteWonByResignation => (Some("white".to_string()), "resignation"),
        GameOverState::BlackWon => (Some("black".to_string()), "checkmate"),
        GameOverState::BlackWonByTime => (Some("black".to_string()), "timeout"),
        GameOverState::BlackWonByResignation => (Some("black".to_string()), "resignation"),
        GameOverState::WhiteWonByAbandonment => (Some("white".to_string()), "abandonment"),
        GameOverState::BlackWonByAbandonment => (Some("black".to_string()), "abandonment"),
        GameOverState::Stalemate => (None, "stalemate"),
        GameOverState::InsufficientMaterial => (None, "insufficient_material"),
        GameOverState::Aborted => (None, "aborted"),
        GameOverState::Playing => return,
    };

    game_ended_events.write(GameEndedEvent {
        game_id,
        winner,
        reason: reason.to_string(),
    });
}

#[cfg(all(test, feature = "solana"))]
mod session_info_spoof_tests {
    use super::*;
    use crate::multiplayer::rollup::manager::EphemeralRollupManager;
    use crate::multiplayer::rollup::session_keys::HandshakeOrderingKeyManager;
    use crate::multiplayer::solana::integration::state::SolanaIntegrationState;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn handle_session_info_rejects_a_spoofed_player_pubkey_when_verified_wallets_are_known() {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let victim_wallet = Pubkey::new_unique(); // not actually white or black
        let attacker_signing_key = Pubkey::new_unique();

        let mut app = App::new();
        let mut rollup = EphemeralRollupManager::new(1, false, "startpos".to_string());
        rollup.game_id = 1;
        app.insert_resource(rollup);
        app.insert_resource(HandshakeOrderingKeyManager::default());
        app.insert_resource(SolanaIntegrationState::default());
        app.insert_resource(CausalChainState::default());
        app.world_mut()
            .resource_mut::<CausalChainState>()
            .verified_wallets
            .insert(1, (white.to_string(), black.to_string()));
        app.add_message::<NetworkEvent>();
        app.add_systems(Update, handle_session_info_from_network);

        app.world_mut()
            .write_message(NetworkEvent::MessageReceived(NetworkMessage::SessionInfo {
                game_id: 1,
                player_pubkey: victim_wallet,
                session_pubkey: Pubkey::new_unique(),
                signing_pubkey: attacker_signing_key,
                expires_at: i64::MAX,
            }));
        app.update();

        let state = app.world().resource::<SolanaIntegrationState>();
        assert_eq!(
            state.opponent_pubkey, None,
            "forged SessionInfo claims must not set opponent_pubkey once the wallet pair is known"
        );
    }
}
