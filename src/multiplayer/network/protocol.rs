use serde::{Deserialize, Serialize};
#[cfg(feature = "solana")]
use solana_sdk::pubkey::Pubkey;

#[cfg(not(feature = "solana"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct Pubkey(pub [u8; 32]);

#[cfg(not(feature = "solana"))]
impl std::fmt::Display for Pubkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", bs58::encode(self.0).into_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    Move {
        game_id: u64,
        turn: u16,
        move_uci: String,
        next_fen: String,
        nonce: u64,
        /// Wall-clock time the move was sent (ms since UNIX epoch).
        #[serde(default)]
        timestamp_ms: u64,
        /// Ed25519 public key (iroh NodeId bytes) of the sender.
        /// Empty on legacy messages — skip causal check if absent.
        #[serde(default)]
        agent_id: Vec<u8>,
        /// Monotonic counter per agent across all games.
        /// Allows detecting replays and sequence gaps independently of nonce.
        #[serde(default)]
        seq: u64,
        /// version_hash(prev_fen, prev_turn) of the move this one builds on.
        /// "0" on the first move. Used to detect equivocation forks.
        #[serde(default)]
        parent_version: String,
    },
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
    Resign {
        game_id: u64,
        winner: String,
        nonce: u64,
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
    GameStateBroadcast {
        game_id: u64,
        fen: String,
        last_move: Option<String>,
        move_number: u32,
        is_check: bool,
    },
    /// Sent by a player offering a draw.
    DrawOffer {
        game_id: u64,
        /// "white" or "black"
        player: String,
    },
    /// Response to a DrawOffer — accepted=true means the game ends in a draw.
    DrawResponse {
        game_id: u64,
        /// "white" or "black" — the player sending this response
        player: String,
        accepted: bool,
    },
    /// Sent when a player's clock runs out to let the opponent verify and trigger game-over.
    FlagTimeout {
        game_id: u64,
        /// The player whose clock expired ("white" or "black").
        flagged_player: String,
    },
    /// Periodic liveness ping so both clients can detect a dropped connection.
    Ping {
        game_id: u64,
        /// Sender's wall-clock timestamp (milliseconds since UNIX epoch).
        timestamp_ms: u64,
    },
    /// Pong reply to a Ping.
    Pong {
        game_id: u64,
        timestamp_ms: u64,
    },
    /// Offer to rematch after a game ends.
    RematchOffer {
        game_id: u64,
        player: String,
    },
    /// Response to a RematchOffer.
    RematchResponse {
        game_id: u64,
        player: String,
        accepted: bool,
    },
    /// Request to pause the game clocks (both players must agree or host decides).
    PauseRequest {
        game_id: u64,
        player: String,
    },
    /// Resume game clocks after a pause.
    ResumeRequest {
        game_id: u64,
        player: String,
    },
    /// Request moves missed since `since_version` (content-hash of last applied move).
    /// Sent by a reconnecting client so the peer can replay the gap.
    BraidResyncRequest {
        game_id: u64,
        since_version: String,
    },
    /// Response to [`BraidResyncRequest`]: ordered list of missed move payloads.
    BraidResyncResponse {
        game_id: u64,
        /// JSON-encoded [`braid_chess::MovePayload`] values, oldest first.
        move_payloads: Vec<String>,
    },
    /// Broadcast by any peer when a new neighbor joins a game gossip topic.
    /// Carries the full current game state so the newcomer can catch up instantly.
    GameSnapshot {
        game_id: u64,
        /// Current FEN (authoritative board position).
        fen: String,
        /// All moves so far, each JSON-encoded as [`braid_chess::MovePayload`].
        move_payloads: Vec<String>,
        /// Content-addressed version of the last move in the log.
        head_version: String,
    },
    /// Clock snapshot sent after each local move so peers/spectators can track time.
    Clock {
        game_id: u64,
        white_ms: u64,
        black_ms: u64,
        timestamp_ms: u64,
    },
    /// In-game chat message sent over iroh gossip instead of Braid-HTTP.
    Chat {
        game_id: u64,
        player: String,
        text: String,
        timestamp_ms: u64,
    },
}

