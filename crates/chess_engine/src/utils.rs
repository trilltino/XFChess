//! Utility functions and helpers
//!
//! This module contains utility functions used throughout the engine,
//! including memory allocation helpers and array initialization utilities.

use super::types::KK;

/// Helper to create a boxed array directly on the heap to avoid stack overflow
///
/// This is necessary for large arrays like the transposition table (2M entries).
/// Allocating such arrays on the stack would cause stack overflow.
///
/// # Safety
///
/// This function uses a minimal amount of unsafe code to convert a `Box<[T]>` to `Box<[T; N]>`.
/// The conversion is safe because:
/// - We create a Vec with exactly N elements
/// - We convert it to `Box<[T]>` which has length N
/// - `[T; N]` and `[T]` have the same memory layout when the length matches
/// - The returned Box properly manages the memory lifetime
///
/// # Alternatives Considered
///
/// - `Box::new([T::default(); N])` - Causes stack overflow for large N (2M entries)
/// - `vec![T::default(); N].into_boxed_slice()` - Returns `Box<[T]>`, not `Box<[T; N]>`
/// - `Box::new_uninit_slice()` - Requires nightly Rust
/// - Manual allocation with `std::alloc` - More unsafe code, less safe
///
/// # Panics
///
/// Panics if memory allocation fails (this is unrecoverable).
pub(crate) fn create_boxed_array<T: Default, const N: usize>() -> Box<[T; N]> {
    // Use Vec as an intermediate step - it allocates on the heap
    // and we can safely convert it to a boxed array
    let mut vec = Vec::with_capacity(N);

    // Initialize all elements with Default::default()
    // This is done on the heap, avoiding stack overflow
    vec.resize_with(N, T::default);

    // Convert Vec to Box<[T]>
    let boxed_slice: Box<[T]> = vec.into_boxed_slice();

    // Verify length matches (defensive check)
    assert_eq!(
        boxed_slice.len(),
        N,
        "Boxed slice must have exactly N elements"
    );

    // Convert Box<[T]> to Box<[T; N]>
    // SAFETY: This conversion is safe because:
    // 1. We created the slice with exactly N elements (verified above)
    // 2. [T; N] and [T] have identical memory layouts when length matches
    // 3. The pointer cast preserves the memory layout
    // 4. Box will properly deallocate the memory when dropped
    unsafe {
        let raw = Box::into_raw(boxed_slice);
        // Cast from *mut [T] to *mut [T; N]
        // This is safe because the memory layout is identical
        Box::from_raw(raw as *mut [T; N])
    }
}

/// Create an array of 64 empty Vecs for move tables
///
/// This is a helper function to initialize move tables for all 64 squares.
/// Uses a more idiomatic approach with array initialization.
pub(crate) fn create_empty_move_table_array() -> [Vec<KK>; 64] {
    // Use array initialization with a const array and then map
    // This is more idiomatic than manually writing 64 Vec::new() calls
    [(); 64].map(|_| Vec::new())
}
