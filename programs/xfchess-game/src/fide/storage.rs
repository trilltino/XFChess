use super::*;
use anchor_lang::prelude::*;
use crate::{errors::GameErrorCode as E, state::{Game, GameStatus}};
use nimzovich_engine::{adjudication::*, on_chain_rules::canonical_position_key, CompactBoard};

pub const FIDE_MAGIC: &[u8; 8] = b"XFIDE001";

pub fn timestamp_millis(now: i64) -> Result<u64> {
    u64::try_from(now).ok().and_then(|value| value.checked_mul(1000))
        .ok_or_else(|| error!(E::InvalidFideClock))
}

pub fn rules_error(_: impl core::fmt::Debug) -> anchor_lang::error::Error {
    error!(E::InvalidFideState)
}

pub fn game_claim_id(id: u64) -> [u8; 32] {
    let mut out = crate::ID.to_bytes();
    out[..8].copy_from_slice(&id.to_le_bytes());
    out
}

/// Borrow account bytes directly. No history allocation, large stack frame,
/// companion account or untrusted caller-supplied occurrence count is needed.
pub struct FideState<'a> { pub bytes: &'a mut [u8] }

impl<'a> FideState<'a> {
    pub fn load(data: &'a mut [u8], game: &Game) -> Result<Self> {
        require!(data.len() == FIDE_GAME_SPACE, E::LegacyGameRules);
        let view = Self { bytes: &mut data[LEGACY_GAME_SPACE..] };
        require!(&view.bytes[..8] == FIDE_MAGIC && view.bytes[8] == RULES_VERSION, E::InvalidFideState);
        require!(view.u64(57)? == game.game_id, E::InvalidFideState);
        require!(view.bytes[9] <= 1 && view.bytes[11] <= 1 && view.bytes[65] <= 1, E::InvalidFideState);
        require!(view.u16(49)? >= 1 && view.u16(49)? <= 151, E::InvalidFideState);
        require!(view.u32(51)? == u32::from(game.move_count) && view.u16(55)? == game.halfmove_clock, E::InvalidFideState);
        Ok(view)
    }

    pub fn initialize(data: &'a mut [u8], game: &Game) -> Result<()> {
        require!(data.len() == FIDE_GAME_SPACE && game.move_count == 0, E::InvalidFideState);
        let bytes = &mut data[LEGACY_GAME_SPACE..];
        bytes.fill(0);
        bytes[..8].copy_from_slice(FIDE_MAGIC);
        bytes[8] = RULES_VERSION;
        let mut state = Self { bytes };
        state.put_u64(57, game.game_id)?;
        state.put_u16(49, 1)?;
        let key = canonical_position_key(&CompactBoard::from_bytes(&game.board_state)).map_err(rules_error)?;
        state.bytes[FIDE_HEADER_SIZE..FIDE_HEADER_SIZE + 34].copy_from_slice(&key.to_bytes());
        if game.status == GameStatus::Active { state.start(game, game.updated_at)?; }
        Ok(())
    }

    pub fn start(&mut self, game: &Game, now: i64) -> Result<()> {
        require!(self.bytes[9] == 0 && game.move_count == 0 && game.status == GameStatus::Active, E::InvalidFideState);
        let base = game.base_time_seconds.checked_mul(1000).ok_or(E::ArithmeticOverflow)?;
        self.put_u64(25, base)?;
        self.put_u64(33, base)?;
        self.put_u64(41, timestamp_millis(now)?)?;
        self.bytes[9] = 1;
        Ok(())
    }

    pub fn clock(&self, game: &Game) -> Result<AdjudicatedClock> {
        require!(self.bytes[9] == 1, E::InvalidFideClock);
        AdjudicatedClock::restore(ClockCheckpoint {
            remaining_millis: [self.u64(25)?, self.u64(33)?],
            increment_millis: u32::from(game.increment_seconds) * 1000,
            side_to_move: if game.turn % 2 == 1 { 1 } else { -1 },
            updated_at_millis: self.u64(41)?, paused: self.bytes[65] != 0,
            timed: game.base_time_seconds != 0,
        }).map_err(rules_error)
    }

