use crate::{AdjudicatedClock, ClaimAvailability, ClockError, Termination};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimReason {
    Repetition,
    FiftyMoves,
}

/// Transport authentication and legal intended-move validation precede receipt.
/// Keep this envelope in durable game state, never an unbound UI resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimRequest {
    pub game: [u8; 32],
    pub ply: u32,
    pub nonce: u64,
    pub claimant: i8,
    pub reason: ClaimReason,
    pub intended_move: Option<[u8; 5]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimDecision {
    Confirmed(Termination),
    IncorrectPenalized {
        opponent: i8,
        mandatory_move: Option<[u8; 5]>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimError {
    InvalidCheckpoint,
    WrongGame,
    StalePly,
    StaleNonce,
    WrongTurn,
    Pending,
    NotPending,
    Finished,
    MandatoryMove,
    MalformedIntent,
    Overflow,
    Clock(ClockError),
}

impl From<ClockError> for ClaimError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

/// Pure procedure after seat authentication. Cryptography, legal intended-move
/// proof, terminal-position validation and durable transaction commit belong to
/// the adapter. Preserve the entire struct and clock atomically on recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimProcedure {
    game: [u8; 32],
    next_nonce: u64,
    pending: Option<ClaimRequest>,
    mandatory_move: Option<[u8; 5]>,
    terminal: Option<Termination>,
    draw_offered_by: Option<i8>,
}

impl ClaimProcedure {
    /// Restore only a completed atomic adjudication. Pending asynchronous
    /// procedures require their full durable receipt, not this constructor.
    pub fn restore_completed(
        game: [u8; 32],
        next_nonce: u64,
        mandatory_move: Option<[u8; 5]>,
        terminal: Option<Termination>,
        draw_offered_by: Option<i8>,
    ) -> Result<Self, ClaimError> {
        let mandatory_move = mandatory_move.map(canonical_uci).transpose()?;
        if draw_offered_by.is_some_and(|side| !matches!(side, -1 | 1))
            || (terminal.is_some() && (mandatory_move.is_some() || draw_offered_by.is_some()))
        {
            return Err(ClaimError::InvalidCheckpoint);
        }
        Ok(Self {
            game,
            next_nonce,
            pending: None,
            mandatory_move,
            terminal,
            draw_offered_by,
        })
    }
    pub fn new(game: [u8; 32]) -> Self {
        Self {
            game,
            next_nonce: 0,
            pending: None,
            mandatory_move: None,
            terminal: None,
            draw_offered_by: None,
        }
    }

    pub fn pending(&self) -> Option<ClaimRequest> {
        self.pending
    }
    pub fn mandatory_move(&self) -> Option<[u8; 5]> {
        self.mandatory_move
    }
    pub fn next_nonce(&self) -> u64 {
        self.next_nonce
    }

    /// Article 9.1.2.3: the formal claim is also a non-withdrawable draw offer.
    /// The mode adapter can accept it under Article 5.2.3 after both have moved.
    pub fn draw_offered_by(&self) -> Option<i8> {
        self.draw_offered_by
    }

    /// `request.claimant` must already be bound to an authenticated current
    /// seat. Malformed or illegal intended moves are rejected before this call.
    /// The nonce is consumed at durable receipt, not again on delayed resolution.
    pub fn receive_authenticated(
        &mut self,
        request: ClaimRequest,
        current_ply: u32,
        clock: &mut AdjudicatedClock,
        now: u64,
    ) -> Result<(), ClaimError> {
        if self.terminal.is_some() {
            return Err(ClaimError::Finished);
        }
        if request.game != self.game {
            return Err(ClaimError::WrongGame);
        }
        if request.ply != current_ply {
            return Err(ClaimError::StalePly);
        }
        if request.nonce != self.next_nonce {
            return Err(ClaimError::StaleNonce);
        }
        if request.claimant != clock.side_to_move() {
            return Err(ClaimError::WrongTurn);
        }
        if self.pending.is_some() {
            return Err(ClaimError::Pending);
        }
        if self.mandatory_move.is_some() {
            return Err(ClaimError::MandatoryMove);
        }
        let mut request = request;
        request.intended_move = request.intended_move.map(canonical_uci).transpose()?;
        let next = self.next_nonce.checked_add(1).ok_or(ClaimError::Overflow)?;
        clock.pause_for_claim(now)?;
        self.next_nonce = next;
        self.pending = Some(request);
        self.draw_offered_by = Some(request.claimant);
        Ok(())
    }

    /// Use eligibility calculated from the pending request's authoritative
    /// current position or legally simulated intended move, not peer hints.
    /// On success the intended move has not been applied to board or history.
    pub fn resolve(
        &mut self,
        eligibility: ClaimAvailability,
        clock: &mut AdjudicatedClock,
        now: u64,
    ) -> Result<ClaimDecision, ClaimError> {
        let request = self.pending.ok_or(ClaimError::NotPending)?;
        if clock.side_to_move() != request.claimant {
            return Err(ClaimError::WrongTurn);
        }
        clock.check_claim_resolution(now)?;
        let reason = match request.reason {
            ClaimReason::Repetition if eligibility.repetition => Some(Termination::ThreefoldClaim),
            ClaimReason::FiftyMoves if eligibility.fifty_moves => Some(Termination::FiftyMoveClaim),
            _ => None,
        };
        let decision = if let Some(reason) = reason {
            self.terminal = Some(reason);
            self.draw_offered_by = None;
            ClaimDecision::Confirmed(reason)
        } else {
            clock.reject_claim_and_resume(now)?;
            self.mandatory_move = request.intended_move;
            ClaimDecision::IncorrectPenalized {
                opponent: -request.claimant,
                mandatory_move: request.intended_move,
            }
        };
        self.pending = None;
        Ok(decision)
    }

    pub fn check_move_allowed(&self, uci: [u8; 5]) -> Result<(), ClaimError> {
        if self.terminal.is_some() {
            return Err(ClaimError::Finished);
        }
        if self.pending.is_some() {
            return Err(ClaimError::Pending);
        }
        let uci = canonical_uci(uci)?;
        if self.mandatory_move.is_some_and(|required| required != uci) {
            return Err(ClaimError::MandatoryMove);
        }
        Ok(())
    }

    /// Call only within the atomic accepted-move transition after legality,
    /// correct turn and clock validation. Failed transactions retain obligation.
    pub fn record_accepted_move(
        &mut self,
        uci: [u8; 5],
        moving_side: i8,
    ) -> Result<(), ClaimError> {
        self.check_move_allowed(uci)?;
        if !matches!(moving_side, -1 | 1) {
            return Err(ClaimError::WrongTurn);
        }
        if self
            .draw_offered_by
            .is_some_and(|offerer| offerer != moving_side)
        {
            self.draw_offered_by = None;
        }
        self.mandatory_move = None;
        Ok(())
    }
}

fn canonical_uci(mut uci: [u8; 5]) -> Result<[u8; 5], ClaimError> {
    if !(b'a'..=b'h').contains(&uci[0])
        || !(b'1'..=b'8').contains(&uci[1])
        || !(b'a'..=b'h').contains(&uci[2])
        || !(b'1'..=b'8').contains(&uci[3])
        || (uci[0] == uci[2] && uci[1] == uci[3])
        || !matches!(uci[4], 0 | b' ' | b'q' | b'r' | b'b' | b'n')
    {
        return Err(ClaimError::MalformedIntent);
    }
    if uci[4] == b' ' {
        uci[4] = 0;
    }
    Ok(uci)
}
