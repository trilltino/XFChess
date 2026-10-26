//! Player actions in a wagered game that the Game account must record.
//!
//! A resignation or an agreed draw only pays out once the account says `Finished`,
//! and the backend settles nothing for them on its own. So in a wagered game:
//!
//! * a draw offer or acceptance is signed in the wallet and recorded **first**,
//!   and only then announced to the opponent. The opponent can therefore never
//!   accept something the chain does not have, and a refused wallet prompt leaves
//!   the game running instead of ending it locally with nothing to settle;
//! * a resignation is recorded the same way before it takes effect, for the same
//!   reason: the screen must never say "resigned" while the chain says the game
//!   is still on (an unrecorded resignation would later time out against whoever
//!   happened to be on move).
//!
//! If recording fails the player is told, the game carries on, and "Try again"
//! repeats it. Transactions go to the rollup while the game is delegated (see
//! [`super::lobby::submit_game_action`]). Casual games have no account and never
//! come through here: their requests are answered directly.

use bevy::prelude::*;
use bevy_egui::egui;
use solana_sdk::pubkey::Pubkey;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

use super::addon::{SolanaGameSync, SolanaWallet};
use super::lobby::{submit_game_action, GameChainAction};
use crate::game::events::{
    DrawActionKind, DrawActionRequest, DrawOfferEvent, DrawResponseEvent, ResignEvent,
    ResignRequest,
};
use crate::game::systems::network_move::ChainDrawGate;

/// How long a success notice stays up.
const NOTICE_SECS: u64 = 5;

struct InFlight {
    action: GameChainAction,
    game_id: u64,
    /// Colour label of the acting player ("white"/"black").
    player: String,
    rx: oneshot::Receiver<Result<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ChainNotice {
    pub text: String,
    pub is_error: bool,
    /// What pressing "Try again" repeats, for a failure.
    pub retry: Option<(GameChainAction, String)>,
    shown_at: Instant,
}

#[derive(Resource, Default)]
pub struct ChainActions {
    in_flight: Vec<InFlight>,
    pub notice: Option<ChainNotice>,
}

struct Target {
    wallet: Pubkey,
    game_id: u64,
    rpc_url: String,
}

/// Where this game's account lives, if it has to be written to. A free rated game
/// (no wager, never delegated) settles off-chain, so its resignation and draws need
/// no wallet prompt and no base-layer fee.
fn target(wallet: Option<&SolanaWallet>, sync: Option<&SolanaGameSync>) -> Option<Target> {
    let sync = sync?;
    if !(sync.requires_delegation || sync.wager_amount > 0) {
        return None;
    }
    Some(Target {
        wallet: wallet?.pubkey?,
        game_id: sync.game_id?,
        rpc_url: sync.rpc_url.clone(),
    })
}

impl ChainActions {
    fn is_running(&self, action: GameChainAction) -> bool {
        self.in_flight.iter().any(|f| f.action == action)
    }

    pub fn is_busy(&self) -> bool {
        !self.in_flight.is_empty()
    }

    fn start(&mut self, action: GameChainAction, player: String, target: &Target) {
        // A second click while the wallet prompt is up must not queue a second one.
        if self.is_running(action) {
            return;
        }
        let (tx, rx) = oneshot::channel();
        let (wallet, game_id, rpc_url) = (target.wallet, target.game_id, target.rpc_url.clone());
        crate::core::net_task::spawn_detached("game-action", move || {
            let result = submit_game_action(action, &rpc_url, wallet, game_id)
                .map(|signature| signature.to_string());
            let _ = tx.send(result);
        });
        self.notice = None;
        self.in_flight.push(InFlight {
            action,
            game_id: target.game_id,
            player,
            rx,
        });
    }

    fn notify(&mut self, text: impl Into<String>, is_error: bool) {
        self.notice = Some(ChainNotice {
            text: text.into(),
            is_error,
            retry: None,
            shown_at: Instant::now(),
        });
    }

    fn fail(&mut self, action: GameChainAction, player: String, error: &str) {
        self.notice = Some(ChainNotice {
            text: friendly_error(action, error),
            is_error: true,
            retry: Some((action, player)),
            shown_at: Instant::now(),
        });
    }

