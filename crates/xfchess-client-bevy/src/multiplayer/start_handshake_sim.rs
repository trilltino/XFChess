//! Deterministic simulation of the online start handshake.
//!
//! Two clients run the real `handle_network_events` and `announce_game_readiness`
//! systems against a simulated gossip network. The network loses a message when the
//! receiver has not subscribed yet, and the test adds seeded drops, delays and
//! duplicates on top. Whatever the timing, both clients must end with a completed
//! start barrier, because an incomplete barrier blocks all input.
//!
//! The scenarios cover the bugs that have actually shipped: readiness arriving while
//! the receiver is still in the menu (the barrier was reset when it bound to the game),
//! and a single `GameReady` that is lost for good.
//!
//! Reproduce one scenario by calling `simulate` with the parameters printed on failure.

use std::time::Duration;

use bevy::prelude::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::core::GameMode;
use crate::game::components::{Piece, PieceColor, PieceType};
use crate::game::events::ResignEvent;
use crate::multiplayer::network::online_game_session::{
    announce_game_readiness, numeric_game_id, OnlineGameSession,
};
use crate::multiplayer::network::protocol::NetworkMessage;
use crate::multiplayer::systems::handle_network_events;
use crate::multiplayer::types::{
    CausalChainState, NetworkEvent, OnlineNetworkState, OnlineStartBarrier, PendingMoveBuffer,
};

const GAME: &str = "RA10-SIM";
const TICK: Duration = Duration::from_millis(100);
const MAX_TICKS: u32 = 3000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// When a client binds the game (subscribes to its topic) and when it starts playing.
#[derive(Clone, Copy, Debug)]
struct Timing {
    configure_tick: u32,
    enter_tick: u32,
}

struct Client {
    app: App,
    sent: UnboundedReceiver<NetworkMessage>,
    inbox: UnboundedSender<NetworkEvent>,
    timing: Timing,
    subscribed: bool,
}

impl Client {
    fn new(timing: Timing) -> Self {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(GameMode::SinglePlayer);
        app.insert_resource(OnlineGameSession::default());
        app.init_resource::<OnlineStartBarrier>();
        app.insert_resource(CausalChainState::default());
        app.insert_resource(PendingMoveBuffer::default());
        app.add_message::<NetworkEvent>();
        app.add_message::<ResignEvent>();
        let (out_tx, sent) = unbounded_channel();
        let (inbox, in_rx) = unbounded_channel();
        app.insert_resource(OnlineNetworkState {
            message_sender: Some(out_tx),
            event_receiver: Some(in_rx),
            ..Default::default()
        });
        app.add_systems(
            Update,
            (handle_network_events, announce_game_readiness).chain(),
        );
        Self {
            app,
            sent,
            inbox,
            timing,
            subscribed: false,
        }
    }

    fn step(&mut self, tick: u32) {
        if tick == self.timing.configure_tick {
            let mut session = self.app.world_mut().resource_mut::<OnlineGameSession>();
            session.base_url = "http://relay.invalid".into();
            session.game_id = GAME.into();
            session.active = true;
            self.subscribed = true;
        }
        if tick == self.timing.enter_tick {
            *self.app.world_mut().resource_mut::<GameMode>() = GameMode::OnlineMultiplayer;
            for i in 0..32u8 {
                self.app.world_mut().spawn(Piece::new(
                    PieceColor::White,
                    PieceType::Pawn,
                    i % 8,
                    i / 8,
                ));
            }
        }
        self.app.world_mut().resource_mut::<Time>().advance_by(TICK);
        self.app.update();
    }

    fn barrier(&self) -> &OnlineStartBarrier {
        self.app.world().resource::<OnlineStartBarrier>()
    }
}

#[derive(Clone, Copy, Debug)]
struct Network {
    seed: u64,
    drop_percent: u64,
    duplicate_percent: u64,
    max_delay_ticks: u64,
}

