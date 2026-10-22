use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

pub fn fetch_profile_elo(
    rpc: &RpcClient,
    program_id: Pubkey,
    player: Pubkey,
) -> anyhow::Result<f64> {
    let profile_pda = Pubkey::find_program_address(&[b"profile", player.as_ref()], &program_id).0;
    let account = rpc.get_account(&profile_pda)?;
    anyhow::ensure!(account.owner == program_id, "profile owner mismatch");
    profile_elo(&account.data)
}

fn profile_elo(data: &[u8]) -> anyhow::Result<f64> {
    check_discriminator(data, "PlayerProfile")?;
    let mut cursor = BorshCursor::new(data);
    cursor.take(8 + 32)?;
    let country_len = cursor.u32()? as usize;
    anyhow::ensure!(country_len <= 2, "invalid profile country length");
    cursor.take(country_len + 4 * 4)?;
    let elo = f64::from_le_bytes(cursor.take(8)?.try_into()?);
    anyhow::ensure!(elo.is_finite(), "invalid profile rating");
    Ok(elo)
}

fn check_discriminator(data: &[u8], name: &str) -> anyhow::Result<()> {
    let expected = Sha256::digest(format!("account:{name}"));
    anyhow::ensure!(
        data.get(..8) == Some(&expected[..8]),
        "{name} discriminator mismatch"
    );
    Ok(())
}

pub const TOURNAMENT_STATUS_COMPLETED: u8 = 2;

struct BorshCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BorshCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> anyhow::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| anyhow::anyhow!("borsh cursor: offset overflow"))?;
        let slice = self.data.get(self.pos..end).ok_or_else(|| {
            anyhow::anyhow!(
                "borsh cursor: out of bounds reading {n} bytes at {}",
                self.pos
            )
        })?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> anyhow::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> anyhow::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }

    fn u32(&mut self) -> anyhow::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }

    fn u64(&mut self) -> anyhow::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
    }

    fn i64(&mut self) -> anyhow::Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into()?))
    }

    fn pubkey(&mut self) -> anyhow::Result<Pubkey> {
        Ok(Pubkey::new_from_array(self.take(32)?.try_into()?))
    }

    fn string(&mut self) -> anyhow::Result<()> {
        let len = self.u32()? as usize;
        self.take(len)?;
        Ok(())
    }

    fn option_i64(&mut self) -> anyhow::Result<Option<i64>> {
        if self.u8()? == 1 {
            Ok(Some(self.i64()?))
        } else {
            Ok(None)
        }
    }

    fn option_pubkey(&mut self) -> anyhow::Result<Option<Pubkey>> {
        if self.u8()? == 1 {
            Ok(Some(self.pubkey()?))
        } else {
            Ok(None)
        }
    }
}

pub fn fetch_tournament_places(
    rpc: &RpcClient,
    program_id: Pubkey,
    tournament_id: u64,
) -> anyhow::Result<(u8, [Option<Pubkey>; 3])> {
    let tournament_pda =
        Pubkey::find_program_address(&[b"tournament", &tournament_id.to_le_bytes()], &program_id).0;
    let account = rpc.get_account(&tournament_pda)?;
    anyhow::ensure!(account.owner == program_id, "tournament owner mismatch");
    let data = account.data;
    check_discriminator(&data, "Tournament")?;
    let mut c = BorshCursor::new(&data);

    c.take(8)?; // Anchor account discriminator
    c.u64()?; // tournament_id
    c.pubkey()?; // authority
    c.string()?; // name
    c.u64()?; // entry_fee
    c.u64()?; // platform_fee
    c.u64()?; // prize_pool
    c.u16()?; // max_players
    c.u16()?; // player_count
    c.u16()?; // num_registered_players
    let status = c.u8()?; // status (TournamentStatus)
    c.option_i64()?; // start_time
    c.option_i64()?; // end_time
    c.u64()?; // fees_advanced
    c.pubkey()?; // fee_payer
    let type_tag = c.u8()?; // tournament_type discriminant (0 = Swiss, 1 = SingleElimination)
    anyhow::ensure!(type_tag <= 1, "invalid tournament type");
    if type_tag == 0 {
        c.u8()?; // Swiss { rounds }
    }
    c.u8()?; // current_round
    c.u8()?; // total_rounds
    c.u16()?; // total_matches
    c.u16()?; // final_match_index
    c.u32()?; // elo_min
    c.u32()?; // elo_max
    c.u16()?; // min_players
    let winner = c.option_pubkey()?;
    let second_place = c.option_pubkey()?;
    let third_place = c.option_pubkey()?;

    Ok((status, [winner, second_place, third_place]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn profile_rating_follows_variable_length_country() -> anyhow::Result<()> {
        for country in ["", "GB"] {
            let mut data = Sha256::digest("account:PlayerProfile")[..8].to_vec();
            data.extend([0; 32]);
            data.extend((country.len() as u32).to_le_bytes());
            data.extend(country.as_bytes());
            data.extend([0; 16]);
            data.extend(1400f64.to_le_bytes());
            assert_eq!(
                profile_elo(&data).context(concat!(file!(), ":", line!()))?,
                1400.0
            );
            assert!(profile_elo(&data[..data.len() - 1]).is_err());
            data[0] ^= 1;
            assert!(profile_elo(&data).is_err());
        }
        Ok(())
    }
}
