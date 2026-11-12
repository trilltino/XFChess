//! Minimal reader for the on-chain `Tournament` account: just the fields the Swiss
//! chain reconciler needs (`status`, `current_round`, `total_rounds`).

pub const TOURNAMENT_STATUS_ACTIVE: u8 = 1;
pub const TOURNAMENT_STATUS_COMPLETED: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwissChainProgress {
    pub status: u8,
    /// 0-based round currently being played; equals `total_rounds` once every round
    /// has been advanced.
    pub current_round: u8,
    pub total_rounds: u8,
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .ok_or("tournament account overflow")?;
        let slice = self
            .data
            .get(self.at..end)
            .ok_or("tournament account truncated")?;
        self.at = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn skip_option(&mut self, inner: usize) -> Result<(), String> {
        if self.u8()? == 1 {
            self.take(inner)?;
        }
        Ok(())
    }

    fn u16(&mut self) -> Result<u16, String> {
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| "short u16")?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn option_pubkey(&mut self) -> Result<Option<solana_sdk::pubkey::Pubkey>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => solana_sdk::pubkey::Pubkey::try_from(self.take(32)?)
                .map(Some)
                .map_err(|e| format!("bad pubkey: {e}")),
            tag => Err(format!("invalid option tag {tag}")),
        }
    }

    fn skip_string(&mut self) -> Result<(), String> {
        let len = u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| "short string length")?,
        ) as usize;
        self.take(len)?;
        Ok(())
    }
}

pub fn parse_swiss_progress(data: &[u8]) -> Result<SwissChainProgress, String> {
    let mut c = Cursor { data, at: 0 };
    c.take(8)?; // Anchor discriminator
    c.take(8)?; // tournament_id
    c.take(32)?; // authority
    c.skip_string()?; // name
    c.take(8 * 3)?; // entry_fee, platform_fee, prize_pool
    c.take(2 * 3)?; // max_players, player_count, num_registered_players
    let status = c.u8()?;
    c.skip_option(8)?; // start_time
    c.skip_option(8)?; // end_time
    c.take(8)?; // fees_advanced
    c.take(32)?; // fee_payer
    match c.u8()? {
        0 => {
            c.u8()?; // Swiss { rounds }
        }
        1 => return Err("tournament is not Swiss".into()),
        tag => return Err(format!("invalid tournament type tag {tag}")),
    }
    let current_round = c.u8()?;
    let total_rounds = c.u8()?;
    Ok(SwissChainProgress {
        status,
        current_round,
        total_rounds,
    })
}

/// The prize fields of an on-chain `Tournament`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrizeLedger {
    pub status: u8,
    pub places: [Option<solana_sdk::pubkey::Pubkey>; 10],
    pub prize_shares: [u16; 10],
    pub prizes_claimed: u16,
}

impl PrizeLedger {
    /// The wallets `distribute_tournament_prizes` requires, in place order:
    /// every placed, funded place not yet claimed. The program rejects any
    /// other list, so a place cannot be skipped or added.
    pub fn unpaid_wallets(&self) -> Vec<solana_sdk::pubkey::Pubkey> {
        self.places
            .iter()
            .zip(self.prize_shares)
            .enumerate()
            .filter_map(|(i, (place, share))| {
                let claimed = self.prizes_claimed & (1u16 << i) != 0;
                (share > 0 && !claimed).then_some(*place).flatten()
            })
            .collect()
    }
}

pub fn parse_prize_ledger(data: &[u8]) -> Result<PrizeLedger, String> {
    let mut c = Cursor { data, at: 0 };
    c.take(8)?; // Anchor discriminator
    c.take(8)?; // tournament_id
    c.take(32)?; // authority
    c.skip_string()?; // name
    c.take(8 * 3)?; // entry_fee, platform_fee, prize_pool
    c.take(2 * 3)?; // max_players, player_count, num_registered_players
    let status = c.u8()?;
    c.skip_option(8)?; // start_time
    c.skip_option(8)?; // end_time
    c.take(8)?; // fees_advanced
    c.take(32)?; // fee_payer
    match c.u8()? {
        0 => {
            c.u8()?; // Swiss { rounds }
        }
        1 => {} // SingleElimination
        tag => return Err(format!("invalid tournament type tag {tag}")),
    }
    c.take(1 + 1 + 2 + 2)?; // current_round, total_rounds, total_matches, final_match_index
    c.take(4 + 4 + 2)?; // elo_min, elo_max, min_players
    let mut places = [None; 10];
    for place in &mut places {
        *place = c.option_pubkey()?;
    }
    let mut prize_shares = [0u16; 10];
    for share in &mut prize_shares {
        *share = c.u16()?;
    }
    c.take(8)?; // created_at
    c.skip_option(8)?; // started_at
    c.skip_option(8)?; // completed_at
    c.take(1)?; // bump
    let prizes_claimed = c.u16()?;
    Ok(PrizeLedger {
        status,
        places,
        prize_shares,
        prizes_claimed,
    })
}

