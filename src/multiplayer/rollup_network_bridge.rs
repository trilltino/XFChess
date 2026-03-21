// Rollup Network Bridge for MagicBlock ER
use bevy::prelude::*;
use solana_sdk::{message::Message, pubkey::Pubkey, signature::Signer};

use crate::game::events::{GameEndedEvent, GameStartedEvent};
use crate::multiplayer::{
    magicblock_resolver::{MagicBlockEvent, MagicBlockResolver},
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

        // Core network bridge systems
        app.add_systems(Update, handle_rollup_to_network_events);
        app.add_systems(Update, handle_network_to_rollup_events);
        app.add_systems(Update, process_batch_commit_requests);

        // Magic Block ER delegation systems
        app.add_systems(Update, handle_game_start_delegation);
        app.add_systems(Update, handle_game_end_undelegation);
        app.add_systems(Update, handle_magic_block_events);

        info!("RollupNetworkBridgePlugin initialized with Magic Block ER support");
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
    mut rollup_events: MessageReader<RollupEvent>,
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
                    *game_id,
                    rollup_manager.committed_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                );
                send_network_msg(
                    &network_state,
                    NetworkMessage::BatchPropose {
                        game_id: *game_id,
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
                    NetworkMessage::ResyncRequest { game_id: *game_id },
                );
                warn!("Requested resync for game {}", game_id);
            }
            _ => {}
        }
    }
}

