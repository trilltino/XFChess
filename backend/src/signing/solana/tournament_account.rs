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

    fn skip_string(&mut self) -> Result<(), String> {
        let len = u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize;
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

    #[test]
    fn reads_bracket_match_seats_winner_and_status() {
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
            parse_bracket_match(&d).unwrap(),
            BracketMatchOnChain {
                player_white: Some(white),
                player_black: Some(black),
                winner: Some(black),
                status: MATCH_STATUS_COMPLETED,
            }
        );
        assert!(parse_bracket_match(&d[..20]).is_err());
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
    fn reads_round_progress_past_variable_length_fields() {
        for (name, start) in [("", None), ("Spring Swiss", Some(1_700_000_000))] {
            let p = parse_swiss_progress(&account(name, TOURNAMENT_STATUS_ACTIVE, start, 2, 5))
                .unwrap();
            assert_eq!(
                p,
                SwissChainProgress {
                    status: TOURNAMENT_STATUS_ACTIVE,
                    current_round: 2,
                    total_rounds: 5
                }
            );
        }
    }

    #[test]
    fn rejects_truncated_accounts() {
        let data = account("x", 1, None, 0, 3);
        assert!(parse_swiss_progress(&data[..60]).is_err());
    }
}