pub fn fetch_prize_ledger(
    rpc: &solana_client::rpc_client::RpcClient,
    program_id: &solana_sdk::pubkey::Pubkey,
    tournament_id: u64,
) -> Result<PrizeLedger, String> {
    let pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[super::TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let account = rpc
        .get_account(&pda)
        .map_err(|e| format!("fetch tournament {tournament_id}: {e}"))?;
    if account.owner != *program_id {
        return Err(format!("tournament {tournament_id} has the wrong owner"));
    }
    parse_prize_ledger(&account.data)
}

pub fn fetch_swiss_progress(
    rpc: &solana_client::rpc_client::RpcClient,
    program_id: &solana_sdk::pubkey::Pubkey,
    tournament_id: u64,
) -> Result<SwissChainProgress, String> {
    let pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[super::TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        program_id,
    )
    .0;
    let account = rpc
        .get_account(&pda)
        .map_err(|e| format!("fetch tournament {tournament_id}: {e}"))?;
    if account.owner != *program_id {
        return Err(format!("tournament {tournament_id} has the wrong owner"));
    }
    parse_swiss_progress(&account.data)
}

/// On-chain `MatchStatus::Completed`.
pub const MATCH_STATUS_COMPLETED: u8 = 2;

/// The fields of an on-chain bracket `TournamentMatch` the chain mirror checks
/// before writing, so a retried mirror never re-sends a landed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketMatchOnChain {
    pub player_white: Option<solana_sdk::pubkey::Pubkey>,
    pub player_black: Option<solana_sdk::pubkey::Pubkey>,
    pub winner: Option<solana_sdk::pubkey::Pubkey>,
    pub status: u8,
}

impl Cursor<'_> {
    fn pubkey_option(&mut self) -> Result<Option<solana_sdk::pubkey::Pubkey>, String> {
        if self.u8()? == 0 {
            return Ok(None);
        }
        let bytes: [u8; 32] = self
            .take(32)?
            .try_into()
            .map_err(|_| "pubkey is 32 bytes")?;
        Ok(Some(solana_sdk::pubkey::Pubkey::new_from_array(bytes)))
    }
}

pub fn parse_bracket_match(data: &[u8]) -> Result<BracketMatchOnChain, String> {
    let mut c = Cursor { data, at: 0 };
    c.take(8)?; // Anchor discriminator
    c.take(8 + 2 + 1)?; // tournament_id, match_index, round
    let player_white = c.pubkey_option()?;
    let player_black = c.pubkey_option()?;
    let winner = c.pubkey_option()?;
    c.skip_option(32)?; // game_pda
    c.skip_option(8)?; // game_id
    let status = c.u8()?;
    Ok(BracketMatchOnChain {
        player_white,
        player_black,
        winner,
        status,
    })
}

