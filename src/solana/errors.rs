use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum XfChessError {
    #[error("Game is already full")]
    GameFull,

    #[error("Not this player's turn")]
    NotPlayersTurn,

    #[error("Invalid move")]
    InvalidMove,

    #[error("Game has already ended")]
    GameEnded,

    #[error("Player not part of this game")]
    UnauthorizedPlayer,

    #[error("Game does not exist")]
    GameNotFound,
}

impl From<XfChessError> for ProgramError {
    fn from(e: XfChessError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xfchess_error_conversion() {
        let error = XfChessError::GameFull;
        let program_error: ProgramError = error.into();

        match program_error {
            ProgramError::Custom(code) => {
                assert_eq!(code, XfChessError::GameFull as u32);
            }
            _ => panic!("Expected ProgramError::Custom"),
        }
    }

    #[test]
    fn test_all_error_variants() {
        // Test that all error variants convert properly to ProgramError
        let errors = [
            XfChessError::GameFull,
            XfChessError::NotPlayersTurn,
            XfChessError::InvalidMove,
            XfChessError::GameEnded,
            XfChessError::UnauthorizedPlayer,
            XfChessError::GameNotFound,
        ];

        for error in errors.iter() {
            let program_error: ProgramError = (*error).into();
            match program_error {
                ProgramError::Custom(_) => {} // All good
                _ => panic!("Expected ProgramError::Custom for {:?}", error),
            }
        }
    }
}
