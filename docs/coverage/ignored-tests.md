# Ignored-test audit

18 ignored tests. Owners below are component ownership roles; no individual assignment is implied. Fixtures must never be run by a blanket `--ignored` job. Live runs need explicit secrets and funding.

| Test | Reason / prerequisites | Owner | Disposition |
|---|---|---|---|
| `backend/tests/e2e_rpc_live.rs::admin_creates_tournament_and_game_client_can_see_it` | hits a live RPC endpoint and spends real devnet SOL (3 on-chain inits) — opt in with --ignored | backend maintainers | nightly-live |
| `backend/tests/e2e_rpc_live.rs::debug_transaction_reports_real_on_chain_failure` | hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored | backend maintainers | nightly-live |
| `backend/tests/e2e_rpc_live.rs::debug_transaction_reports_real_on_chain_outcome` | hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored | backend maintainers | nightly-live |
| `backend/tests/e2e_rpc_live.rs::deployed_program_account_is_reachable_via_failover` | hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored | backend maintainers | nightly-live |
| `backend/tests/e2e_rpc_live.rs::fallback_url_is_independently_reachable` | hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored | backend maintainers | nightly-live |
| `backend/tests/e2e_rpc_live.rs::init_profile_flow_flips_onchain_username_set` | hits a live RPC endpoint and spends real devnet SOL (airdrop) — opt in with --ignored | backend maintainers | nightly-live |
| `backend/tests/e2e_rpc_live.rs::primary_rpc_reports_healthy` | hits a live RPC endpoint (Triton in prod/staging) — opt in with --ignored | backend maintainers | nightly-live |
| `libs/anti-cheat/xfchess-anticheat/src/engine/stockfish.rs::fake_engine_child` | child process fixture | client maintainers | subprocess-fixture |
| `libs/chess/xfchess-stockfish/src/tests.rs::bundled_engine` | requires bundled native Stockfish; cargo test -p xfchess-stockfish bundled_engine -- --ignored | client maintainers | nightly-offline |
| `libs/chess/xfchess-stockfish/src/tests.rs::fake_engine_child` | subprocess fixture, invoked only by protocol tests | client maintainers | subprocess-fixture |
| `libs/zarathustra_net/braid-http/src/client/fuzzer.rs::test_parser_fuzz_forever` | unbounded manual fuzz loop; owner: netcode maintainers; use bounded nightly fuzzing | netcode maintainers | manual-unbounded |
| `libs/zarathustra_net/braid-http/tests/reliable_channel.rs::child_journal_writer` | Only invoked by the process-crash regression | netcode maintainers | subprocess-fixture |
| `libs/zarathustra_net/braid-http/tests/xfchess_javascript.rs::javascript_and_rust_exchange_snapshots_and_range_patches` | Requires Node.js and BRAID_JS_SOURCE pointing to an upstream braid-http checkout | netcode maintainers | manual-upstream |
| `programs/xfchess-game/tests/local_er_integration_tests.rs::delegate_move_undelegate_restores_exact_game_bytes_and_retains_identity` | requires the explicitly started local L1/ER stack on ports 18899/17799 | contract maintainers | manual-local-stack |
| `programs/xfchess-game/tests/tournament_cancel_tests.rs::active_cancel_refunds_host_and_cannot_repeat` | Anchor 1.1.2 SOL transfer CPI is unsupported by the native host processor | contract maintainers | quarantine (review 2026-11-02) |
| `programs/xfchess-game/tests/tournament_pass_devnet.rs::the_deployed_programs_attest_expire_revoke_and_burn_passes` | spends Devnet SOL; needs VPS_KEYPAIR and PAYER_KEYPAIR | contract maintainers | nightly-live |
| `programs/xfchess-game/tests/tournament_registration_e2e_tests.rs::register_player_deposits_entry_fee_and_adds_to_shard` | needs keys/vps_authority.json (devnet-only, gitignored) â€” see docs/development/verification.md | contract maintainers | nightly-live |
| `tests/live_devnet_lifecycle.rs::creator_cancels_unjoined_wagered_game_and_is_refunded` | live devnet: spends ~0.01 SOL; set XFCHESS_LIVE_FUNDER and pass --ignored | client maintainers | nightly-live |
