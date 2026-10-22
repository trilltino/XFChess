Vendored from crates.io proc-macro-error2 2.0.1, under MIT OR Apache-2.0.

The source change makes `extern crate proc_macro` public to match the existing
public re-export. This fixes Rust future-incompatibility E0365 without changing
the macro API. Registry versions have not changed. Test-only manifest entries
are omitted because this copy is a dependency, outside the XFChess workspace.
