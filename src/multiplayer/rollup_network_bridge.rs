#![cfg(feature = "solana")]
use bevy::prelude::*;
use bevy::ecs::event::{EventReader, EventWriter};
use solana_sdk::{message::Message, pubkey::Pubkey, signature::Signer};

use crate::multiplayer::{
    network_protocol::{calculate_batch_hash, NetworkMessage},
    rollup_manager::{EphemeralRollupManager, GameStateStatus, RollupEvent}, // GameStateStatus should have PartialEq
    session_key_manager::SessionKeyManager,
    BraidNetworkState,
    NetworkEvent,
};
use crate::solana::constants::SOLANA_PROGRAM_ID;
use crate::solana::instructions::commit_move_batch_ix;

#[derive(Resource, Default)]
pub struct RollupNetworkBridge {
    awaiting_commit_confirmation: bool,
    last_sent_batch_hash: Option<String>,
    pending_batches: std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
    incoming_tx_message: Option<(u64, Vec<u8>)>,
    incoming_signatures: std::collections::HashMap<Pubkey, Vec<u8>>,
}

pub struct RollupNetworkBridgePlugin;

impl Plugin for RollupNetworkBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RollupNetworkBridge>();
        app.add_systems(Update, handle_rollup_to_network_events);
        app.add_systems(Update, handle_network_to_rollup_events);
        app.add_systems(Update, process_batch_commit_requests);
    }
}

fn send_network_msg(state: &BraidNetworkState, msg: NetworkMessage) {
    if let Some(tx) = &state.message_sender {
        if let Err(e) = tx.send(msg) {
            warn!("Failed to send NetworkMessage: {}", e);
        }
    }
}

fn handle_rollup_to_network_events(
    mut rollup_events: EventReader<RollupEvent>,
    network_state: Res<BraidNetworkState>,
    mut bridge: ResMut<RollupNetworkBridge>,
    rollup_manager: Res<EphemeralRollupManager>,
) {
    for event in rollup_events.read() {
        match event {
            RollupEvent::BatchReady {
                game_id,
                moves,
                next_fens,
            } => {
                let batch_hash = calculate_batch_hash(
                    game_id,
                    rollup_manager.committed_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                );
                send_network_msg(
                    &network_state,
                    NetworkMessage::BatchPropose {
                        game_id,
                        start_turn: rollup_manager.committed_turn,
                        moves: moves.clone(),
                        next_fens: next_fens.clone(),
                    },
                );
                bridge
                    .pending_batches
                    .insert(batch_hash.clone(), (moves.clone(), next_fens.clone()));
                bridge.last_sent_batch_hash = Some(batch_hash);
                bridge.awaiting_commit_confirmation = true;
                info!("Sent BatchPropose for game {}", game_id);
            }
            RollupEvent::BatchFailed { game_id, .. } | RollupEvent::NeedResync { game_id } => {
                send_network_msg(
                    &network_state,
                    NetworkMessage::ResyncRequest { game_id },
                );
                warn!("Requested resync for game {}", game_id);
            }
            _ => {}
        }
    }
}

fn handle_network_to_rollup_events(
    mut network_events: EventReader<NetworkEvent>,
    network_state: Res<BraidNetworkState>,
    mut rollup_events: EventWriter<RollupEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    session_key_manager: Res<SessionKeyManager>,
) {
    for event in network_events.read() {
        let msg = match event {
            NetworkEvent::MessageReceived(m) => m,
            _ => continue,
        };

        match msg {
            NetworkMessage::BatchPropose {
                game_id,
                start_turn,
                moves,
                next_fens,
            } => {
                if !validate_batch_proposal(start_turn, moves.as_slice(), next_fens.as_slice(), &rollup_manager) {
                    warn!("Rejected invalid BatchPropose for game {}", game_id);
                    rollup_events.send(RollupEvent::BatchFailed {
                        game_id,
                        moves: moves.clone(),
                        next_fens: next_fens.clone(),
                    });
                    continue;
                }

                for (uci, fen) in moves.iter().zip(next_fens.iter()) {
                    rollup_manager.add_remote_move(uci.clone(), fen.clone());
                }

                let batch_hash = calculate_batch_hash(game_id, start_turn, moves.as_slice(), next_fens.as_slice());
                send_network_msg(
                    &network_state,
                    NetworkMessage::BatchAccept {
                        game_id,
                        batch_hash,
                    },
                );

                initiate_two_party_signing(
                    game_id,
                    moves.clone(),
                    next_fens.clone(),
                    &network_state,
                    &rollup_manager,
                    &session_key_manager,
                );
            }

            NetworkMessage::BatchAccept {
                game_id,
                batch_hash,
            } => {
                info!(
                    "Peer accepted batch for game {}, hash: {}",
                    game_id, batch_hash
                );
            }

            NetworkMessage::BatchReject { game_id, reason } => {
                warn!("Peer rejected batch for game {}: {}", game_id, reason);
                send_network_msg(
                    &network_state,
                    NetworkMessage::ResyncRequest { game_id },
                );
            }

            NetworkMessage::TxMessage {
                game_id,
                message_bytes,
            } => {
                bridge.incoming_tx_message = Some((game_id, message_bytes.clone()));
                if let Some(kp) = session_key_manager.get_session_keypair() {
                    let sig = kp.sign_message(message_bytes.as_slice());
                    send_network_msg(
                        &network_state,
                        NetworkMessage::TxSignature {
                            game_id,
                            signer_pubkey: kp.pubkey(),
                            signature_bytes: sig.as_ref().to_vec(),
                        },
                    );
                }
            }

            NetworkMessage::TxSignature {
                game_id,
                signer_pubkey,
                signature_bytes,
            } => {
                bridge
                    .incoming_signatures
                    .insert(signer_pubkey, signature_bytes.clone());

                if let Some((msg_gid, _)) = &bridge.incoming_tx_message {
                    if *msg_gid == game_id && bridge.incoming_signatures.len() >= 2 {
                        info!(
                            "All signatures received for game {} — ready to submit",
                            game_id
                        );
                    }
                }
            }

            NetworkMessage::Committed {
                game_id,
                tx_sig,
                new_fen,
                new_turn,
            } => {
                if game_id == rollup_manager.game_id {
                    rollup_manager.committed_fen = new_fen.clone();
                    rollup_manager.committed_turn = new_turn;
                    rollup_manager.status = GameStateStatus::Synced;
                    info!("Batch committed on-chain, tx: {}", tx_sig);
                    rollup_events.send(RollupEvent::BatchCommitted {
                        game_id,
                        new_fen: new_fen.clone(),
                        new_turn,
                    });
                }
            }

            NetworkMessage::ResyncRequest { game_id } => {
                if game_id == rollup_manager.game_id {
                    send_network_msg(
                        &network_state,
                        NetworkMessage::ResyncResponse {
                            game_id,
                            committed_fen: rollup_manager.committed_fen.clone(),
                            committed_turn: rollup_manager.committed_turn,
                        },
                    );
                }
            }

            NetworkMessage::ResyncResponse {
                game_id,
                committed_fen,
                committed_turn,
            } => {
                if game_id == rollup_manager.game_id {
                    rollup_manager.committed_fen = committed_fen.clone();
                    rollup_manager.committed_turn = committed_turn;
                    rollup_manager.status = GameStateStatus::Synced;
                    info!(
                        "Resynced game {} from peer, turn {}",
                        game_id, committed_turn
                    );
                }
            }

            NetworkMessage::Move {
                game_id,
                move_uci,
                next_fen,
                ..
            } => {
                if game_id == rollup_manager.game_id {
                    rollup_manager.add_remote_move(move_uci.clone(), next_fen.clone());
                }
            }

            _ => {}
        }
    }
}

