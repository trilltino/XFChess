use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FideTermination {
    None, Checkmate, Stalemate, DeadPosition, ThreefoldClaim, FiftyMoveClaim,
    FivefoldRepetition, SeventyFiveMoves, Agreement, Resignation,
    ResignationWithoutPossibleMate, FlagFall, FlagFallWithoutPossibleMate,
}

#[cfg(feature = "move-validation")]
impl From<nimzovich_engine::adjudication::Termination> for FideTermination {
    fn from(reason: nimzovich_engine::adjudication::Termination) -> Self {
        use nimzovich_engine::adjudication::Termination as T;
        match reason {
            T::Checkmate => Self::Checkmate, T::Stalemate => Self::Stalemate,
            T::DeadPosition => Self::DeadPosition, T::ThreefoldClaim => Self::ThreefoldClaim,
            T::FiftyMoveClaim => Self::FiftyMoveClaim, T::FivefoldRepetition => Self::FivefoldRepetition,
            T::SeventyFiveMoves => Self::SeventyFiveMoves, T::Agreement => Self::Agreement,
            T::Resignation => Self::Resignation, T::ResignationWithoutPossibleMate => Self::ResignationWithoutPossibleMate,
            T::FlagFall => Self::FlagFall, T::FlagFallWithoutPossibleMate => Self::FlagFallWithoutPossibleMate,
        }
    }
}

#[event]
pub struct FideGameEnded {
    pub game_id: u64,
    pub reason: FideTermination,
    pub winner: Option<Pubkey>,
    pub timestamp: i64,
}

#[event]
pub struct FideClaimAdjudicated {
    pub game_id: u64,
    pub claimant: Pubkey,
    pub claim_nonce: u64,
    pub move_nonce: u64,
    pub repetition_claim: bool,
    pub confirmed: bool,
    pub intended_move: Option<[u8; 5]>,
    pub remaining_millis: [u64; 2],
    pub timestamp: i64,
}