impl NetworkMessage {
    pub fn game_id(&self) -> u64 {
        match self {
            NetworkMessage::Move { game_id, .. } => *game_id,
            NetworkMessage::SessionInfo { game_id, .. } => *game_id,
            NetworkMessage::BatchPropose { game_id, .. } => *game_id,
            NetworkMessage::BatchAccept { game_id, .. } => *game_id,
            NetworkMessage::BatchReject { game_id, .. } => *game_id,
            NetworkMessage::TxMessage { game_id, .. } => *game_id,
            NetworkMessage::TxSignature { game_id, .. } => *game_id,
            NetworkMessage::Committed { game_id, .. } => *game_id,
            NetworkMessage::ResyncRequest { game_id, .. } => *game_id,
            NetworkMessage::ResyncResponse { game_id, .. } => *game_id,
            NetworkMessage::Resign { game_id, .. } => *game_id,
            NetworkMessage::BatchConfirmation { game_id, .. } => *game_id,
            NetworkMessage::GameInvite { game_id, .. } => *game_id,
            NetworkMessage::InviteResponse { game_id, .. } => *game_id,
            NetworkMessage::GameStart { game_id, .. } => *game_id,
            NetworkMessage::GameStateBroadcast { game_id, .. } => *game_id,
            NetworkMessage::DrawOffer { game_id, .. } => *game_id,
            NetworkMessage::DrawResponse { game_id, .. } => *game_id,
            NetworkMessage::FlagTimeout { game_id, .. } => *game_id,
            NetworkMessage::Ping { game_id, .. } => *game_id,
            NetworkMessage::Pong { game_id, .. } => *game_id,
            NetworkMessage::RematchOffer { game_id, .. } => *game_id,
            NetworkMessage::RematchResponse { game_id, .. } => *game_id,
            NetworkMessage::PauseRequest { game_id, .. } => *game_id,
            NetworkMessage::ResumeRequest { game_id, .. } => *game_id,
            NetworkMessage::BraidResyncRequest { game_id, .. } => *game_id,
            NetworkMessage::BraidResyncResponse { game_id, .. } => *game_id,
            NetworkMessage::GameSnapshot { game_id, .. } => *game_id,
            NetworkMessage::Clock { game_id, .. } => *game_id,
            NetworkMessage::Chat { game_id, .. } => *game_id,
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

/// A signed wrapper around [`NetworkMessage`] that carries an Ed25519 signature
/// from the on-chain session key.  Peers verify the signature before accepting
/// the inner message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedNetworkMessage {
    pub msg: NetworkMessage,
    pub session_pubkey: Vec<u8>,
    pub signature: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = i as u8;
        }
        k
    }

    #[test]
    fn sign_and_verify() {
        let sk = test_key();
        let msg = NetworkMessage::Move {
            game_id: 1,
            turn: 1,
            move_uci: "e2e4".to_string(),
            next_fen: "start".to_string(),
            nonce: 1,
            timestamp_ms: 0,
            agent_id: vec![],
            seq: 0,
            parent_version: String::new(),
        };
        let signed = SignedNetworkMessage::sign(msg.clone(), &sk);
        assert!(signed.verify());
    }

    #[test]
    fn tampered_message_rejected() {
        let sk = test_key();
        let msg = NetworkMessage::Move {
            game_id: 1,
            turn: 1,
            move_uci: "e2e4".to_string(),
            next_fen: "start".to_string(),
            nonce: 1,
            timestamp_ms: 0,
            agent_id: vec![],
            seq: 0,
            parent_version: String::new(),
        };
        let mut signed = SignedNetworkMessage::sign(msg, &sk);
        signed.msg = NetworkMessage::Move {
            game_id: 1,
            turn: 1,
            move_uci: "d2d4".to_string(),
            next_fen: "start".to_string(),
            nonce: 1,
            timestamp_ms: 0,
            agent_id: vec![],
            seq: 0,
            parent_version: String::new(),
        };
        assert!(!signed.verify());
    }

    #[test]
    fn tampered_signature_rejected() {
        let sk = test_key();
        let msg = NetworkMessage::Move {
            game_id: 1,
            turn: 1,
            move_uci: "e2e4".to_string(),
            next_fen: "start".to_string(),
            nonce: 1,
            timestamp_ms: 0,
            agent_id: vec![],
            seq: 0,
            parent_version: String::new(),
        };
        let mut signed = SignedNetworkMessage::sign(msg, &sk);
        if let Some(b) = signed.signature.first_mut() {
            *b ^= 0xFF;
        }
        assert!(!signed.verify());
    }
}

impl SignedNetworkMessage {
    /// Sign a [`NetworkMessage`] with the given Ed25519 signing key.
    /// The key bytes are the raw 32-byte seed (same format as Solana keypairs).
    pub fn sign(msg: NetworkMessage, signing_key_bytes: &[u8; 32]) -> Self {
        use ed25519_dalek::{Signer, SigningKey};
        let signing_key = SigningKey::from_bytes(signing_key_bytes);
        let signable = bincode::serialize(&msg).expect("bincode serialize");
        let signature = signing_key.sign(&signable).to_bytes().to_vec();
        let session_pubkey = signing_key.verifying_key().to_bytes().to_vec();
        Self {
            msg,
            session_pubkey,
            signature,
        }
    }

    /// Verify the Ed25519 signature on this message.
    /// Returns `true` iff the signature is cryptographically valid.
    pub fn verify(&self) -> bool {
        use ed25519_dalek::{Signature, VerifyingKey};
        if self.session_pubkey.len() != 32 || self.signature.len() != 64 {
            return false;
        }
        let pubkey_arr: [u8; 32] = match self.session_pubkey[..32].try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let sig_arr: [u8; 64] = match self.signature[..64].try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let Ok(verifying_key) = VerifyingKey::from_bytes(&pubkey_arr) else {
            return false;
        };
        let Ok(signable) = bincode::serialize(&self.msg) else {
            return false;
        };
        let signature = match Signature::try_from(&sig_arr[..]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        verifying_key.verify_strict(&signable, &signature).is_ok()
    }
}
