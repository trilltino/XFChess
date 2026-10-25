use crate::errors::GameErrorCode;
use crate::state::{Game, GameStatus};
use anchor_lang::prelude::*;

/// Permanent seat and wager binding. Never delegated, reassigned, or closed.
#[account]
#[derive(InitSpace)]
pub struct RecoveryIdentity {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub wager_amount: u64,
    pub status: RecoveryStatus,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum RecoveryStatus {
    Waiting,
    Ready,
    Delegated,
    Closed,
    Recovered,
}

impl RecoveryIdentity {
    pub const SEED: &'static [u8] = b"recovery_identity";

    pub fn new(
        game_id: u64,
        white: Pubkey,
        black: Pubkey,
        wager_amount: u64,
        bump: u8,
    ) -> Result<Self> {
        require!(
            white != Pubkey::default(),
            GameErrorCode::RecoveryIdentityMismatch
        );
        wager_amount
            .checked_mul(2)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        if black != Pubkey::default() {
            require_keys_neq!(white, black, GameErrorCode::CannotPlaySelf);
        }
        Ok(Self {
            game_id,
            white,
            black,
            wager_amount,
            bump,
            status: if black == Pubkey::default() {
                RecoveryStatus::Waiting
            } else {
                RecoveryStatus::Ready
            },
        })
    }

    pub fn prepare_join(&mut self, game: &Game, black: Pubkey) -> Result<()> {
        self.require_game(game)?;
        require!(
            self.status == RecoveryStatus::Waiting
                && self.black == Pubkey::default()
                && game.black == Pubkey::default()
                && !game.is_delegated
                && game.status == GameStatus::WaitingForOpponent
                && black != Pubkey::default(),
            GameErrorCode::RecoveryIdentityMismatch
        );
        require_keys_neq!(self.white, black, GameErrorCode::CannotPlaySelf);
        self.black = black;
        self.status = RecoveryStatus::Ready;
        Ok(())
    }

    pub fn from_created_game(game: &Game, bump: u8) -> Result<Self> {
        require!(
            !game.is_delegated && game.wager_token.is_none(),
            GameErrorCode::RecoveryIdentityUnavailable
        );
        require!(
            game.white != Pubkey::default(),
            GameErrorCode::RecoveryIdentityUnavailable
        );
        let status = if game.black == Pubkey::default() {
            require!(
                game.status == GameStatus::WaitingForOpponent,
                GameErrorCode::InvalidGameStatus
            );
            RecoveryStatus::Waiting
        } else {
            require_keys_neq!(game.white, game.black, GameErrorCode::CannotPlaySelf);
            require!(
                game.status == GameStatus::Active,
                GameErrorCode::InvalidGameStatus
            );
            RecoveryStatus::Ready
        };
        Ok(Self {
            game_id: game.game_id,
            white: game.white,
            black: game.black,
            wager_amount: game.wager_amount,
            status,
            bump,
        })
    }

    pub fn require_game(&self, game: &Game) -> Result<()> {
        require!(
            self.game_id == game.game_id
                && self.white == game.white
                && self.wager_amount == game.wager_amount
                && game.wager_token.is_none(),
            GameErrorCode::RecoveryIdentityMismatch
        );
        Ok(())
    }

    pub fn bind_joined_game(&mut self, game: &Game) -> Result<()> {
        self.require_game(game)?;
        require!(
            self.status == RecoveryStatus::Waiting
                && self.black == Pubkey::default()
                && !game.is_delegated
                && game.status == GameStatus::Active,
            GameErrorCode::RecoveryIdentityMismatch
        );
        require!(
            game.black != Pubkey::default(),
            GameErrorCode::RecoveryIdentityMismatch
        );
        require_keys_neq!(self.white, game.black, GameErrorCode::CannotPlaySelf);
        self.black = game.black;
        self.status = RecoveryStatus::Ready;
        Ok(())
    }

    pub fn mark_delegated(&mut self, game: &Game) -> Result<()> {
        self.require_game(game)?;
        require!(
            matches!(
                self.status,
                RecoveryStatus::Ready | RecoveryStatus::Delegated
            ) && !game.is_delegated
                && game.status == GameStatus::Active
                && self.black == game.black
                && self.black != Pubkey::default(),
            GameErrorCode::RecoveryIdentityMismatch
        );
        self.status = RecoveryStatus::Delegated;
        Ok(())
    }
}
