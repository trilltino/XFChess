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
}

#[derive(Event, Message, Debug, Clone)]
pub struct GameStartedEvent {
    pub game_id: u64,
    pub white_player: String,
    pub black_player: String,
}

#[derive(Event, Message, Debug, Clone)]
pub struct GameEndedEvent {
    pub game_id: u64,
    pub winner: Option<String>,
    pub reason: String,
}

#[derive(Message, Debug, Clone)]
pub struct PlayerJoinedEvent {
    pub player_id: String,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct NetworkMoveEvent {
    pub from: (u8, u8),
    pub to: (u8, u8),
    pub promotion: Option<char>,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct ResignEvent {
    pub player: String,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct DrawOfferEvent {
    pub player: String,
}

#[derive(Message, Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
pub struct DrawResponseEvent {
    pub player: String,
    pub accepted: bool,
}