/// Runs one scenario and returns the tick at which both barriers were complete.
fn simulate(host: Timing, joiner: Timing, net: Network) -> Result<u32, String> {
    let mut clients = [Client::new(host), Client::new(joiner)];
    let mut rng = Rng(net.seed | 1);
    let mut in_flight: Vec<(u32, usize, NetworkMessage)> = Vec::new();
    let gid = numeric_game_id(GAME);
    let mut safety: Option<String> = None;

    for tick in 0..MAX_TICKS {
        for client in clients.iter_mut() {
            client.step(tick);
        }

        for from in 0..2 {
            while let Ok(message) = clients[from].sent.try_recv() {
                let to = 1 - from;
                // Gossip only reaches peers already subscribed to the topic.
                if !clients[to].subscribed || rng.chance(net.drop_percent) {
                    continue;
                }
                let copies = if rng.chance(net.duplicate_percent) {
                    2
                } else {
                    1
                };
                for _ in 0..copies {
                    let delay = if net.max_delay_ticks == 0 {
                        0
                    } else {
                        rng.next() % (net.max_delay_ticks + 1)
                    };
                    in_flight.push((tick + 1 + delay as u32, to, message.clone()));
                }
            }
        }
        let mut still = Vec::new();
        for (due, to, message) in in_flight.drain(..) {
            if due <= tick + 1 {
                let _ = clients[to]
                    .inbox
                    .send(NetworkEvent::MessageReceived(message));
            } else {
                still.push((due, to, message));
            }
        }
        in_flight = still;

        // A start must never complete on one side before the other announced.
        for me in 0..2 {
            if clients[me].barrier().is_complete(gid) && !clients[1 - me].barrier().ready_sent {
                safety.get_or_insert(format!(
                    "client {me} started at tick {tick} before client {} announced readiness",
                    1 - me
                ));
            }
        }
        if let Some(violation) = safety {
            return Err(violation);
        }
        if clients.iter().all(|c| c.barrier().is_complete(gid)) {
            return Ok(tick);
        }
    }
    let describe = |c: &Client| {
        let b = c.barrier();
        format!(
            "game_id={} local_ready={} remote_ready={} start_confirmed={} remote_started={}",
            b.game_id, b.local_ready, b.remote_ready, b.start_confirmed, b.remote_started
        )
    };
    Err(format!(
        "no start after {MAX_TICKS} ticks | host: {} | joiner: {}",
        describe(&clients[0]),
        describe(&clients[1])
    ))
}

#[test]
fn a_perfect_network_starts_immediately_whatever_the_join_timing() -> Result<(), String> {
    for host_conf in [0, 3] {
        for joiner_conf in [0, 3, 8] {
            for joiner_gap in [0, 2, 6] {
                let host = Timing {
                    configure_tick: host_conf,
                    enter_tick: host_conf + 1,
                };
                let joiner = Timing {
                    configure_tick: joiner_conf,
                    enter_tick: joiner_conf + joiner_gap,
                };
                let net = Network {
                    seed: 1,
                    drop_percent: 0,
                    duplicate_percent: 0,
                    max_delay_ticks: 0,
                };
                simulate(host, joiner, net)
                    .map_err(|e| format!("host {host:?} joiner {joiner:?} perfect network: {e}"))?;
            }
        }
    }
    Ok(())
}

#[test]
fn readiness_that_arrives_while_the_receiver_is_still_in_the_menu_is_not_lost() -> Result<(), String>
{
    // The joiner subscribes immediately but only starts playing 30 ticks later, as in
    // the game logs: the host's single GameReady lands while the joiner is in the menu.
    let host = Timing {
        configure_tick: 0,
        enter_tick: 1,
    };
    let joiner = Timing {
        configure_tick: 0,
        enter_tick: 30,
    };
    let net = Network {
        seed: 9,
        drop_percent: 0,
        duplicate_percent: 0,
        max_delay_ticks: 0,
    };
    simulate(host, joiner, net)
        .map(|_| ())
        .map_err(|e| format!("host {host:?} joiner {joiner:?}: {e}"))
}

#[test]
fn every_timing_survives_loss_delay_and_duplication() -> Result<(), String> {
    let confs = [0, 4];
    let gaps = [0, 3, 25];
    let nets = [(0, 0, 0), (30, 0, 3), (30, 30, 8), (60, 20, 5)];
    let mut scenarios = 0;
    for &host_conf in &confs {
        for &host_gap in &gaps {
            for &joiner_conf in &confs {
                for &joiner_gap in &gaps {
                    for (index, &(drop, dup, delay)) in nets.iter().enumerate() {
                        for seed in 1..=3u64 {
                            let host = Timing {
                                configure_tick: host_conf,
                                enter_tick: host_conf + host_gap,
                            };
                            let joiner = Timing {
                                configure_tick: joiner_conf,
                                enter_tick: joiner_conf + joiner_gap,
                            };
                            let net = Network {
                                seed: seed * 7919 + index as u64,
                                drop_percent: drop,
                                duplicate_percent: dup,
                                max_delay_ticks: delay,
                            };
                            simulate(host, joiner, net).map_err(|e| {
                                format!("host {host:?} joiner {joiner:?} {net:?}: {e}")
                            })?;
                            scenarios += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(scenarios >= 200, "only {scenarios} scenarios ran");
    Ok(())
}

#[test]
fn both_clients_leave_the_barrier_resettable_for_the_next_game() {
    let timing = Timing {
        configure_tick: 0,
        enter_tick: 1,
    };
    let mut client = Client::new(timing);
    for tick in 0..5 {
        client.step(tick);
    }
    assert!(client.barrier().ready_sent);
    client
        .app
        .world_mut()
        .resource_mut::<OnlineStartBarrier>()
        .reset(0);
    assert!(!client.barrier().ready_sent);
}
