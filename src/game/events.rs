use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Event, Debug, Clone, Serialize, Deserialize, Message)]
pub struct NetworkMoveEvent {
    pub from: (u8, u8),
    pub to: (u8, u8),
}