pub fn fetch_bracket_match(
    rpc: &solana_client::rpc_client::RpcClient,
    program_id: &solana_sdk::pubkey::Pubkey,
    tournament_id: u64,
    match_index: u16,
) -> Result<BracketMatchOnChain, String> {
    let pda = solana_sdk::pubkey::Pubkey::find_program_address(
        &[
            super::instructions::TOURNAMENT_MATCH_SEED,
            &tournament_id.to_le_bytes(),
            &match_index.to_le_bytes(),
        ],
        program_id,
    )
    .0;
    let account = rpc
        .get_account(&pda)
        .map_err(|e| format!("fetch match {match_index} of tournament {tournament_id}: {e}"))?;
    if account.owner != *program_id {
        return Err(format!("match {match_index} has the wrong owner"));
    }
    parse_bracket_match(&account.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    #[test]
    fn reads_bracket_match_seats_winner_and_status() -> anyhow::Result<()> {
        use solana_sdk::pubkey::Pubkey;
        let (white, black) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut d = vec![0u8; 8 + 8 + 2 + 1];
        for seat in [Some(white), Some(black), Some(black), None] {
            match seat {
                Some(key) => {
                    d.push(1);
                    d.extend(key.to_bytes());
                }
                None => d.push(0),
            }
        }
        d.extend([1, 9, 0, 0, 0, 0, 0, 0, 0]); // game_id: Some(9)
        d.push(MATCH_STATUS_COMPLETED);
        d.extend([0u8; 16]);
        assert_eq!(
            parse_bracket_match(&d)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("test setup failed")?,
            BracketMatchOnChain {
                player_white: Some(white),
                player_black: Some(black),
                winner: Some(black),
                status: MATCH_STATUS_COMPLETED,
            }
        );
        assert!(parse_bracket_match(&d[..20]).is_err());
        Ok(())
    }

    fn account(name: &str, status: u8, start: Option<i64>, current: u8, total: u8) -> Vec<u8> {
        let mut d = vec![0u8; 8 + 8 + 32];
        d.extend((name.len() as u32).to_le_bytes());
        d.extend(name.as_bytes());
        d.extend([0u8; 24 + 6]);
        d.push(status);
        match start {
            Some(t) => {
                d.push(1);
                d.extend(t.to_le_bytes());
            }
            None => d.push(0),
        }
        d.push(0); // end_time
        d.extend([0u8; 8 + 32]);
        d.extend([0, total]); // Swiss { rounds }
        d.extend([current, total]);
        d.extend([0u8; 64]);
        d
    }

    #[test]
    fn reads_round_progress_past_variable_length_fields() -> anyhow::Result<()> {
        for (name, start) in [("", None), ("Spring Swiss", Some(1_700_000_000))] {
            let p = parse_swiss_progress(&account(name, TOURNAMENT_STATUS_ACTIVE, start, 2, 5))
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("test setup failed")?;
            assert_eq!(
                p,
                SwissChainProgress {
                    status: TOURNAMENT_STATUS_ACTIVE,
                    current_round: 2,
                    total_rounds: 5
                }
            );
        }
        Ok(())
    }

    #[test]
    fn prize_ledger_matches_the_program_layout_and_selection() -> anyhow::Result<()> {
        use anchor_lang::AccountSerialize;
        use solana_sdk::pubkey::Pubkey;
        use xfchess_game::state::{
            PayoutType, Tournament, TournamentStatus, TournamentType, VestingParams,
        };

        let players: Vec<Pubkey> = (0..4).map(|_| Pubkey::new_unique()).collect();
        let tournament = Tournament {
            tournament_id: 7,
            authority: Pubkey::new_unique(),
            name: "Autumn Open".into(),
            entry_fee: 1,
            platform_fee: 2,
            prize_pool: 3,
            max_players: 8,
            player_count: 4,
            num_registered_players: 4,
            status: TournamentStatus::Completed,
            start_time: Some(10),
            end_time: None,
            fees_advanced: 4,
            fee_payer: Pubkey::new_unique(),
            tournament_type: TournamentType::Swiss { rounds: 3 },
            current_round: 3,
            total_rounds: 3,
            total_matches: 6,
            final_match_index: 0,
            elo_min: 0,
            elo_max: u32::MAX,
            min_players: 2,
            winner: Some(players[0]),
            second_place: Some(players[1]),
            third_place: Some(players[2]),
            fourth_place: Some(players[3]),
            fifth_place: None,
            sixth_place: None,
            seventh_place: None,
            eighth_place: None,
            ninth_place: None,
            tenth_place: None,
            prize_shares: [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
            created_at: 1,
            started_at: Some(2),
            completed_at: Some(3),
            bump: 254,
            prizes_claimed: 0b10, // second place already claimed
            platform_fee_pool: 0,
            usdc_prize_mint: None,
            usdc_prize_pool: 0,
            usdc_prize_funded: false,
            host_treasury: Pubkey::new_unique(),
            prize_token_mint: None,
            payout_type: PayoutType::LumpSum,
            vesting_params: Some(VestingParams {
                start_time: 0,
                duration_seconds: 0,
                cliff_seconds: None,
            }),
            base_time_seconds: 300,
            increment_seconds: 0,
            winner_takes_all: false,
            round_boards_reported: [0; 16],
        };
        let mut data = Vec::new();
        tournament
            .try_serialize(&mut data)
            .context("test setup failed")?;

        let ledger = parse_prize_ledger(&data)
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("test setup failed")?;
        assert_eq!(ledger.status, TOURNAMENT_STATUS_COMPLETED);
        assert_eq!(ledger.prize_shares, tournament.prize_shares);
        assert_eq!(ledger.prizes_claimed, 0b10);
        // Fourth place is placed but unfunded; second has claimed already.
        assert_eq!(ledger.unpaid_wallets(), vec![players[0], players[2]]);
        Ok(())
    }

    #[test]
    fn rejects_truncated_accounts() {
        let data = account("x", 1, None, 0, 3);
        assert!(parse_swiss_progress(&data[..60]).is_err());
    }
}
