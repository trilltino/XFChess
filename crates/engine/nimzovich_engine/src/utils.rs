use super::types::KK;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Create an array of 64 empty Vecs for move tables
///
/// This is a helper function to initialize move tables for all 64 squares.
/// Uses a more idiomatic approach with array initialization.
pub(crate) fn create_empty_move_table_array() -> [Vec<KK>; 64] {
    // Use array initialization with a const array and then map
    // This is more idiomatic than manually writing 64 Vec::new() calls
    [(); 64].map(|_| Vec::new())
}
