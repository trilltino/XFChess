# Publishing

The [release workflow](../.github/workflows/release.yml) builds platform artifacts.
It checks backend health before platform builds. Inspect the workflow and its
results for a release; do not infer signing or publication from a local build.

## Trigger and inspect

On Windows, `just release -DryRun` previews branch/tag actions.
`just release` pushes the branch and a version tag; `-Version vX.Y.Z` selects
a version. `-Deploy` also requests the separate VPS deployment after CI.
See [scripts](../scripts/README.md).

A pushed `v*` tag triggers release attachment. Manual dispatch builds workflow
artifacts without attaching them to a GitHub Release:

```text
gh workflow run release.yml --ref <branch>
gh run list --workflow=release.yml --limit 5
gh run view <run-id> --log-failed
```

Use a clean reviewed commit, run the affected checks and inspect each job before
announcing availability. The consumer Tauri build omits `tournament-admin`;
local operator builds include it.

## Artifacts and signing

| Platform | Output | Signing configuration |
| --- | --- | --- |
| Windows | NSIS installer | Azure Trusted Signing, conditional on account configuration |
| macOS | DMG | Apple signing/notarization, conditional on certificate configuration |
| Linux | x86_64 tarball | Game, bridge, wallet UI, assets and Stockfish |
| Chrome OS | Relabeled Linux tarball | Same Linux payload |
| Android | `app-release-unsigned.apk` workflow artifact | Current job builds `assembleRelease` without release signing |

Windows secrets: `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`,
`AZURE_TS_ENDPOINT`, `AZURE_TS_ACCOUNT`, `AZURE_TS_PROFILE`.
macOS secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`.
Confirm their presence without printing values: `gh secret list`.

Android release signing/store publication is a separate setup. Preserve the same
release keystore for every update, keep it outside the repo and verify its
certificate fingerprint. See [Android](runbooks/android-release.md) and
[Gradle configuration](../mobile/app/build.gradle.kts).

## Packaging invariants

- Builds that run the game need real assets; release checkout enables LFS.
- Stockfish download and UCI handshake checks must fail when the engine is absent.
- Packaged launchers must start the wallet bridge with the game and clean it up.
- Embedded-wallet configuration is build-time frontend configuration.
- Program-test requires the freshly built SBF artifact, not a stale binary.
- A release tag, backend health SHA and deployed program bytes are separate evidence.

Run the [verification checklist](development/verification.md). Backend releases
use [ops](../ops/README.md); game-program upgrades use
[the upgrade runbook](runbooks/program-upgrade.md). Confirm caller/IDL compatibility
when a release changes on-chain instructions.
