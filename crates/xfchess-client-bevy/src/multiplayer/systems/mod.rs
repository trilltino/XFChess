//! The P2P mesh lifecycle: joining the gossip mesh ([`mesh`]), turning network
//! messages into game events ([`events`]), game control and resync ([`control`]),
//! and feeding moves to the rollup ([`rollup_feed`], Solana builds).

use bevy::prelude::*;
use braid_chess;
use braid_http::{Update, Version};
use braid_iroh::{BraidIrohConfig, BraidIrohNode, DiscoveryConfig};
use futures_lite::StreamExt;
use iroh::{EndpointId, SecretKey};
use iroh_gossip::api::Event as IrohEvent;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use crate::game::events::ResignEvent;
use crate::multiplayer::network::protocol::{NetworkMessage, SignedNetworkMessage};
use crate::multiplayer::network::reorder::IngestOutcome;
use crate::multiplayer::types::*;
use crate::multiplayer::TokioRuntime;

#[cfg(feature = "solana")]
use crate::game::events::{GameEndedEvent, MoveMadeEvent};
use crate::game::resources::history::game_over::GameOverState;
#[cfg(feature = "solana")]
use crate::rendering::PieceType;

mod control;
mod events;
mod mesh;
#[cfg(feature = "solana")]
mod rollup_feed;
pub use control::*;
pub use events::*;
pub use mesh::*;
#[cfg(feature = "solana")]
pub use rollup_feed::*;
