# `braid-core/src` module map

The thin facade over [`braid-http`](../../braid-http). See the [crate README](../README.md)
for the full re-export surface and the crate's scope.

| Item | Responsibility |
|------|----------------|
| `core/` | Runtime-abstraction traits (`BraidRuntime`, `BraidNetwork`, `BraidStorage`), the `error` type, and re-exports of the protocol vocabulary — see [`core/README.md`](core/README.md) |
| `lib.rs` | Top-level re-exports: `Version`, `Update`, `Patch`, `BraidRequest`/`BraidResponse`, and (with the `client` feature) `BraidClient`/`Subscription` |
