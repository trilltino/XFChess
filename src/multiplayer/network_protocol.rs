use serde::{Deserialize, Serialize};
#[cfg(feature = "solana")]
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Move {
        game_id: u64,
        turn: u16,
        move_uci: String,
        next_fen: String,
    },
    #[cfg(feature = "solana")]
    SessionInfo {
        game_id: u64,
        player_pubkey: Pubkey,
        session_pubkey: Pubkey,
        expires_at: i64,
    },
    BatchPropose {
        game_id: u64,
        start_turn: u16,
        moves: Vec<String>,
        next_fens: Vec<String>,
    },
    BatchAccept {
        game_id: u64,
        batch_hash: String,
    },
    BatchReject {
        game_id: u64,
        reason: String,
    },
    TxMessage {
        game_id: u64,
        message_bytes: Vec<u8>,
    },
    #[cfg(feature = "solana")]
    TxSignature {
        game_id: u64,
        signer_pubkey: Pubkey,
        signature_bytes: Vec<u8>,
    },
    Committed {
        game_id: u64,
        tx_sig: String,
        new_fen: String,
        new_turn: u16,
    },
    ResyncRequest {
        game_id: u64,
    },
    ResyncResponse {
        game_id: u64,
        committed_fen: String,
        committed_turn: u16,
    },
    BatchConfirmation {
        game_id: u64,
        batch_hash: String,
        tx_sig: String,
    },
    GameInvite {
        game_id: u64,
        from_node: String,
        from_wallet: String,
    },
    InviteResponse {
        game_id: u64,
        accepted: bool,
    },
    GameStart {
        game_id: u64,
        white_player: String,
        black_player: String,
        initial_fen: String,
    },
}

impl NetworkMessage {
    pub fn game_id(&self) -> u64 {
        match self {
            NetworkMessage::Move { game_id, .. } => *game_id,
            #[cfg(feature = "solana")]
            NetworkMessage::SessionInfo { game_id, .. } => *game_id,
            NetworkMessage::BatchPropose { game_id, .. } => *game_id,
            NetworkMessage::BatchAccept { game_id, .. } => *game_id,
            NetworkMessage::BatchReject { game_id, .. } => *game_id,
            NetworkMessage::TxMessage { game_id, .. } => *game_id,
            #[cfg(feature = "solana")]
            NetworkMessage::TxSignature { game_id, .. } => *game_id,
            NetworkMessage::Committed { game_id, .. } => *game_id,
            NetworkMessage::ResyncRequest { game_id, .. } => *game_id,
            NetworkMessage::ResyncResponse { game_id, .. } => *game_id,
            NetworkMessage::BatchConfirmation { game_id, .. } => *game_id,
            NetworkMessage::GameInvite { game_id, .. } => *game_id,
            NetworkMessage::InviteResponse { game_id, .. } => *game_id,
            NetworkMessage::GameStart { game_id, .. } => *game_id,
        }
    }
}

// Helper function to calculate deterministic batch hash
pub fn calculate_batch_hash(
    game_id: u64,
    start_turn: u16,
    moves: &[String],
    next_fens: &[String],
) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(game_id.to_le_bytes());
    hasher.update(start_turn.to_le_bytes());

    for move_str in moves {
        hasher.update(move_str.as_bytes());
    }

    for fen in next_fens {
        hasher.update(fen.as_bytes());
    }

    format!("{:x}", hasher.finalize())
}
