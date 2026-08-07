//! Wire protocol for peer-to-peer game messages.
//!
//! [`NetworkMessage`] is the single enum carried over both the Iroh/QUIC
//! P2P transport and the HTTP relay fallback — variants cover move
//! exchange, batch commit/confirmation for on-chain move batching,
//! invites/matchmaking handshake (`GameInvite`/`InviteResponse`/`GameStart`),
//! in-game signaling (`DrawOffer`, `Resign`, `FlagTimeout`, `Chat`, `Clock`),
//! Braid-relay resync (`BraidResyncRequest`/`Response`, `GameSnapshot`), and
//! liveness (`Ping`/`Pong`). [`SignedNetworkMessage`] wraps a message with
//! its sender's signature for authenticity between peers.
use serde::{Deserialize, Serialize};
#[cfg(feature = "solana")]
use solana_sdk::pubkey::Pubkey;

#[cfg(not(feature = "solana"))]
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
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
        /// Ed25519 public key of the key that signs this message's gossip
        /// envelope — i.e. `OnlineNetworkState::session_signing_key`'s derived
        /// verifying key, **not** the iroh NodeId. (It was documented as the
        /// NodeId, and a version of the sender did populate it that way; since
        /// the receiver overwrites it with the verified signer in
        /// `bind_identity`, the two never matched and every move failed the
        /// roster check. Hence the explicit name.)
        ///
        /// Empty on legacy messages — skip causal check if absent.
        /// Serialized as `agent_id` for wire compatibility with older peers.
        #[serde(default, rename = "agent_id")]
        signer_pubkey: Vec<u8>,
        /// Monotonic counter per sender across all games.
        /// Allows detecting replays and sequence gaps independently of nonce.
        #[serde(default)]
        seq: u64,
        /// version_hash(prev_fen, prev_turn) of the move this one builds on.
        /// "0" on the first move. Used to detect equivocation forks.
        ///
        /// This is the *per-sender gossip* chain, not the shared Braid stream
        /// head — see `braid_transport::BraidStreamHeads`.
        #[serde(default)]
        parent_version: String,
    },
    SessionInfo {
        game_id: u64,
        player_pubkey: Pubkey,
        /// VPS/backend on-chain session-delegation key (used to route which
        /// key signs this player's on-chain moves) — NOT the key that signs
        /// this message's own outer P2P envelope. Kept separate from
        /// `signing_pubkey` below; conflating the two previously made every
        /// move look like it came from "a non-participant signer" once a
        /// roster existed at all (see `signing_pubkey`'s doc comment).
        session_pubkey: Pubkey,
        /// The sender's gossip message-signing pubkey (`OnlineNetworkState
        /// ::session_signing_key`, a fresh ephemeral keypair generated per
        /// connection — see `sync_session_key_to_network`). This, not
        /// `session_pubkey` above, is what the receiver's `bind_identity`
        /// verifies incoming `Move`/`Resign` envelopes against and sets as
        /// `signer_pubkey`. The per-game participant roster built in
        /// `multiplayer::systems` MUST be populated from this field —
        /// populating it from `session_pubkey` instead means the roster can
        /// never contain a key that any real move's verified signer will
        /// ever match, so every move gets rejected as "non-participant"
        /// as soon as the roster has any entry at all.
        #[serde(default)]
        signing_pubkey: Pubkey,
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
    /// In-game chat message sent over the online transport.
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
            NetworkMessage::BraidResyncRequest { game_id, .. } => *game_id,
            NetworkMessage::BraidResyncResponse { game_id, .. } => *game_id,
            NetworkMessage::GameSnapshot { game_id, .. } => *game_id,
            NetworkMessage::Clock { game_id, .. } => *game_id,
            NetworkMessage::Chat { game_id, .. } => *game_id,
        }
    }

    /// Short variant name for logging — cheap enough to compute on every
    /// send/receive without the cost (or noise) of dumping full contents.
    pub fn kind_str(&self) -> &'static str {
        match self {
            NetworkMessage::Move { .. } => "Move",
            NetworkMessage::SessionInfo { .. } => "SessionInfo",
            NetworkMessage::BatchPropose { .. } => "BatchPropose",
            NetworkMessage::BatchAccept { .. } => "BatchAccept",
            NetworkMessage::BatchReject { .. } => "BatchReject",
            NetworkMessage::TxMessage { .. } => "TxMessage",
            NetworkMessage::TxSignature { .. } => "TxSignature",
            NetworkMessage::Committed { .. } => "Committed",
            NetworkMessage::ResyncRequest { .. } => "ResyncRequest",
            NetworkMessage::ResyncResponse { .. } => "ResyncResponse",
            NetworkMessage::Resign { .. } => "Resign",
            NetworkMessage::BatchConfirmation { .. } => "BatchConfirmation",
            NetworkMessage::GameInvite { .. } => "GameInvite",
            NetworkMessage::InviteResponse { .. } => "InviteResponse",
            NetworkMessage::GameStart { .. } => "GameStart",
            NetworkMessage::GameStateBroadcast { .. } => "GameStateBroadcast",
            NetworkMessage::DrawOffer { .. } => "DrawOffer",
            NetworkMessage::DrawResponse { .. } => "DrawResponse",
            NetworkMessage::FlagTimeout { .. } => "FlagTimeout",
            NetworkMessage::Ping { .. } => "Ping",
            NetworkMessage::Pong { .. } => "Pong",
            NetworkMessage::RematchOffer { .. } => "RematchOffer",
            NetworkMessage::RematchResponse { .. } => "RematchResponse",
            NetworkMessage::BraidResyncRequest { .. } => "BraidResyncRequest",
            NetworkMessage::BraidResyncResponse { .. } => "BraidResyncResponse",
            NetworkMessage::GameSnapshot { .. } => "GameSnapshot",
            NetworkMessage::Clock { .. } => "Clock",
            NetworkMessage::Chat { .. } => "Chat",
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
            signer_pubkey: vec![],
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
            signer_pubkey: vec![],
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
            signer_pubkey: vec![],
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
            signer_pubkey: vec![],
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
