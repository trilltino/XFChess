# solana-chess-client

Client-side Rust bindings for the `xfchess-game` Solana program: PDA derivation,
account fetching, and instruction builders for every game/session flow. The game
client links it behind `--features solana`; the backend's signing routes use the same
builders to construct unsigned transactions that players sign locally.

## Modules

### `rpc.rs` — the main client

One client struct wrapping an RPC URL, exposing:

- **PDA derivation** — `get_game_pda`, `get_escrow_pda`, `get_profile_pda`,
  `get_move_log_pda`, `get_session_delegation_pda`, `get_global_session_pda`
  (mirrors the program's seed scheme exactly; see `programs/xfchess-game/src/constants.rs`).
- **Account fetch/deserialize** — `fetch_game`, `fetch_all_games`, `fetch_profile`.
- **Instruction builders** — profiles (`create_init_profile_ix`), game lifecycle
  (`create_create_game_ix`, `create_join_game_ix`, `create_record_move_ix`,
  `create_finalize_game_ix`, `create_withdraw_expired_wager_ix`), per-game session
  keys (`create_authorize_session_key_ix`, `create_revoke_session_key_ix`), and the
  global persistent session flow (`create_authorize_global_session_ix`,
  `create_revoke_global_session_ix`, `create_global_create_game_ix`,
  `create_global_join_game_ix`).

### `wallet.rs`

`KeypairWallet`: construct from an in-memory `Keypair`, load from a JSON keyfile
(`load_from_file`), or `generate_new` — used by CLIs, tests, and the desktop client's
session keys.

## Invariants

- Builders must stay byte-compatible with the deployed program's instruction layout
  and account ordering; when the program adds or changes an instruction, add the
  matching builder here in the same change.
- This crate never holds or transmits private keys for signing on a server — the
  backend builds unsigned transactions only.