    #[cfg(test)]
    fn push_finished_for_test(
        &mut self,
        action: GameChainAction,
        player: &str,
        result: Result<String, String>,
    ) {
        let (tx, rx) = oneshot::channel();
        let _ = tx.send(result);
        self.in_flight.push(InFlight {
            action,
            game_id: 0,
            player: player.to_string(),
            rx,
        });
    }
}

fn thing(action: GameChainAction) -> &'static str {
    match action {
        GameChainAction::Resign => "your resignation",
        GameChainAction::OfferDraw => "the draw offer",
        GameChainAction::AcceptDraw => "the draw",
    }
}

/// Text for the player; the technical error goes to the log.
fn friendly_error(action: GameChainAction, error: &str) -> String {
    let lower = error.to_lowercase();
    if ["reject", "denied", "declined", "cancel"]
        .iter()
        .any(|word| lower.contains(word))
    {
        return format!(
            "You declined the wallet request, so {} was not recorded.",
            thing(action)
        );
    }
    let consequence = match action {
        GameChainAction::Resign => {
            " The game will still settle against the side that stops moving."
        }
        _ => " The game is still on.",
    };
    format!("Couldn't record {} on-chain.{consequence}", thing(action))
}

/// Draws are recorded first (and announced after) exactly while a wagered game with a
/// connected wallet is running.
fn update_gate(
    mut gate: ResMut<ChainDrawGate>,
    wallet: Option<Res<SolanaWallet>>,
    sync: Option<Res<SolanaGameSync>>,
) {
    let active = target(wallet.as_deref(), sync.as_deref()).is_some();
    if gate.active != active {
        gate.active = active;
    }
}

fn submit_chain_draws(
    mut requests: MessageReader<DrawActionRequest>,
    gate: Res<ChainDrawGate>,
    mut chain: ResMut<ChainActions>,
    wallet: Option<Res<SolanaWallet>>,
    sync: Option<Res<SolanaGameSync>>,
) {
    for request in requests.read() {
        let action = match request.kind {
            DrawActionKind::Offer => GameChainAction::OfferDraw,
            DrawActionKind::Accept => GameChainAction::AcceptDraw,
            DrawActionKind::Decline => continue,
        };
        if !gate.active {
            continue;
        }
        if let Some(target) = target(wallet.as_deref(), sync.as_deref()) {
            chain.start(action, request.player.clone(), &target);
        }
    }
}

fn submit_chain_resign(
    mut resigns: MessageReader<ResignRequest>,
    gate: Res<ChainDrawGate>,
    mut chain: ResMut<ChainActions>,
    wallet: Option<Res<SolanaWallet>>,
    sync: Option<Res<SolanaGameSync>>,
) {
    for request in resigns.read() {
        if !gate.active {
            continue;
        }
        let Some(target) = target(wallet.as_deref(), sync.as_deref()) else {
            continue;
        };
        // The side that wins is the other one.
        let resigning = if request.winner == "white" {
            "black"
        } else {
            "white"
        };
        chain.start(GameChainAction::Resign, resigning.to_string(), &target);
    }
}

fn poll_chain_actions(
    mut chain: ResMut<ChainActions>,
    mut offers: MessageWriter<DrawOfferEvent>,
    mut answers: MessageWriter<DrawResponseEvent>,
    mut resigns: MessageWriter<ResignEvent>,
    sync: Option<Res<SolanaGameSync>>,
) {
    if chain.in_flight.is_empty() {
        return;
    }
    let current = sync.and_then(|sync| sync.game_id);
    let mut finished = Vec::new();
    chain
        .in_flight
        .retain_mut(|flight| match flight.rx.try_recv() {
            // An answer for a game that is no longer the running one must not leak into the next.
            Ok(_) if current.is_some_and(|current| current != flight.game_id) => false,
            Ok(result) => {
                finished.push((flight.action, flight.player.clone(), result));
                false
            }
            Err(oneshot::error::TryRecvError::Empty) => true,
            Err(oneshot::error::TryRecvError::Closed) => {
                finished.push((
                    flight.action,
                    flight.player.clone(),
                    Err("the wallet request stopped unexpectedly".to_string()),
                ));
                false
            }
        });
    for (action, player, result) in finished {
        match (action, result) {
            // Recorded: now the opponent hears of it.
            (GameChainAction::OfferDraw, Ok(_)) => {
                offers.write(DrawOfferEvent {
                    player,
                    remote: false,
                    ply: None,
                });
                chain.notify("Draw offered.", false);
            }
            (GameChainAction::AcceptDraw, Ok(_)) => {
                answers.write(DrawResponseEvent {
                    player,
                    accepted: true,
                    remote: false,
                    ply: None,
                });
                chain.notice = None;
            }
            (GameChainAction::Resign, Ok(_)) => {
                resigns.write(ResignEvent {
                    winner: if player == "white" { "black" } else { "white" }.to_string(),
                    remote: false,
                });
                chain.notify("Resignation recorded on-chain.", false);
            }
            (action, Err(error)) => chain.fail(action, player, &error),
        }
    }
}

