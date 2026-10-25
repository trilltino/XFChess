# Bind emergency recovery to original players and migrate current Solana APIs

Wiped Game data previously allowed the dispute authority to choose recovery recipients. Legacy wiped recovery now fails closed. The approved RecoveryIdentity PDA binds both original players and their wager before delegation; recovery pays only those identities, and terminal settlements prevent a second payout. The changes also enforce profile/callback PDAs, checked wager expiry, wallet-signed session renewal and atomic mixed SOL/USDC prize claims.

Validation: 263 default program/client tests passed, including actual SBF security regressions. Strict actual-workspace Clippy, backend all-target check, normal safety-linted IDL generation and eight rules tests passed. The real local Agave/ER flow passed delegation, a legal move, stale rejection, scheduled-task creation/cancellation, commit/undelegation, exact Game restoration and escrow/identity conservation. Four default network tests remain explicitly ignored; the local ER target was run separately. Emergency authority fixtures skip cryptographic signature verification and exercise the exact authority signer role in SBF.

Latest direct contract dependencies and tooling compile and run. The strict all-transitive-latest requirement remains BLOCKED: 342 of 953 closure entries are below latest stable, including mandatory upstream interfaces. Published bincode 3.0.0 deliberately does not compile. Four closure advisories remain recorded, with target/feature reachability analysis and no suppressions. This is a devnet release, not independent audit sign-off or mainnet approval.

## Behavior and ABI review

- Program ID, existing instruction discriminators/arguments, Game/Profile layouts,
  events and existing error values are preserved. Two errors append at the end.
- Nine v2 instructions add discriminators and append a permanent record to the
  corresponding original account prefix; creation/joining/delegation clients migrate.
- Legacy delegation and wiped recovery fail closed. No historical identities are
  fabricated or backfilled.
- Legacy creation/join require the canonical empty identity PDA as a trailing
  account, preventing ID reuse or bypass of a v2 binding. Old unmodified clients
  fail closed; their account interface must migrate too.
- Existing normal payout discriminators require the canonical record as a writable
  trailing account, including an empty System PDA for a legacy game. This approved
  account-interface transition must roll out with clients. Remaining accounts are
  declared in the shipped IDL by the documented assembly step.
- Record rent uses the original fee payer or validated session vault. Game rent
  continues to sweep to the existing treasury after verified recovery. Extra escrow
  stays in escrow. A nonzero remainder below rent requires a top-up before recovery;
  otherwise the instruction rejects without any payments. Zero wagers need no
  recipient rent top-up because no escrow transfer occurs.
- The extracted review branch's four narrow edits were inspected at
  `ceb3f540cdc856ff028b3b108683f315c12f448a`. They were ported selectively. Typed
  callback System accounts require individual AccountInfo values; the extraction's
  heterogeneous unchecked-account array cannot simply be copied unchanged.
- The checked-in client IDL was already stale: 69 instructions and an extra `elo`
  argument for register_player. Source HEAD already used its one-argument form.
  Regeneration reflects source, with 82 current instructions. Game/Profile layout
  and old discriminator checks run during assembly.

## Release provenance

The source review checkout excludes unrelated dirty desktop/Tauri changes. Security behavior and the dependency/runtime migration are recorded in separate commits. The owner explicitly authorized the recovery schema/client migration, devnet upgrade, contracts-main push and XFChess PR. Mainnet and authority changes remain outside scope.

The devnet artifact is 1,152,400 bytes, SHA-256 f60ce59fc4ac7cd02a83e520c54b75ea9d44c6127a986fa8efb01b3e431f74fe. Compiler archive warnings about unused crypto serialization frames are retained; the linked ELF has no out-of-frame direct r10 access and passed the runtime suites. This supplemental static inspection is not a complete verifier.

Deployment/commit links are recorded after the upgrade in the dated verification and deployment reports. Wiped historical games without verified identities remain locked. Account-matrix semantic review, unconfirmed observations and exhaustive fuzz/CU campaigns remain outstanding; they are not relabelled resolved findings. Rollback must preserve the new identity safeguards and requires review of affected accounts.
