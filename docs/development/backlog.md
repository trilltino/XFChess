# Backlog

Keep reproducible work in issues with a code owner, acceptance test and source
commit. This preserves remaining concerns from earlier plans; it is not a fresh
audit or a declaration that the current release fails. Check current code before
implementing a proposed fix.

| Area | Next evidence or decision | Boundary |
| --- | --- | --- |
| Live recovery | Two-machine restart, uncertain submission and ER outage rehearsals | [Verification](verification.md) |
| Resumed clocks | Decide/test authoritative per-side remaining time after restart | [Resume](../../src/multiplayer/resume.rs), [clock rules](../../programs/xfchess-game/src/lifecycle/clock.rs) |
| Anti-cheat/payout | Document verdict timing, hold-window expiry and verdicts after payment | [Settlement worker](../../backend/src/tasks/settlement_worker.rs), [tournament scheduler](../../backend/src/tasks/tournament_scheduler.rs) |
| Bevy scheduling | Triage reported ambiguities; test repeated mode transitions before changing cleanup ownership | [Schedule](../../src/core/schedule.rs), [state lifecycle](../../src/core/state_lifecycle.rs) |
| Identity/wallets | Device-test guest/account separation, Lichess linking, sponsored profiles and wallet rejection/recovery | [Account UI](../../src/ui/account/), [wallet bridge](../../tauri/src/wallet/) |
| Tournament compatibility | Compare deployed program with updated Swiss/game-creation callers | [Upgrade runbook](../runbooks/program-upgrade.md) |
| Authority hygiene | Confirm historical credential rotation and recovery access; retain the chosen dedicated-wallet model | [Secret rotation](../../ops/SECRETS_ROTATION.md) |
| Android distribution | Signing, store setup and physical-device lifecycle verification | [Android runbook](../runbooks/android-release.md) |
| Operator actions | Keep unsupported actions explicit; force-resign returns `not_implemented` | [Admin routes](../../backend/src/signing/routes/admin.rs) |
| Website discovery | Verify canonical routes and submit the sitemap through owner accounts | [Website](../../xfchessdotcom/) |

Capacity changes need measured bottlenecks; see [capacity](../../ops/docs/CAPACITY.md).
Account-layout changes require an ABI/migration plan, even for fields that appear
unused. Do not remove active Braid/Iroh or session compatibility paths solely
because an old audit called them legacy.
