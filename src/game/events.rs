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
    pub next_fen: String,
}

#[derive(Message, Debug, Clone)]
pub struct RemoteMoveApplied {
    pub uci: String,
    pub next_fen: String,
}

#[derive(Message, Debug, Clone)]
pub struct NetworkMoveResult {
    pub version: Option<String>,
    pub fen: Option<String>,
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
    pub expected_fen: Option<String>,
    #[serde(default)]
    pub dedup_version: Option<String>,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct ResignEvent {
    pub winner: String,
    pub remote: bool,
}

/// The player chose to resign (or to leave a game, which resigns it). In a wagered game
/// the resignation is recorded on-chain before it takes effect; elsewhere it becomes
/// a local [`ResignEvent`] at once.
#[derive(Message, Debug, Clone)]
pub struct ResignRequest {
    /// The side that wins by it ("white" / "black").
    pub winner: String,
}

/// What the player asked for in the draw UI. In a wagered game an offer or acceptance
/// is recorded on-chain before it is announced; everywhere else it is announced
/// straight away as a [`DrawOfferEvent`] / [`DrawResponseEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawActionKind {
    Offer,
    Accept,
    Decline,
}

#[derive(Message, Debug, Clone)]
pub struct DrawActionRequest {
    /// Colour label of the player acting ("white" / "black").
    pub player: String,
    pub kind: DrawActionKind,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct DrawOfferEvent {
    pub player: String,
    pub remote: bool,
    /// Position the offer was made in, when the transport says (Braid does,
    /// gossip does not); otherwise the current position is assumed.
    #[serde(default)]
    pub ply: Option<u32>,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct DrawResponseEvent {
    pub player: String,
    pub accepted: bool,
    pub remote: bool,
    /// Position the answer refers to, when the transport says.
    #[serde(default)]
    pub ply: Option<u32>,
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
