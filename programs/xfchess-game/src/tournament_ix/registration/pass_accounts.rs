use crate::constants::XFCHESS_PASS_PROGRAM_ID;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

/// First 8 bytes of sha256("account:TournamentPassConfig") and sha256("account:EligibilityRecord"),
/// the Anchor discriminators of the pass program's two accounts.
const CONFIG_DISCRIMINATOR: [u8; 8] = [55, 121, 229, 33, 132, 84, 144, 61];
const RECORD_DISCRIMINATOR: [u8; 8] = [63, 254, 108, 207, 86, 176, 56, 170];
/// `TOURNAMENT_PASS_CONFIG_VERSION` in the pass program.
const CONFIG_VERSION: u8 = 1;
/// Account sizes: discriminator plus the fields of each pass-program account.
const CONFIG_LEN: usize = 100;
const RECORD_LEN: usize = 86;

/// The fields of the pass program's `TournamentPassConfig` that binding relies on. Layout after
/// the discriminator: version, bump, meta-list bump, mint, tournament id, registrar, min Elo,
/// max Elo, max age in slots, paused flag.
#[derive(Debug, PartialEq, Eq)]
pub struct PassConfigView {
    pub mint: Pubkey,
    pub tournament_id: u64,
    pub registrar: Pubkey,
    pub min_elo: u32,
    pub max_elo: u32,
}

/// The fields of the pass program's `EligibilityRecord` that registration relies on. Layout after
/// the discriminator: bump, mint, player, verified Elo, verified slot, revoked flag.
#[derive(Debug, PartialEq, Eq)]
pub struct EligibilityView {
    pub mint: Pubkey,
    pub player: Pubkey,
    pub is_revoked: bool,
}

fn pubkey_at(data: &[u8], offset: usize) -> Option<Pubkey> {
    Some(Pubkey::new_from_array(
        data.get(offset..offset.checked_add(32)?)?.try_into().ok()?,
    ))
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn u64_at(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(offset..offset.checked_add(8)?)?.try_into().ok()?,
    ))
}

pub fn parse_config(data: &[u8]) -> Option<PassConfigView> {
    if data.len() != CONFIG_LEN
        || data[..8] != CONFIG_DISCRIMINATOR
        || data[8] != CONFIG_VERSION
    {
        return None;
    }
    Some(PassConfigView {
        mint: pubkey_at(data, 11)?,
        tournament_id: u64_at(data, 43)?,
        registrar: pubkey_at(data, 51)?,
        min_elo: u32_at(data, 83)?,
        max_elo: u32_at(data, 87)?,
    })
}

pub fn parse_record(data: &[u8]) -> Option<EligibilityView> {
    if data.len() != RECORD_LEN || data[..8] != RECORD_DISCRIMINATOR {
        return None;
    }
    Some(EligibilityView {
        mint: pubkey_at(data, 9)?,
        player: pubkey_at(data, 41)?,
        is_revoked: *data.get(85)? != 0,
    })
}

/// Reads the pass program's config account. It must be owned by the pass program, so a lookalike
/// account owned by anyone else is refused.
pub fn load_config(info: &AccountInfo) -> Result<PassConfigView> {
    require_keys_eq!(
        *info.owner,
        XFCHESS_PASS_PROGRAM_ID,
        GameErrorCode::PassConfigInvalid
    );
    parse_config(&info.try_borrow_data()?).ok_or_else(|| error!(GameErrorCode::PassConfigInvalid))
}

/// Reads a wallet's eligibility record from the pass program, or `None` when it was never issued
/// (an empty, system-owned address).
pub fn load_record(info: &AccountInfo) -> Result<Option<EligibilityView>> {
    if info.owner == &System::id() && info.data_is_empty() {
        return Ok(None);
    }
    require_keys_eq!(
        *info.owner,
        XFCHESS_PASS_PROGRAM_ID,
        GameErrorCode::PassEligibilityMissing
    );
    parse_record(&info.try_borrow_data()?)
        .map(Some)
        .ok_or_else(|| error!(GameErrorCode::PassEligibilityMissing))
}

