# src/assets

Centralized asset preloading and tracking: the chess-piece GLTF, per-piece mesh
handles, and loading progress, held in resources so other modules never call
`asset_server.load` with raw paths.

## Role in XFChess

The loading flow blocks on `LoadingProgress` before entering the main menu;
`rendering/pieces` pulls its mesh handles from `GameAssets.piece_meshes`. Asset files
themselves live in the repo-root `assets/` directory and are copied next to the
binary by `build.rs`.

## Key files

| File | Contents |
|------|----------|
| [mod.rs](mod.rs) | `GameAssets` / `PieceMeshes` / `LoadingProgress` resources, `start_asset_loading` + `check_asset_loading` systems, `AssetLoadFailedEvent` handlers |

## Example

```rust
// mod.rs — rendering reads mesh handles from the registry, never from paths
pub struct GameAssets {
    pub pieces_gltf: Handle<Gltf>,     // chess pieces GLTF file
    pub piece_meshes: PieceMeshes,     // king/queen/rook/bishop/knight/pawn Handle<Mesh>
    pub loaded: bool,
    pub failed: bool,
    pub error_message: Option<String>,
}
```

## Gotchas

- Load failures set `GameAssets.failed` + `error_message` via the
  `handle_asset_loading_errors` systems instead of panicking — if a piece is
  invisible, check the log and the `failed` flag first.
- `PieceMeshes` fields are `Option<Handle<Mesh>>` until the GLTF resolves; consumers
  must handle the `None` window during startup.
