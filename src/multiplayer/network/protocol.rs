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
        #[serde(default)]
        timestamp_ms: u64,
        #[serde(default, rename = "agent_id")]
        signer_pubkey: Vec<u8>,
        #[serde(default)]
        seq: u64,
        #[serde(default)]
        parent_version: String,
    },
    SessionInfo {
        game_id: u64,
        player_pubkey: Pubkey,
        session_pubkey: Pubkey,
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
        from_display: String,
    },
    InviteResponse {
        game_id: u64,
        accepted: bool,
        display_name: String,
    },
    GameStart {
        game_id: u64,
        white_player: String,
        black_player: String,
        initial_fen: String,
        white_display: String,
        black_display: String,
    },
    GameReady {
        game_id: u64,
        player_pubkey: Pubkey,
        ready_token: String,
    },
    GameStartConfirmed {
        game_id: u64,
        start_token: String,
    },
    GameStateBroadcast {
        game_id: u64,
        fen: String,
        last_move: Option<String>,
        move_number: u32,
        is_check: bool,
    },
    DrawOffer {
        game_id: u64,
        player: String,
    },
    DrawResponse {
        game_id: u64,
        player: String,
        accepted: bool,
    },
    FlagTimeout {
        game_id: u64,
        flagged_player: String,
    },
    Ping {
        game_id: u64,
        timestamp_ms: u64,
    },
    Pong {
        game_id: u64,
        timestamp_ms: u64,
    },
    RematchOffer {
        game_id: u64,
        player: String,
    },
    RematchResponse {
        game_id: u64,
        player: String,
        accepted: bool,
    },
    BraidResyncRequest {
        game_id: u64,
        since_version: String,
    },
    BraidResyncResponse {
        game_id: u64,
        move_payloads: Vec<String>,
    },
    GameSnapshot {
        game_id: u64,
        fen: String,
        move_payloads: Vec<String>,
        head_version: String,
    },
    Clock {
        game_id: u64,
        white_ms: u64,
        black_ms: u64,
        timestamp_ms: u64,
    },
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
            NetworkMessage::GameReady { game_id, .. } => *game_id,
            NetworkMessage::GameStartConfirmed { game_id, .. } => *game_id,
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
            NetworkMessage::GameReady { .. } => "GameReady",
            NetworkMessage::GameStartConfirmed { .. } => "GameStartConfirmed",
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

#[cfg(test)]
mod synchronized_start_tests {
    use super::{NetworkMessage, Pubkey};

    #[test]
    fn synchronized_start_messages_round_trip() {
        let ready = NetworkMessage::GameReady {
            game_id: 7,
            player_pubkey: Pubkey::default(),
            ready_token: "game-7".to_string(),
        };
        let encoded = bincode::serialize(&ready).expect("ready message serializes");
        let decoded: NetworkMessage =
            bincode::deserialize(&encoded).expect("ready message decodes");
        assert_eq!(decoded.game_id(), 7);
        assert_eq!(decoded.kind_str(), "GameReady");

        let start = NetworkMessage::GameStartConfirmed {
            game_id: 7,
            start_token: "game-7".to_string(),
        };
        let encoded = bincode::serialize(&start).expect("start message serializes");
        let decoded: NetworkMessage =
            bincode::deserialize(&encoded).expect("start message decodes");
        assert_eq!(decoded.game_id(), 7);
        assert_eq!(decoded.kind_str(), "GameStartConfirmed");
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
