# Latest dependency compatibility spike — separate workstream

**BLOCKED: ALL-LATEST TRANSITIVE GRAPH INCOMPATIBLE WITH PUBLISHED SDK CONSTRAINTS.**

The checked-in isolated manifest, source and lockfile actually compile this CPI
boundary using published latest stable core releases queried during the task:

| Package | Security workspace | Compiled spike |
|---|---|---|
| anchor-lang | 1.1.2 | 1.2.1 |
| anchor-spl | 1.1.2 | 1.2.1 |
| ephemeral-rollups-sdk | 0.16.2 | 0.17.3 |
| solana-program | 3.0.0 | 5.1.0 |

`cargo check --manifest-path <spike>/Cargo.toml` passed. This disproves an assumed
blanket incompatibility of those four latest top-level releases at this callback
boundary. It does not compile the entire application against them, run SBF, prove
MagicBlock runtime behavior, or satisfy the requested all-dependencies end state.

The live registry data records these precise blockers in the newest parents:

- SDK 0.17.3 requires `base64ct =1.6.0`; latest stable is 1.8.3. Cargo must retain
  1.6.0 until the parent publishes a changed constraint. An additional direct
  dependency cannot replace the exact requirement.
- SDK 0.17.3 requires `solana-program ^3.0.0`, `bincode ^1.3.3`, `thiserror ^1.0`
  and the older Solana account/instruction family. The isolated graph therefore
  still contains older packages even with solana-program 5.1.0 selected directly.
- Anchor 1.2.1 requires `base64 ^0.21`, `bincode ^1`, `thiserror ^1` and Solana
  instruction `^3`. These exclude their newest stable major releases.
- Latest magicblock-magic-program-api 1.1.1 still requires bincode `^1.3.3`
  and solana-program `^3.0`. Crank integration needs a separate API migration.
- Two Anza Quinn registry names have no non-yanked stable release; prereleases
  cannot be silently called stable. See inventory status `NO_STABLE_RELEASE`.

The smallest upstream resolution starts with a MagicBlock release relaxing the
base64ct exact pin, followed by coordinated Anchor/MagicBlock serialization and
Solana-family migrations. No approved fork or custom MagicBlock replacement was
provided. Forcing graph overrides would violate the prompt's constraint and
serialization requirements. No dependency versions were changed in the security
workspace as a substitute for this work.

The full inventory contains 955 resolved package entries and 790 registry names,
including optional and test/build dependencies. All latest-stable lookups succeeded;
684 entries require upgrades. The CSV records parents and source URLs. Its feature
column explicitly identifies lockfile limitations rather than inventing feature
activation. Advisory matching found 21 contract-closure findings requiring actual
reachability analysis. No vulnerability suppression was added.

Registry primary sources: [Anchor index](https://index.crates.io/an/ch/anchor-lang),
[MagicBlock SDK index](https://index.crates.io/ep/he/ephemeral-rollups-sdk),
[base64ct index](https://index.crates.io/ba/se/base64ct),
[Solana Program index](https://index.crates.io/so/la/solana-program),
[Magic Program API index](https://index.crates.io/ma/gi/magicblock-magic-program-api).
The machine-readable snapshot is `../dependency-registry-20261023.json`.

Before integrating an upgrade: migrate every direct/dev/build package and client
interface; re-resolve and audit all indirect packages; test generated interfaces,
SBF, real ER lifecycle and every funding path; measure bytes, CU and rent. Any
remaining outdated package keeps this workstream blocked. This spike's lockfile
is intentionally separate from the application's shared lockfile.
