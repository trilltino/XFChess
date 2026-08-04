// Aggregator target that runs the subdirectory module tests (`tests/components/`
// and `tests/resources/`), which Cargo does NOT compile on their own. The
// top-level `tests/*.rs` files (systems_tests, types_tests, core_tests, …) are
// already their own integration targets, so they must NOT be re-included here —
// doing so double-compiles them and fans out any failure.
pub mod components;
pub mod resources;