fn chain_notice_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut chain: ResMut<ChainActions>,
    mut requests: MessageWriter<DrawActionRequest>,
    mut resign_requests: MessageWriter<ResignRequest>,
) {
    // Successes fade; failures wait to be dealt with.
    if chain.notice.as_ref().is_some_and(|notice| {
        !notice.is_error && notice.shown_at.elapsed() > Duration::from_secs(NOTICE_SECS)
    }) {
        chain.notice = None;
    }
    let waiting = chain.is_busy();
    if !waiting && chain.notice.is_none() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut dismiss = false;
    let mut retry = None;
    egui::Window::new("chain_action_notice")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_TOP, [-16.0, 56.0])
        .fixed_size([320.0, 0.0])
        .show(ctx, |ui| {
            if waiting {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Approve in your wallet to record it on-chain…");
                });
            } else if let Some(notice) = &chain.notice {
                let color = if notice.is_error {
                    egui::Color32::from_rgb(255, 160, 120)
                } else {
                    egui::Color32::from_rgb(140, 220, 150)
                };
                ui.colored_label(color, &notice.text);
                if notice.is_error {
                    ui.horizontal(|ui| {
                        if notice.retry.is_some() && ui.button("Try again").clicked() {
                            retry = notice.retry.clone();
                        }
                        if ui.button("Dismiss").clicked() {
                            dismiss = true;
                        }
                    });
                }
            }
        });

    if dismiss {
        chain.notice = None;
    }
    if let Some((action, player)) = retry {
        chain.notice = None;
        match action {
            GameChainAction::OfferDraw => {
                requests.write(DrawActionRequest {
                    player,
                    kind: DrawActionKind::Offer,
                });
            }
            GameChainAction::AcceptDraw => {
                requests.write(DrawActionRequest {
                    player,
                    kind: DrawActionKind::Accept,
                });
            }
            GameChainAction::Resign => {
                resign_requests.write(ResignRequest {
                    winner: if player == "white" { "black" } else { "white" }.to_string(),
                });
            }
        }
    }
}

/// Whatever is still waiting on a wallet belongs to the game being left.
fn forget_chain_actions(mut chain: ResMut<ChainActions>) {
    chain.in_flight.clear();
    chain.notice = None;
}

pub struct ChainActionsPlugin;