fn process_batch_commit_requests(
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut rollup_events: EventWriter<RollupEvent>,
    network_state: Res<BraidNetworkState>,
    mut bridge: ResMut<RollupNetworkBridge>,
    session_key_manager: Res<SessionKeyManager>,
) {
    if bridge.awaiting_commit_confirmation {
        return;
    }
    if rollup_manager.status != GameStateStatus::Pending || !rollup_manager.should_flush() {
        return;
    }
    if let Some((moves, next_fens)) = rollup_manager.prepare_batch_for_commit() {
        initiate_two_party_signing(
            rollup_manager.game_id,
            moves,
            next_fens,
            &network_state,
            &rollup_manager,
            &session_key_manager,
        );
        bridge.awaiting_commit_confirmation = true;
    }
}

fn validate_batch_proposal(
    start_turn: u16,
    moves: &[String],
    next_fens: &[String],
    rollup_manager: &EphemeralRollupManager,
) -> bool {
    if start_turn != rollup_manager.committed_turn {
        warn!(
            "Batch start_turn {} != committed_turn {}",
            start_turn, rollup_manager.committed_turn
        );
        return false;
    }
    !moves.is_empty()
        && moves.len() == next_fens.len()
        && moves.len() <= rollup_manager.max_batch_size
}

fn initiate_two_party_signing(
    game_id: u64,
    moves: Vec<String>,
    next_fens: Vec<String>,
    network_state: &BraidNetworkState,
    rollup_manager: &EphemeralRollupManager,
    session_key_manager: &SessionKeyManager,
) {
    let session_kp = match session_key_manager.get_session_keypair() {
        Some(kp) => kp,
        None => {
            error!("No session keypair for game {}", game_id);
            return;
        }
    };
    let (white_session, black_session) = match rollup_manager.session_keys {
        Some(keys) => keys,
        None => {
            error!(
                "Session keys not set in rollup_manager for game {}",
                game_id
            );
            return;
        }
    };

    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    
    // Derive game_pda from game_id using the same seeds as the program
    let game_pda = Pubkey::find_program_address(
        &[b"game", &game_id.to_le_bytes()],
        &program_id,
    ).0;
    
    // Convert moves from Vec<String> to Vec<(u8, u8)> - simplified conversion
    // This is a placeholder - actual implementation would parse UCI notation
    let moves_converted: Vec<(u8, u8)> = moves.iter()
        .map(|m| {
            // Simple placeholder: convert first two chars to bytes
            let bytes = m.as_bytes();
            if bytes.len() >= 2 {
                (bytes[0], bytes[1])
            } else {
                (0u8, 0u8)
            }
        })
        .collect();
    
    let ix = commit_move_batch_ix(
        session_kp.pubkey(), // payer
        game_pda,
        moves_converted,
        vec![], // signatures - empty for now
    ).expect("Failed to create commit_move_batch instruction");

    let message = Message::new(&[ix], Some(&session_kp.pubkey()));
    let message_bytes = message.serialize();
    let sig = session_kp.sign_message(&message_bytes);

    send_network_msg(
        network_state,
        NetworkMessage::TxMessage {
            game_id,
            message_bytes: message_bytes.clone(),
        },
    );
    send_network_msg(
        network_state,
        NetworkMessage::TxSignature {
            game_id,
            signer_pubkey: session_kp.pubkey(),
            signature_bytes: sig.as_ref().to_vec(),
        },
    );
    info!("Sent TxMessage + TxSignature for game {}", game_id);
}