    pub fn save_clock(&mut self, clock: &AdjudicatedClock) -> Result<()> {
        let checkpoint = clock.checkpoint();
        self.put_u64(25, checkpoint.remaining_millis[0])?;
        self.put_u64(33, checkpoint.remaining_millis[1])?;
        self.put_u64(41, checkpoint.updated_at_millis)?;
        self.bytes[65] = u8::from(checkpoint.paused);
        Ok(())
    }

    pub fn tracker(&mut self) -> Result<RepetitionTracker<'_>> {
        let checkpoint = RepetitionCheckpoint { len: self.u16(49)?, ply: self.u32(51)?, halfmoves: self.u16(55)? };
        let keys: &mut [PositionKey] = bytemuck::try_cast_slice_mut(&mut self.bytes[FIDE_HEADER_SIZE..]).map_err(rules_error)?;
        for key in keys.iter().take(usize::from(checkpoint.len)) {
            PositionKey::from_bytes(key.to_bytes()).map_err(rules_error)?;
        }
        RepetitionTracker::restore(keys, checkpoint).map_err(rules_error)
    }

    pub fn save_checkpoint(&mut self, checkpoint: RepetitionCheckpoint) -> Result<()> {
        self.put_u16(49, checkpoint.len)?;
        self.put_u32(51, checkpoint.ply)?;
        self.put_u16(55, checkpoint.halfmoves)
    }

    pub fn claims(&self, game: &Game) -> Result<ClaimProcedure> {
        let mandatory = if self.bytes[11] == 0 { None } else {
            Some(self.bytes[12..17].try_into().map_err(rules_error)?)
        };
        ClaimProcedure::restore_completed(game_claim_id(game.game_id), self.u64(17)?, mandatory,
            None, game.draw_offered_by.map(|player| if player == game.white { 1 } else { -1 }))
            .map_err(rules_error)
    }

    pub fn save_claims(&mut self, claims: &ClaimProcedure) -> Result<()> {
        require!(claims.pending().is_none(), E::InvalidFideState);
        self.put_u64(17, claims.next_nonce())?;
        self.bytes[11] = u8::from(claims.mandatory_move().is_some());
        self.bytes[12..17].copy_from_slice(&claims.mandatory_move().unwrap_or([0; 5]));
        Ok(())
    }

    pub fn u16(&self, at: usize) -> Result<u16> { Ok(u16::from_le_bytes(self.bytes.get(at..at+2).ok_or(E::InvalidFideState)?.try_into().map_err(rules_error)?)) }
    pub fn u32(&self, at: usize) -> Result<u32> { Ok(u32::from_le_bytes(self.bytes.get(at..at+4).ok_or(E::InvalidFideState)?.try_into().map_err(rules_error)?)) }
    pub fn u64(&self, at: usize) -> Result<u64> { Ok(u64::from_le_bytes(self.bytes.get(at..at+8).ok_or(E::InvalidFideState)?.try_into().map_err(rules_error)?)) }
    pub fn put_u16(&mut self, at: usize, value: u16) -> Result<()> { self.bytes.get_mut(at..at+2).ok_or(E::InvalidFideState)?.copy_from_slice(&value.to_le_bytes()); Ok(()) }
    pub fn put_u32(&mut self, at: usize, value: u32) -> Result<()> { self.bytes.get_mut(at..at+4).ok_or(E::InvalidFideState)?.copy_from_slice(&value.to_le_bytes()); Ok(()) }
    pub fn put_u64(&mut self, at: usize, value: u64) -> Result<()> { self.bytes.get_mut(at..at+8).ok_or(E::InvalidFideState)?.copy_from_slice(&value.to_le_bytes()); Ok(()) }
}

pub fn start_account_clock(info: &AccountInfo<'_>, game: &Game, now: i64) -> Result<()> {
    FideState::load(&mut info.try_borrow_mut_data()?, game)?.start(game, now)
}
