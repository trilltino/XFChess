use crate::RulesError;

/// Exact encoding v1: 32 bytes of piece nibbles in a1..h8 order (low nibble
/// first; 0 empty, 1..6 white PNBRQK, 7..12 black pnbrqk); castling KQkq in
/// byte 32 bits 0..3, black-to-move in bit 4; byte 33 is a *legal* EP file
/// 0..7 or 8 for none. No clocks, FEN move numbers or compact-board padding.
/// No fingerprint collision can turn different positions into a draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
#[cfg_attr(feature = "account-bytes", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct PositionKey([u8; 34]);

impl Default for PositionKey {
    fn default() -> Self {
        let mut bytes = [0; 34];
        bytes[33] = 8;
        Self(bytes)
    }
}

impl PositionKey {
    /// Adapters must validate the position and establish EP king safety first.
    /// This encoder validates representation, not historical reachability.
    pub fn from_parts(
        squares: &[i8; 64],
        castling: u8,
        side: i8,
        legal_ep: Option<u8>,
    ) -> Result<Self, RulesError> {
        if castling & !15 != 0 || !matches!(side, -1 | 1) {
            return Err(RulesError::InvalidPosition);
        }
        let mut bytes = [0; 34];
        for (index, &piece) in squares.iter().enumerate() {
            let nibble = match piece {
                0..=6 => piece as u8,
                -6..=-1 => (6 - piece) as u8,
                _ => return Err(RulesError::InvalidPosition),
            };
            bytes[index / 2] |= nibble << ((index % 2) * 4);
        }
        bytes[32] = castling | if side == -1 { 16 } else { 0 };
        bytes[33] = match legal_ep {
            None => 8,
            Some(square) if square < 64 && square / 8 == if side == 1 { 5 } else { 2 } => {
                square % 8
            }
            _ => return Err(RulesError::InvalidPosition),
        };
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: [u8; 34]) -> Result<Self, RulesError> {
        if bytes[32] & !31 != 0
            || bytes[33] > 8
            || bytes[..32].iter().any(|b| b & 15 > 12 || b >> 4 > 12)
        {
            return Err(RulesError::InvalidPosition);
        }
        Ok(Self(bytes))
    }

    pub fn to_bytes(self) -> [u8; 34] {
        self.0
    }

    pub fn castling(self) -> u8 {
        self.0[32] & 15
    }

    pub fn side_to_move(self) -> i8 {
        if self.0[32] & 16 == 0 {
            1
        } else {
            -1
        }
    }
}