/// A pass can be bound only if the game program alone can attest wallets to it and the pass
/// program accepts every rating this tournament admits. The registrar and policy are fixed when
/// the pass is initialized, so they cannot be corrected after binding.
pub fn check_config(
    config: &PassConfigView,
    mint: &Pubkey,
    tournament_id: u64,
    registrar: &Pubkey,
    elo_min: u32,
    elo_max: u32,
) -> std::result::Result<(), GameErrorCode> {
    if config.mint != *mint || config.tournament_id != tournament_id {
        return Err(GameErrorCode::PassConfigInvalid);
    }
    if config.registrar != *registrar {
        return Err(GameErrorCode::PassRegistrarMismatch);
    }
    if config.min_elo > elo_min || config.max_elo < elo_max {
        return Err(GameErrorCode::PassPolicyMismatch);
    }
    Ok(())
}

/// A player may redeem a pass only while the pass program holds a current, unrevoked record for
/// them. That record exists only if this program attested the player (KYC-verified, rating in
/// range), so a pass that reached a wallet any other way, such as a mint before the authority was
/// revoked, cannot be used to register.
pub fn check_record(
    record: Option<&EligibilityView>,
    mint: &Pubkey,
    player: &Pubkey,
) -> std::result::Result<(), GameErrorCode> {
    let Some(record) = record else {
        return Err(GameErrorCode::PassEligibilityMissing);
    };
    if record.mint != *mint || record.player != *player {
        return Err(GameErrorCode::PassEligibilityMissing);
    }
    if record.is_revoked {
        return Err(GameErrorCode::PassEligibilityRevoked);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_bytes(
        mint: &Pubkey,
        tournament_id: u64,
        registrar: &Pubkey,
        min_elo: u32,
        max_elo: u32,
    ) -> Vec<u8> {
        let mut data = CONFIG_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&[CONFIG_VERSION, 255, 254]);
        data.extend_from_slice(mint.as_ref());
        data.extend_from_slice(&tournament_id.to_le_bytes());
        data.extend_from_slice(registrar.as_ref());
        data.extend_from_slice(&min_elo.to_le_bytes());
        data.extend_from_slice(&max_elo.to_le_bytes());
        data.extend_from_slice(&100u64.to_le_bytes());
        data.push(0);
        data
    }

    fn record_bytes(mint: &Pubkey, player: &Pubkey, is_revoked: bool) -> Vec<u8> {
        let mut data = RECORD_DISCRIMINATOR.to_vec();
        data.push(255);
        data.extend_from_slice(mint.as_ref());
        data.extend_from_slice(player.as_ref());
        data.extend_from_slice(&1_500u32.to_le_bytes());
        data.extend_from_slice(&7u64.to_le_bytes());
        data.push(u8::from(is_revoked));
        data
    }

    #[test]
    fn layouts_have_the_pass_programs_sizes() {
        let (mint, other) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert_eq!(config_bytes(&mint, 1, &other, 1, 2).len(), CONFIG_LEN);
        assert_eq!(record_bytes(&mint, &other, false).len(), RECORD_LEN);
    }

    #[test]
    fn config_fields_are_read_from_their_offsets() {
        let (mint, registrar) = (Pubkey::new_unique(), Pubkey::new_unique());
        let data = config_bytes(&mint, 9, &registrar, 1_400, 1_700);
        assert_eq!(
            parse_config(&data),
            Some(PassConfigView {
                mint,
                tournament_id: 9,
                registrar,
                min_elo: 1_400,
                max_elo: 1_700,
            })
        );
    }

    #[test]
    fn config_rejects_wrong_length_discriminator_and_version() {
        let (mint, registrar) = (Pubkey::new_unique(), Pubkey::new_unique());
        let data = config_bytes(&mint, 9, &registrar, 1_400, 1_700);
        assert!(parse_config(&data[..CONFIG_LEN - 1]).is_none());
        let mut longer = data.clone();
        longer.push(0);
        assert!(parse_config(&longer).is_none());
        let mut wrong_discriminator = data.clone();
        wrong_discriminator[0] ^= 1;
        assert!(parse_config(&wrong_discriminator).is_none());
        let mut wrong_version = data;
        wrong_version[8] = CONFIG_VERSION + 1;
        assert!(parse_config(&wrong_version).is_none());
    }

    #[test]
    fn record_is_not_mistaken_for_a_config_or_the_reverse() {
        let (mint, player) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert!(parse_config(&record_bytes(&mint, &player, false)).is_none());
        assert!(parse_record(&config_bytes(&mint, 1, &player, 1, 2)).is_none());
    }

    #[test]
    fn record_fields_are_read_from_their_offsets() {
        let (mint, player) = (Pubkey::new_unique(), Pubkey::new_unique());
        for is_revoked in [false, true] {
            assert_eq!(
                parse_record(&record_bytes(&mint, &player, is_revoked)),
                Some(EligibilityView {
                    mint,
                    player,
                    is_revoked,
                })
            );
        }
        assert!(parse_record(&record_bytes(&mint, &player, false)[..RECORD_LEN - 1]).is_none());
    }

    #[test]
    fn a_config_binds_only_for_this_mint_tournament_registrar_and_range() {
        let (mint, registrar) = (Pubkey::new_unique(), Pubkey::new_unique());
        let Some(config) = parse_config(&config_bytes(&mint, 9, &registrar, 1_400, 1_700)) else {
            panic!("config bytes parse");
        };
        let check = |mint: &Pubkey, id, registrar: &Pubkey, min, max| {
            check_config(&config, mint, id, registrar, min, max)
        };
        assert!(check(&mint, 9, &registrar, 1_400, 1_700).is_ok());
        assert!(check(&mint, 9, &registrar, 1_500, 1_600).is_ok());
        assert!(matches!(
            check(&Pubkey::new_unique(), 9, &registrar, 1_500, 1_600),
            Err(GameErrorCode::PassConfigInvalid)
        ));
        assert!(matches!(
            check(&mint, 10, &registrar, 1_500, 1_600),
            Err(GameErrorCode::PassConfigInvalid)
        ));
        assert!(matches!(
            check(&mint, 9, &Pubkey::new_unique(), 1_500, 1_600),
            Err(GameErrorCode::PassRegistrarMismatch)
        ));
        assert!(matches!(
            check(&mint, 9, &registrar, 1_399, 1_600),
            Err(GameErrorCode::PassPolicyMismatch)
        ));
        assert!(matches!(
            check(&mint, 9, &registrar, 1_500, 1_701),
            Err(GameErrorCode::PassPolicyMismatch)
        ));
    }

    #[test]
    fn only_an_unrevoked_record_for_this_mint_and_player_redeems() {
        let (mint, player) = (Pubkey::new_unique(), Pubkey::new_unique());
        let (Some(current), Some(revoked)) = (
            parse_record(&record_bytes(&mint, &player, false)),
            parse_record(&record_bytes(&mint, &player, true)),
        ) else {
            panic!("record bytes parse");
        };
        assert!(check_record(Some(&current), &mint, &player).is_ok());
        assert!(matches!(
            check_record(None, &mint, &player),
            Err(GameErrorCode::PassEligibilityMissing)
        ));
        assert!(matches!(
            check_record(Some(&revoked), &mint, &player),
            Err(GameErrorCode::PassEligibilityRevoked)
        ));
        assert!(matches!(
            check_record(Some(&current), &Pubkey::new_unique(), &player),
            Err(GameErrorCode::PassEligibilityMissing)
        ));
        assert!(matches!(
            check_record(Some(&current), &mint, &Pubkey::new_unique()),
            Err(GameErrorCode::PassEligibilityMissing)
        ));
    }
}
