pub fn is_set(bitmap: &[u8; 16], board: u16) -> bool {
    let byte = (board / 8) as usize;
    let bit = (board % 8) as u8;
    bitmap[byte] & (1 << bit) != 0
}

pub fn set(bitmap: &mut [u8; 16], board: u16) {
    let byte = (board / 8) as usize;
    let bit = (board % 8) as u8;
    bitmap[byte] |= 1 << bit;
}

pub fn all_set(bitmap: &[u8; 16], boards: u16) -> bool {
    (0..boards).all(|board| is_set(bitmap, board))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_individual_bits_without_disturbing_others() {
        let mut bitmap = [0u8; 16];
        assert!(!is_set(&bitmap, 0));
        assert!(!all_set(&bitmap, 1));

        set(&mut bitmap, 0);
        assert!(is_set(&bitmap, 0));
        assert!(!is_set(&bitmap, 1));
        assert!(all_set(&bitmap, 1));
        assert!(!all_set(&bitmap, 2));
    }

    #[test]
    fn covers_the_256_player_worst_case_of_128_boards() {
        let mut bitmap = [0u8; 16];
        for board in 0..128u16 {
            assert!(!all_set(&bitmap, 128));
            set(&mut bitmap, board);
        }
        assert!(all_set(&bitmap, 128));
    }

    #[test]
    fn last_bit_of_last_byte_is_addressable() {
        // Board 127 is bit 7 of byte 15 — the top corner of the 16-byte array.
        let mut bitmap = [0u8; 16];
        set(&mut bitmap, 127);
        assert!(is_set(&bitmap, 127));
        assert_eq!(bitmap[15], 0b1000_0000);
    }
}
