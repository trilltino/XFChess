# backend/src/bin

Auxiliary backend binaries (the API server itself is
[../signing_server.rs](../signing_server.rs)).

| Binary | File | Purpose |
|--------|------|---------|
| `tournament_admin` | [tournament_admin.rs](tournament_admin.rs) | CLI for creating/starting/inspecting tournaments against the admin API |
| `vps_admin` | [vps_admin.rs](vps_admin.rs) | VPS operational tasks |
| `import_puzzles` | [import_puzzles.rs](import_puzzles.rs) | Bulk-import tactics puzzles into SQLite (migration 018) |
| — | [convert_keys.rs](convert_keys.rs) / [convert_keys.js](convert_keys.js) | Keypair format conversion helpers |

```bash
cargo run --bin tournament_admin -- --help
cargo run --bin import_puzzles -- <puzzles.csv>
```
