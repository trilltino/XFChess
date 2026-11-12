use super::*;

#[derive(Resource, Debug, Clone, Default)]
pub struct GameOverPayoutInfo {
    pub wager_amount: u64,
    pub country_fee: u64,
    pub elo_fee: u64,
    pub rent_return: u64,
    pub operating_cost: u64,
    pub winning_prize: u64,
    pub is_draw: bool,

    // Identity
    pub player_color: Option<PieceColor>,
    pub local_player_pubkey: Option<String>,
    pub game_id: u64,

    // Settlement
    pub payout_confirmed: bool,
    /// Why settlement is not confirmed yet, once the client's own
    /// finalization attempt has finished without on-chain proof.
    pub settlement_pending_reason: Option<String>,
    pub fee_breakdown_confirmed: bool,
    pub finalize_sig: Option<String>,
    pub game_ended_at: Option<std::time::Instant>,
    pub settlement_started_at: Option<std::time::Instant>,
    pub dispute_pending: bool,
    pub dispute_sig: Option<String>,

    // ELO / rating
    pub is_rated: bool,
    pub elo_before: u32,
    pub elo_after: u32,
    pub win_streak: u32,

    pub tournament_id: Option<u64>,
    pub usdc_mint: Option<String>,
    pub usdc_prize_usdc: Option<u64>,
}

impl GameOverPayoutInfo {
    pub(super) fn format_sol_usd(lamports: u64, usd_per_sol: Option<f64>) -> String {
        let sol = lamports as f64 / 1_000_000_000.0;
        let sol_str = Self::format_sol_dynamic(sol);
        match usd_per_sol.filter(|r| *r > 0.0) {
            Some(rate) => format!("${:.2} (≈{})", sol * rate, sol_str),
            None => sol_str,
        }
    }

    pub(super) fn format_sol_dynamic(sol: f64) -> String {
        if sol == 0.0 {
            "0 SOL".to_string()
        } else if sol.abs() >= 0.001 {
            format!("{:.3} SOL", sol)
        } else {
            format!("{:.6} SOL", sol)
        }
    }

    pub fn is_wager_game(&self) -> bool {
        self.wager_amount > 0
    }

    pub fn player_winnings(&self) -> u64 {
        if self.is_draw {
            self.wager_amount
                .saturating_sub(self.country_fee / 2)
                .saturating_sub(self.elo_fee / 2)
        } else {
            self.winning_prize
        }
    }

    pub fn dispute_window_open(&self) -> bool {
        match self.game_ended_at {
            Some(t) => t.elapsed().as_secs() < 48 * 3600,
            None => false,
        }
    }

    pub fn settlement_elapsed(&self) -> u64 {
        self.settlement_started_at
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
    }

    pub fn elo_delta(&self) -> i32 {
        if self.elo_before == 0 || self.elo_after == 0 {
            return 0;
        }
        self.elo_after as i32 - self.elo_before as i32
    }
}

#[derive(Resource)]
pub struct PendingDispute {
    pub game_id: u64,
    pub sig_rx: Option<crossbeam_channel::Receiver<String>>,
}

pub fn apply_dispute_trigger(
    mut commands: Commands,
    mut dispute: Option<ResMut<PendingDispute>>,
    #[cfg(feature = "solana")] rollup: Option<
        Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    >,
    mut payout_info: Option<ResMut<GameOverPayoutInfo>>,
) {
    let Some(ref mut dispute) = dispute else {
        return;
    };

    if let Some(ref rx) = dispute.sig_rx {
        if let Ok(sig) = rx.try_recv() {
            if let Some(ref mut info) = payout_info {
                info.dispute_sig = Some(sig);
                info.dispute_pending = false;
            }
            commands.remove_resource::<PendingDispute>();
            return;
        }
        return;
    }

    #[cfg(feature = "solana")]
    let game_id = if let Some(mgr) = rollup {
        if mgr.game_id != 0 {
            mgr.game_id
        } else {
            dispute.game_id
        }
    } else {
        dispute.game_id
    };
    #[cfg(not(feature = "solana"))]
    let game_id = dispute.game_id;

    let Some(ref mut info) = payout_info else {
        commands.remove_resource::<PendingDispute>();
        return;
    };
    if info.dispute_pending {
        return;
    }
    info.dispute_pending = true;
    let player = info.local_player_pubkey.clone().unwrap_or_default();

    let (sig_tx, sig_rx) = crossbeam_channel::bounded::<String>(1);
    dispute.sig_rx = Some(sig_rx);

    crate::core::net_task::spawn_detached("game_over_popup", move || {
        use crate::multiplayer::vps_client;
        match vps_client::vps_submit_dispute(game_id, &player) {
            Ok(sig) => {
                let _ = sig_tx.send(sig);
            }
            Err(e) => error!("[DISPUTE] Failed for game {}: {e}", game_id),
        }
    });
}

#[cfg(feature = "solana")]
pub fn fetch_game_payout_info(
    game_over: Res<GameOverState>,
    solana_sync: Option<Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut payout_info: ResMut<GameOverPayoutInfo>,
    current_turn: Option<Res<crate::game::resources::CurrentTurn>>,
) {
    if !game_over.is_game_over() {
        return;
    }

    *payout_info = GameOverPayoutInfo::default();

    if let Some(sync) = solana_sync {
        if sync.wager_amount > 0 {
            payout_info.wager_amount = sync.wager_amount;
            payout_info.is_rated = true;

            if let Some(_comp) = competitive {
                // Show estimates until finalization supplies confirmed fees. Keep them
                // aligned with backend country-fee estimates and ELO_FEE_LAMPORTS.
                const ELO_FEE_LAMPORTS_ESTIMATE: u64 = 5_000;
                const COUNTRY_FEE_BPS_ESTIMATE: u64 = 100; // 1%
                const OPERATING_COST_ESTIMATE: u64 = 320_000; // ~DELEGATE+UNDELEGATE+ER session fee
                let total_pot = sync.wager_amount * 2;
                payout_info.country_fee = total_pot * COUNTRY_FEE_BPS_ESTIMATE / 10_000;
                payout_info.elo_fee = ELO_FEE_LAMPORTS_ESTIMATE;
                payout_info.operating_cost = OPERATING_COST_ESTIMATE;
                payout_info.rent_return = 2_280_000;
                payout_info.fee_breakdown_confirmed = false;
                payout_info.winning_prize = total_pot
                    .saturating_sub(payout_info.country_fee)
                    .saturating_sub(payout_info.elo_fee);
            }

            payout_info.is_draw = game_over.winner().is_none();
            payout_info.settlement_started_at = Some(std::time::Instant::now());
            payout_info.game_ended_at = Some(std::time::Instant::now());

            if let Some(turn) = current_turn {
                payout_info.player_color = Some(turn.color);
            }
        }
    }
}

pub(super) fn open_url(url: &str) {
    let url = url.to_string();
    crate::core::net_task::spawn_detached("game_over_popup", move || {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &url])
            .spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    });
}
