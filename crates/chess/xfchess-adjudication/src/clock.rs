//! Integer clocks driven by a caller's monotonic, authenticated timestamp.
//! This module does not turn browser timestamps into an adjudication authority.
use crate::INCORRECT_CLAIM_PENALTY_MILLIS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockError {
    InvalidCheckpoint,
    InvalidSide,
    TimeWentBackwards,
    Paused,
    NotPaused,
    FlagFallen(i8),
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdjudicatedClock {
    remaining_millis: [u64; 2],
    increment_millis: u32,
    side_to_move: i8,
    updated_at_millis: u64,
    paused: bool,
    timed: bool,
}

fn index(side: i8) -> Result<usize, ClockError> {
    match side {
        1 => Ok(0),
        -1 => Ok(1),
        _ => Err(ClockError::InvalidSide),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockCheckpoint {
    pub remaining_millis: [u64; 2],
    pub increment_millis: u32,
    pub side_to_move: i8,
    pub updated_at_millis: u64,
    pub paused: bool,
    pub timed: bool,
}

impl AdjudicatedClock {
    pub fn checkpoint(&self) -> ClockCheckpoint {
        ClockCheckpoint {
            remaining_millis: self.remaining_millis,
            increment_millis: self.increment_millis,
            side_to_move: self.side_to_move,
            updated_at_millis: self.updated_at_millis,
            paused: self.paused,
            timed: self.timed,
        }
    }

    pub fn restore(checkpoint: ClockCheckpoint) -> Result<Self, ClockError> {
        index(checkpoint.side_to_move)?;
        if !checkpoint.timed && checkpoint.remaining_millis != [0; 2] {
            return Err(ClockError::InvalidCheckpoint);
        }
        Ok(Self {
            remaining_millis: checkpoint.remaining_millis,
            increment_millis: checkpoint.increment_millis,
            side_to_move: checkpoint.side_to_move,
            updated_at_millis: checkpoint.updated_at_millis,
            paused: checkpoint.paused,
            timed: checkpoint.timed,
        })
    }
    pub fn new(
        base_millis: u64,
        increment_millis: u32,
        side: i8,
        now: u64,
    ) -> Result<Self, ClockError> {
        index(side)?;
        Ok(Self {
            remaining_millis: [base_millis; 2],
            increment_millis,
            side_to_move: side,
            updated_at_millis: now,
            paused: false,
            timed: base_millis != 0,
        })
    }

    pub fn side_to_move(&self) -> i8 {
        self.side_to_move
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn remaining(&self, side: i8) -> Result<u64, ClockError> {
        Ok(self.remaining_millis[index(side)?])
    }

    fn elapsed(&self, now: u64) -> Result<u64, ClockError> {
        now.checked_sub(self.updated_at_millis)
            .ok_or(ClockError::TimeWentBackwards)
    }

    /// Check expiry before accepting a move, resignation or new claim.
    pub fn flag_fallen(&self, now: u64) -> Result<Option<i8>, ClockError> {
        let elapsed = self.elapsed(now)?;
        Ok(
            if self.timed && !self.paused && elapsed >= self.remaining(self.side_to_move)? {
                Some(self.side_to_move)
            } else {
                None
            },
        )
    }

    pub fn complete_move(&mut self, now: u64) -> Result<(), ClockError> {
        if self.paused {
            return Err(ClockError::Paused);
        }
        let elapsed = self.elapsed(now)?;
        if let Some(side) = self.flag_fallen(now)? {
            return Err(ClockError::FlagFallen(side));
        }
        let idx = index(self.side_to_move)?;
        let remaining = if self.timed {
            (self.remaining_millis[idx] - elapsed)
                .checked_add(u64::from(self.increment_millis))
                .ok_or(ClockError::Overflow)?
        } else {
            0
        };
        self.remaining_millis[idx] = remaining;
        self.side_to_move = -self.side_to_move;
        self.updated_at_millis = now;
        Ok(())
    }

    /// Formal receipt, not UI preview: stop both clocks before adjudication.
    pub fn pause_for_claim(&mut self, now: u64) -> Result<(), ClockError> {
        if self.paused {
            return Err(ClockError::Paused);
        }
        let elapsed = self.elapsed(now)?;
        if let Some(side) = self.flag_fallen(now)? {
            return Err(ClockError::FlagFallen(side));
        }
        let idx = index(self.side_to_move)?;
        if self.timed {
            self.remaining_millis[idx] -= elapsed;
        }
        self.updated_at_millis = now;
        self.paused = true;
        Ok(())
    }

    /// Exactly-once callers use ClaimProcedure. No increment accrues for a claim.
    pub(crate) fn reject_claim_and_resume(&mut self, now: u64) -> Result<(), ClockError> {
        self.check_claim_resolution(now)?;
        let opponent = index(-self.side_to_move)?;
        let adjusted = if self.timed {
            self.remaining_millis[opponent]
                .checked_add(INCORRECT_CLAIM_PENALTY_MILLIS)
                .ok_or(ClockError::Overflow)?
        } else {
            0
        };
        self.remaining_millis[opponent] = adjusted;
        self.updated_at_millis = now;
        self.paused = false;
        Ok(())
    }

    pub(crate) fn check_claim_resolution(&self, now: u64) -> Result<(), ClockError> {
        if !self.paused {
            return Err(ClockError::NotPaused);
        }
        self.elapsed(now)?;
        Ok(())
    }
}
