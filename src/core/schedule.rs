//! The order of a frame's `Update` work, configured once by
//! [`CorePlugin`](super::plugin::CorePlugin):
//!
//! ```text
//! NetworkSet::Receive                 transports → game messages, in order:
//!   Transport → Invites → Relay → Game → Feeds
//! NetworkSet::Session                 background tasks and session state, in order:
//!   Wallet → Lobby → Match → Tournament → Rollup → Delegation → Social → Puzzle
//! GameSystems::Input → Validation → Execution → Visual      (InGame only)
//!   Validation: MoveSource::Replay → Resume → Spectator → Puzzle
//! MenuSystems::Main → LearnBoard                             (MainMenu only)
//! ApplySettings                       settings → audio, cameras, lights, themes
//! NetworkSet::Send                    outgoing work, in order:
//!   Moves → Session → Ack → RollupFeed → Settlement
//! ```
//!
//! Receiving first means a remote move that arrives this frame is applied this
//! frame; sending last means outgoing reports see the board after this frame's
//! moves. Inside a set, each plugin chains its own systems, so every system that
//! touches shared network or session state runs in one defined order.
//!
//! `GameSystems` runs only in `InGame` and `MenuSystems` only in `MainMenu`, so the
//! two are declared ambiguous with each other: they never run in the same frame.
//! The network sets run in every state (lobbies and menus poll too). UI draws in `EguiPrimaryContextPass`, after
//! `Update`. Debug builds warn at startup about any two systems in `Update` that
//! share data without a defined order (there are none today); fix one by adding it
//! to the right set or chain.

use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings};
use bevy::prelude::*;

use super::state::GameState;

/// The in-game pipeline; runs only in `GameState::InGame`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum GameSystems {
    /// Camera and pointer input.
    Input,
    Validation,
    /// Moves, turns, clocks, promotion and game-over detection.
    Execution,
    /// Animation and highlights.
    Visual,
}

/// Network and session work around the in-game pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum NetworkSet {
    /// Drains incoming transport messages into game messages.
    Receive,
    /// Finished background tasks and session state machines (see [`SessionSet`]).
    Session,
    /// Reports applied moves, results and acknowledgements.
    Send,
}

/// The ordered parts of [`NetworkSet::Receive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum ReceiveSet {
    /// The Braid subscription and the P2P mesh node.
    Transport,
    /// Direct P2P invites and connections.
    Invites,
    /// The server relay mailbox for lobby handshakes.
    Relay,
    /// Game traffic from the mesh: moves, resync, control, chat.
    Game,
    /// Read-only feeds: spectating and tournament gossip.
    Feeds,
}

/// Applies `GameSettings` to audio, cameras, lights and themes, after every
/// system that may change settings this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct ApplySettings;

/// Main-menu work; runs only in `GameState::MainMenu`, after the session phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum MenuSystems {
    /// The menu scene, assets, identity, music and keys.
    Main,
    /// The animated mini board in the LEARN box.
    LearnBoard,
}

/// The ordered parts of [`NetworkSet::Session`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SessionSet {
    /// Wallet connection, balance, rates and session keys.
    Wallet,
    /// Wagered lobbies and matchmaking.
    Lobby,
    /// Joining a specific game: resume, seat claims, join links.
    Match,
    Tournament,
    /// Ephemeral-rollup events.
    Rollup,
    /// Delegating a starting game to the rollup; dispute results.
    Delegation,
    /// Friends, presence and health logging.
    Social,
    /// Puzzle loading.
    Puzzle,
}

/// The ordered move sources in [`GameSystems::Validation`]. Each serves its own
/// `GameMode`, so at most one is active, but the order is still fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum MoveSource {
    Replay,
    Resume,
    Spectator,
    Puzzle,
}

/// The ordered parts of [`NetworkSet::Send`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SendSet {
    /// Publishing local moves, resigns, chat and clocks to the game log.
    Moves,
    /// Draw offers, heartbeats, game records, telemetry, spectator confirms.
    Session,
    /// Acknowledging applied remote moves.
    Ack,
    /// Feeding moves and the result to the rollup.
    RollupFeed,
    /// Rollup batches, finalization, undelegation and PGN export.
    Settlement,
}

pub(crate) fn configure(app: &mut App) {
    app.configure_sets(
        Update,
        (
            GameSystems::Input,
            GameSystems::Validation,
            GameSystems::Execution,
            GameSystems::Visual,
        )
            .chain()
            .run_if(in_state(GameState::InGame)),
    )
    .configure_sets(
        Update,
        (
            SessionSet::Wallet,
            SessionSet::Lobby,
            SessionSet::Match,
            SessionSet::Tournament,
            SessionSet::Rollup,
            SessionSet::Delegation,
            SessionSet::Social,
            SessionSet::Puzzle,
        )
            .chain()
            .in_set(NetworkSet::Session),
    )
    .configure_sets(
        Update,
        (
            MoveSource::Replay,
            MoveSource::Resume,
            MoveSource::Spectator,
            MoveSource::Puzzle,
        )
            .chain()
            .in_set(GameSystems::Validation),
    )
    .configure_sets(
        Update,
        (
            SendSet::Moves,
            SendSet::Session,
            SendSet::Ack,
            SendSet::RollupFeed,
            SendSet::Settlement,
        )
            .chain()
            .in_set(NetworkSet::Send),
    )
    .configure_sets(
        Update,
        (
            (NetworkSet::Receive, NetworkSet::Session)
                .chain()
                .before(GameSystems::Input),
            NetworkSet::Send.after(GameSystems::Visual),
        ),
    )
    .configure_sets(
        Update,
        (MenuSystems::Main, MenuSystems::LearnBoard)
            .chain()
            .after(NetworkSet::Session)
            .before(NetworkSet::Send)
            .run_if(in_state(GameState::MainMenu)),
    );
    app.configure_sets(
        Update,
        (
            ReceiveSet::Transport,
            ReceiveSet::Invites,
            ReceiveSet::Relay,
            ReceiveSet::Game,
            ReceiveSet::Feeds,
        )
            .chain()
            .in_set(NetworkSet::Receive),
    )
    .configure_sets(
        Update,
        ApplySettings
            .after(GameSystems::Visual)
            .after(MenuSystems::LearnBoard)
            .before(NetworkSet::Send),
    );
    // Menu and game sets are state-exclusive, so their data access never overlaps
    // at run time.
    for menu in [MenuSystems::Main, MenuSystems::LearnBoard] {
        app.configure_sets(
            Update,
            menu.ambiguous_with(GameSystems::Input)
                .ambiguous_with(GameSystems::Validation)
                .ambiguous_with(GameSystems::Execution)
                .ambiguous_with(GameSystems::Visual),
        );
    }

    // Every system that shares data with another has a defined order; debug builds
    // warn at startup if a change breaks that.
    if cfg!(debug_assertions) {
        app.edit_schedule(Update, |schedule| {
            schedule.set_build_settings(ScheduleBuildSettings {
                ambiguity_detection: LogLevel::Warn,
                ..default()
            });
        });
    }
}
