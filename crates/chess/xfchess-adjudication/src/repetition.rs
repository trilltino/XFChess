use crate::{
    claim_availability, ClaimAvailability, PositionKey, RulesError, SEVENTY_FIVE_MOVE_HALFMOVES,
};

/// At most 150 reversible accepted plies can precede automatic termination.
/// Retain the initial position as well. Never evict a still-relevant position.
pub const MAX_REVERSIBLE_POSITIONS: usize = 151;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepetitionCheckpoint {
    pub len: u16,
    pub ply: u32,
    pub halfmoves: u16,
}

/// Borrows bounded caller-owned storage. No heap or 5 KiB stack temporary is
/// required by this API; account allocation/layout remain the caller's concern.
/// Checkpoints must come from authenticated state or verified legal replay.
pub struct RepetitionTracker<'a> {
    positions: &'a mut [PositionKey],
    checkpoint: RepetitionCheckpoint,
}

impl<'a> RepetitionTracker<'a> {
    pub fn from_start_position(
        positions: &'a mut [PositionKey],
        key: PositionKey,
        starting_ply: u32,
        halfmoves: u16,
    ) -> Result<Self, RulesError> {
        if positions.is_empty() || positions.len() > MAX_REVERSIBLE_POSITIONS {
            return Err(RulesError::Capacity);
        }
        if halfmoves > SEVENTY_FIVE_MOVE_HALFMOVES {
            return Err(RulesError::InvalidCheckpoint);
        }
        positions[0] = key;
        Ok(Self {
            positions,
            checkpoint: RepetitionCheckpoint {
                len: 1,
                ply: starting_ply,
                halfmoves,
            },
        })
    }

    /// A snapshot initializes one known occurrence. It never proves missing
    /// earlier positions; callers must gate historical claims accordingly.
    pub fn restore(
        positions: &'a mut [PositionKey],
        checkpoint: RepetitionCheckpoint,
    ) -> Result<Self, RulesError> {
        let len = usize::from(checkpoint.len);
        if positions.len() > MAX_REVERSIBLE_POSITIONS
            || len == 0
            || len > positions.len()
            || checkpoint.halfmoves > SEVENTY_FIVE_MOVE_HALFMOVES
            || u32::from(checkpoint.len - 1) > checkpoint.ply
            || checkpoint.len - 1 > checkpoint.halfmoves
        {
            return Err(RulesError::InvalidCheckpoint);
        }
        // Consecutive keys alternate turns and can only lose castling rights.
        for pair in positions[..len].windows(2) {
            if pair[0].side_to_move() == pair[1].side_to_move()
                || pair[1].castling() & !pair[0].castling() != 0
            {
                return Err(RulesError::InvalidCheckpoint);
            }
        }
        Ok(Self {
            positions,
            checkpoint,
        })
    }

    pub fn checkpoint(&self) -> RepetitionCheckpoint {
        self.checkpoint
    }

    pub fn current_key(&self) -> PositionKey {
        // Constructors and all updates preserve 1 <= len <= positions.len().
        self.positions[usize::from(self.checkpoint.len) - 1]
    }

    pub fn occurrences(&self, key: PositionKey) -> u16 {
        self.positions[..usize::from(self.checkpoint.len)]
            .iter()
            .filter(|&&entry| entry == key)
            .count() as u16
    }

    pub fn current_claims(&self) -> ClaimAvailability {
        claim_availability(
            self.occurrences(self.current_key()),
            self.checkpoint.halfmoves,
        )
    }

    /// The chess adapter must simulate a *legal* intended move first. This
    /// read-only query neither commits that move nor imposes a formal penalty.
    pub fn prospective_claims(
        &self,
        key: PositionKey,
        pawn_move_or_capture: bool,
    ) -> Result<ClaimAvailability, RulesError> {
        self.check_next_key(key)?;
        let halfmoves = self.next_halfmoves(pawn_move_or_capture)?;
        let reset = pawn_move_or_capture || self.current_key().castling() != key.castling();
        let occurrences = if reset { 1 } else { self.occurrences(key) + 1 };
        Ok(claim_availability(occurrences, halfmoves))
    }

    /// Append exactly one accepted legal ply. Compute every fallible condition
    /// before mutation; duplicates, bad ordering and capacity errors are atomic.
    pub fn record_accepted_position(
        &mut self,
        key: PositionKey,
        ply: u32,
        pawn_move_or_capture: bool,
    ) -> Result<(), RulesError> {
        if self
            .checkpoint
            .ply
            .checked_add(1)
            .ok_or(RulesError::Overflow)?
            != ply
        {
            return Err(RulesError::OutOfOrder);
        }
        self.check_next_key(key)?;
        let halfmoves = self.next_halfmoves(pawn_move_or_capture)?;
        let reset = pawn_move_or_capture || self.current_key().castling() != key.castling();
        let index = if reset {
            0
        } else {
            usize::from(self.checkpoint.len)
        };
        if index >= self.positions.len() {
            return Err(RulesError::Capacity);
        }
        self.positions[index] = key;
        self.checkpoint = RepetitionCheckpoint {
            len: (index + 1) as u16,
            ply,
            halfmoves,
        };
        Ok(())
    }

    fn check_next_key(&self, key: PositionKey) -> Result<(), RulesError> {
        if self.checkpoint.halfmoves >= SEVENTY_FIVE_MOVE_HALFMOVES
            || self.occurrences(self.current_key()) >= 5
        {
            return Err(RulesError::AlreadyAutomatic);
        }
        if key.side_to_move() == self.current_key().side_to_move()
            || key.castling() & !self.current_key().castling() != 0
        {
            return Err(RulesError::InvalidPosition);
        }
        Ok(())
    }

    fn next_halfmoves(&self, pawn_move_or_capture: bool) -> Result<u16, RulesError> {
        if pawn_move_or_capture {
            Ok(0)
        } else {
            self.checkpoint
                .halfmoves
                .checked_add(1)
                .ok_or(RulesError::Overflow)
        }
    }
}