impl Plugin for ChainActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChainActions>()
            .add_systems(
                Update,
                (
                    update_gate,
                    (submit_chain_draws, submit_chain_resign),
                    poll_chain_actions,
                )
                    .chain()
                    .in_set(crate::core::schedule::SessionSet::Match),
            )
            .add_systems(OnExit(crate::core::state::InGameplay), forget_chain_actions)
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                chain_notice_ui.run_if(in_state(crate::core::state::InGameplay)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ChainActions>();
        app.add_message::<DrawOfferEvent>();
        app.add_message::<DrawResponseEvent>();
        app.add_message::<ResignEvent>();
        app.add_systems(Update, poll_chain_actions);
        app
    }

    fn drain<M: Message + Clone>(app: &App) -> Vec<M> {
        let messages = app.world().resource::<Messages<M>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn an_offer_is_announced_only_after_the_chain_recorded_it() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChainActions>()
            .push_finished_for_test(GameChainAction::OfferDraw, "white", Ok("sig".into()));
        app.update();
        let offers = drain::<DrawOfferEvent>(&app);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].player, "white");
        assert!(!offers[0].remote);
    }

    #[test]
    fn a_refused_wallet_prompt_announces_nothing_and_offers_a_retry() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChainActions>()
            .push_finished_for_test(
                GameChainAction::AcceptDraw,
                "black",
                Err("Wallet request was rejected".into()),
            );
        app.update();
        assert!(drain::<DrawResponseEvent>(&app).is_empty());
        let chain = app.world().resource::<ChainActions>();
        let notice = chain.notice.as_ref();
        assert!(notice.is_some_and(|n| n.is_error && n.retry.is_some()));
        assert!(notice.is_some_and(|n| n.text.contains("declined")));
    }

    #[test]
    fn an_accepted_draw_is_announced_once_recorded() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChainActions>()
            .push_finished_for_test(GameChainAction::AcceptDraw, "black", Ok("sig".into()));
        app.update();
        let answers = drain::<DrawResponseEvent>(&app);
        assert_eq!(answers.len(), 1);
        assert!(answers[0].accepted && !answers[0].remote);
    }

    #[test]
    fn a_recorded_resignation_takes_effect_for_the_other_side() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChainActions>()
            .push_finished_for_test(GameChainAction::Resign, "black", Ok("sig".into()));
        app.update();
        let resigns = drain::<ResignEvent>(&app);
        assert_eq!(resigns.len(), 1);
        assert_eq!(resigns[0].winner, "white");
        assert!(!resigns[0].remote);
    }

    #[test]
    fn a_resignation_that_could_not_be_recorded_does_not_take_effect() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChainActions>()
            .push_finished_for_test(
                GameChainAction::Resign,
                "black",
                Err("Transaction simulation failed".into()),
            );
        app.update();
        assert!(drain::<ResignEvent>(&app).is_empty());
    }

    #[test]
    fn a_failed_resignation_says_how_the_game_will_still_settle() {
        let text = friendly_error(GameChainAction::Resign, "Transaction simulation failed");
        assert!(text.contains("settle"));
        let draw = friendly_error(GameChainAction::AcceptDraw, "Transaction simulation failed");
        assert!(draw.contains("still on"));
    }

    #[test]
    fn only_a_game_with_a_wager_or_a_rollup_copy_is_written_to_the_chain() {
        let wallet = SolanaWallet {
            pubkey: Some(Pubkey::new_unique()),
            ..Default::default()
        };
        let mut sync = SolanaGameSync {
            game_id: Some(7),
            ..Default::default()
        };
        assert!(
            target(Some(&wallet), Some(&sync)).is_none(),
            "free rated settles off-chain"
        );
        sync.wager_amount = 1;
        assert!(target(Some(&wallet), Some(&sync)).is_some());
        sync.wager_amount = 0;
        sync.requires_delegation = true;
        assert!(
            target(Some(&wallet), Some(&sync)).is_some(),
            "tournament games run on the rollup"
        );
        sync.game_id = None;
        assert!(target(Some(&wallet), Some(&sync)).is_none());
        sync.game_id = Some(7);
        assert!(
            target(Some(&SolanaWallet::default()), Some(&sync)).is_none(),
            "no wallet, no signing"
        );
    }

    #[test]
    fn an_answer_for_a_previous_game_is_dropped() {
        let mut app = app();
        app.insert_resource(SolanaGameSync {
            game_id: Some(99),
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<ChainActions>()
            .push_finished_for_test(GameChainAction::OfferDraw, "white", Ok("sig".into()));
        app.update();
        assert!(drain::<DrawOfferEvent>(&app).is_empty());
        assert!(!app.world().resource::<ChainActions>().is_busy());
    }

    #[test]
    fn a_second_click_does_not_queue_a_second_wallet_prompt() {
        let mut chain = ChainActions::default();
        chain.push_finished_for_test(GameChainAction::OfferDraw, "white", Ok("x".into()));
        assert!(chain.is_running(GameChainAction::OfferDraw));
        assert!(!chain.is_running(GameChainAction::AcceptDraw));
    }
}