fn handle_network_to_rollup_events(
    mut network_events: MessageReader<NetworkEvent>,
    network_state: Res<BraidNetworkState>,
    mut rollup_events: MessageWriter<RollupEvent>,
    mut bridge: ResMut<RollupNetworkBridge>,
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    session_key_manager: Res<SessionKeyManager>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
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
                if !validate_batch_proposal(
                    *start_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                    &rollup_manager,
                ) {
                    warn!("Rejected invalid BatchPropose for game {}", game_id);
                    rollup_events.write(RollupEvent::BatchFailed {
                        game_id: *game_id,
                        moves: moves.clone(),
                        next_fens: next_fens.clone(),
                    });
                    continue;
                }

                for (uci, fen) in moves.iter().zip(next_fens.iter()) {
                    rollup_manager.add_remote_move(uci.clone(), fen.clone());
                }

                let batch_hash = calculate_batch_hash(
                    *game_id,
                    *start_turn,
                    moves.as_slice(),
                    next_fens.as_slice(),
                );
                send_network_msg(
                    &network_state,
                    NetworkMessage::BatchAccept {
                        game_id: *game_id,
                        batch_hash,
                    },
                );

                initiate_two_party_signing(
                    *game_id,
                    moves.clone(),
                    next_fens.clone(),
                    &network_state,
                    &rollup_manager,
                    &session_key_manager,
                    &mut magicblock_resolver,
                    Some(&mut magicblock_events),
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
                    NetworkMessage::ResyncRequest { game_id: *game_id },
                );
            }

            NetworkMessage::TxMessage {
                game_id,
                message_bytes,
            } => {
                bridge.incoming_tx_message = Some((*game_id, message_bytes.clone()));
                if let Some(kp) = session_key_manager.get_session_keypair() {
                    let sig = kp.sign_message(message_bytes.as_slice());
                    send_network_msg(
                        &network_state,
                        NetworkMessage::TxSignature {
                            game_id: *game_id,
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
                    .insert(*signer_pubkey, signature_bytes.clone());

                if let Some((msg_gid, _)) = &bridge.incoming_tx_message {
                    if *msg_gid == *game_id && bridge.incoming_signatures.len() >= 2 {
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
                if *game_id == rollup_manager.game_id {
                    rollup_manager.committed_fen = new_fen.clone();
                    rollup_manager.committed_turn = *new_turn;
                    rollup_manager.status = GameStateStatus::Synced;
                    info!("Batch committed on-chain, tx: {}", tx_sig);
                    rollup_events.write(RollupEvent::BatchCommitted {
                        game_id: *game_id,
                        new_fen: new_fen.clone(),
                        new_turn: *new_turn,
                    });
                }
            }

            NetworkMessage::ResyncRequest { game_id } => {
                if *game_id == rollup_manager.game_id {
                    send_network_msg(
                        &network_state,
                        NetworkMessage::ResyncResponse {
                            game_id: *game_id,
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
                if *game_id == rollup_manager.game_id {
                    rollup_manager.committed_fen = committed_fen.clone();
                    rollup_manager.committed_turn = *committed_turn;
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
                if *game_id == rollup_manager.game_id {
                    rollup_manager.add_remote_move(move_uci.clone(), next_fen.clone());
                }
            }

            _ => {}
        }
    }
}

fn process_batch_commit_requests(
    mut rollup_manager: ResMut<EphemeralRollupManager>,
    mut _rollup_events: MessageWriter<RollupEvent>,
    network_state: Res<BraidNetworkState>,
    mut bridge: ResMut<RollupNetworkBridge>,
    session_key_manager: Res<SessionKeyManager>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
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
            &mut magicblock_resolver,
            Some(&mut magicblock_events),
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
    magicblock_resolver: &mut MagicBlockResolver,
    mut magicblock_events: Option<&mut MessageWriter<MagicBlockEvent>>,
) {
    let session_kp = match session_key_manager.get_session_keypair() {
        Some(kp) => kp,
        None => {
            error!("No session keypair for game {}", game_id);
            return;
        }
    };
    let (_white_session, _black_session) = match rollup_manager.session_keys {
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
    let game_pda = Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

    // Convert moves from UCI notation to compact (from, to) format
    // UCI format: "e2e4" (from e2 to e4), "e7e8q" (promotion to queen)
    let moves_converted: Vec<(u8, u8)> = moves
        .iter()
        .filter_map(|uci| uci_to_square_indices(uci))
        .collect();

    if moves_converted.is_empty() && !moves.is_empty() {
        error!("Failed to convert any moves from UCI format");
        return;
    }

    let ix = commit_move_batch_ix(
        session_kp.pubkey(), // payer
        game_pda,
        moves_converted,
        vec![], // signatures - empty for now
    )
    .expect("Failed to create commit_move_batch instruction");

    // Check if we should route through Magic Block ER
    if magicblock_resolver.is_delegated() {
        info!(
            "Routing batch commit through Magic Block ER for game {}",
            game_id
        );

        match magicblock_resolver.route_transaction(vec![ix.clone()], &session_kp) {
            Ok(signature) => {
                info!("Batch commit routed to ER with signature: {}", signature);
                if let Some(ref mut events) = magicblock_events {
                    events.write(MagicBlockEvent::TransactionRoutedToEr { signature });
                }
            }
            Err(e) => {
                error!("Failed to route batch commit to ER: {}", e);
                // Fall back to network-based signing
                send_network_batch_commit(network_state, game_id, &session_kp, vec![ix]);
            }
        }
    } else {
        // Use traditional network-based signing
        send_network_batch_commit(network_state, game_id, &session_kp, vec![ix]);
    }
}

/// Helper function to send batch commit via network (traditional approach)
fn send_network_batch_commit(
    network_state: &BraidNetworkState,
    game_id: u64,
    session_kp: &solana_sdk::signature::Keypair,
    instructions: Vec<solana_sdk::instruction::Instruction>,
) {
    let message = Message::new(&instructions, Some(&session_kp.pubkey()));
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

/// Handles game start events to delegate the game PDA to the Ephemeral Rollup
///
/// This system listens for GameStartedEvent and triggers delegation to the ER
/// for sub-second transaction processing during gameplay.
fn handle_game_start_delegation(
    mut game_started_events: MessageReader<GameStartedEvent>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    session_key_manager: Res<SessionKeyManager>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    for event in game_started_events.read() {
        let game_id = event.game_id;
        info!("Game {} started - initiating ER delegation", game_id);

        // Derive the game PDA
        let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
        let game_pda =
            Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &program_id).0;

        // Get session keypair for signing
        let session_keypair = match session_key_manager.get_session_keypair() {
            Some(kp) => kp,
            None => {
                error!("No session keypair available for delegation");
                magicblock_events.write(MagicBlockEvent::DelegationFailed {
                    game_pda,
                    error: "No session keypair available".to_string(),
                });
                continue;
            }
        };

        // Delegate game to ER
        match magicblock_resolver.delegate_game(game_pda, &session_keypair) {
            Ok(_) => {
                info!("Successfully delegated game {} to ER", game_id);
                magicblock_events.write(MagicBlockEvent::GameDelegated { game_pda });
            }
            Err(e) => {
                error!("Failed to delegate game {} to ER: {}", game_id, e);
                magicblock_events.write(MagicBlockEvent::DelegationFailed {
                    game_pda,
                    error: e.to_string(),
                });
            }
        }
    }
}

/// Handles game end events to undelegate the game PDA from the Ephemeral Rollup
///
/// This system listens for GameEndedEvent and triggers undelegation from the ER,
/// committing the final game state to Solana.
fn handle_game_end_undelegation(
    mut game_ended_events: MessageReader<GameEndedEvent>,
    mut magicblock_resolver: ResMut<MagicBlockResolver>,
    session_key_manager: Res<SessionKeyManager>,
    mut magicblock_events: MessageWriter<MagicBlockEvent>,
) {
    for event in game_ended_events.read() {
        let game_id = event.game_id;
        info!("Game {} ended - initiating ER undelegation", game_id);

        // Check if game is currently delegated
        if !magicblock_resolver.is_delegated() {
            info!(
                "Game {} was not delegated to ER, skipping undelegation",
                game_id
            );
            continue;
        }

        // Get session keypair for signing
        let session_keypair = match session_key_manager.get_session_keypair() {
            Some(kp) => kp,
            None => {
                error!("No session keypair available for undelegation");
                let game_pda = magicblock_resolver.get_delegated_game().unwrap_or_default();
                magicblock_events.write(MagicBlockEvent::UndelegationFailed {
                    game_pda,
                    error: "No session keypair available".to_string(),
                });
                continue;
            }
        };

        // Undelegate game from ER
        match magicblock_resolver.undelegate_game(&session_keypair) {
            Ok(_) => {
                let game_pda = magicblock_resolver.get_delegated_game().unwrap_or_default();
                info!("Successfully undelegated game {} from ER", game_id);
                magicblock_events.write(MagicBlockEvent::GameUndelegated { game_pda });
            }
            Err(e) => {
                let game_pda = magicblock_resolver.get_delegated_game().unwrap_or_default();
                error!("Failed to undelegate game {} from ER: {}", game_id, e);
                magicblock_events.write(MagicBlockEvent::UndelegationFailed {
                    game_pda,
                    error: e.to_string(),
                });
            }
        }
    }
}

/// Handles Magic Block events for logging and error handling
fn handle_magic_block_events(mut magicblock_events: MessageReader<MagicBlockEvent>) {
    for event in magicblock_events.read() {
        match event {
            MagicBlockEvent::GameDelegated { game_pda } => {
                info!("Magic Block: Game {} delegated to ER", game_pda);
            }
            MagicBlockEvent::GameUndelegated { game_pda } => {
                info!("Magic Block: Game {} undelegated from ER", game_pda);
            }
            MagicBlockEvent::DelegationFailed { game_pda, error } => {
                error!(
                    "Magic Block: Failed to delegate game {}: {}",
                    game_pda, error
                );
            }
            MagicBlockEvent::UndelegationFailed { game_pda, error } => {
                error!(
                    "Magic Block: Failed to undelegate game {}: {}",
                    game_pda, error
                );
            }
            MagicBlockEvent::TransactionRoutedToEr { signature } => {
                info!("Magic Block: Transaction routed to ER: {}", signature);
            }
        }
    }
}

/// Convert UCI notation (e.g., "e2e4", "e7e8q") to square indices (0-63)
///
/// UCI format:
/// - First 2 chars: from square (file a-h, rank 1-8)
/// - Next 2 chars: to square (file a-h, rank 1-8)
/// - Optional 5th char: promotion piece (q, r, b, n)
///
/// Returns: (from_index, to_index) where index is 0-63
/// (0 = a1, 7 = h1, 56 = a8, 63 = h8)
fn uci_to_square_indices(uci: &str) -> Option<(u8, u8)> {
    if uci.len() < 4 {
        warn!("Invalid UCI move format (too short): {}", uci);
        return None;
    }

    let bytes = uci.as_bytes();

    // Parse from square
    let from_file = bytes[0];
    let from_rank = bytes[1];

    // Parse to square
    let to_file = bytes[2];
    let to_rank = bytes[3];

    // Validate file (a-h)
    if !((b'a'..=b'h').contains(&from_file) && (b'a'..=b'h').contains(&to_file)) {
        warn!("Invalid UCI move format (invalid file): {}", uci);
        return None;
    }

    // Validate rank (1-8)
    if !((b'1'..=b'8').contains(&from_rank) && (b'1'..=b'8').contains(&to_rank)) {
        warn!("Invalid UCI move format (invalid rank): {}", uci);
        return None;
    }

    // Convert to indices
    // File: a=0, b=1, ..., h=7
    // Rank: 1=0, 2=1, ..., 8=7
    // Index = rank * 8 + file
    let from_file_idx = from_file - b'a';
    let from_rank_idx = from_rank - b'1';
    let from_index = from_rank_idx * 8 + from_file_idx;

    let to_file_idx = to_file - b'a';
    let to_rank_idx = to_rank - b'1';
    let to_index = to_rank_idx * 8 + to_file_idx;

    Some((from_index, to_index))
}

/// Convert square index (0-63) to UCI notation
fn square_index_to_uci(index: u8) -> String {
    let file = (index % 8) + b'a';
    let rank = (index / 8) + b'1';
    format!("{}{}", file as char, rank as char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uci_conversion() {
        // e2e4: from e2 (index 12) to e4 (index 28)
        assert_eq!(uci_to_square_indices("e2e4"), Some((12, 28)));

        // e7e8q: from e7 (index 52) to e8 (index 60)
        assert_eq!(uci_to_square_indices("e7e8q"), Some((52, 60)));

        // a1h8: from a1 (index 0) to h8 (index 63)
        assert_eq!(uci_to_square_indices("a1h8"), Some((0, 63)));

        // h1a8: from h1 (index 7) to a8 (index 56)
        assert_eq!(uci_to_square_indices("h1a8"), Some((7, 56)));

        // Invalid moves
        assert_eq!(uci_to_square_indices("invalid"), None);
        assert_eq!(uci_to_square_indices("i1a8"), None); // Invalid file
        assert_eq!(uci_to_square_indices("a9a8"), None); // Invalid rank
        assert_eq!(uci_to_square_indices("e2"), None); // Too short
    }

    #[test]
    fn test_index_to_uci_roundtrip() {
        for i in 0..64u8 {
            let uci = square_index_to_uci(i);
            let parsed = uci_to_square_indices(&uci);
            assert_eq!(parsed, Some((i, i)), "Roundtrip failed for index {}", i);
        }
    }
}
