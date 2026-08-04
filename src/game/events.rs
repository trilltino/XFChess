use crate::rendering::pieces::PieceType;
use bevy::prelude::*;

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct MoveMadeEvent {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub player: String,
    pub piece_type: PieceType,
    pub captured_piece: Option<PieceType>,
    pub promotion: Option<PieceType>,
    pub remote: bool,
    pub game_id: Option<u64>,
    /// FEN string of the board position *after* this move was applied.
    pub next_fen: String,
}

/// Emitted by `handle_network_moves` after a remote move has been applied to the
/// chess engine. Used by `feed_remote_moves_to_rollup` to record opponent moves
/// on-chain without a frame-delay FEN race.
#[derive(Message, Debug, Clone)]
pub struct RemoteMoveApplied {
    pub uci: String,
    pub next_fen: String,
}

#[derive(Event, Message, Debug, Clone)]
pub struct GameStartedEvent {
    pub game_id: u64,
}

#[derive(Event, Message, Debug, Clone)]
pub struct GameEndedEvent {
    pub game_id: u64,
    pub winner: Option<String>,
    pub reason: String,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct NetworkMoveEvent {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub promotion: Option<char>,
    /// FEN the remote reported after applying this move; compared against local
    /// computation to detect board desync.
    pub expected_fen: Option<String>,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct ResignEvent {
    pub winner: String,
    pub remote: bool,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct DrawOfferEvent {
    pub player: String,
    /// true = this offer was received from the remote opponent over the network
    pub remote: bool,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct DrawResponseEvent {
    pub player: String,
    pub accepted: bool,
    pub remote: bool,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct RematchOfferEvent {
    pub player: String,
    pub remote: bool,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct RematchResponseEvent {
    pub player: String,
    pub accepted: bool,
    pub remote: bool,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct FlagTimeoutEvent {
    pub flagged_player: String,
    pub remote: bool,
}
