# Contributing to XFChess

Use a focused branch and pull request. Explain the problem, link related issues
and include validation appropriate to the affected component. Report security
issues privately through [SECURITY.md](SECURITY.md).

## Setup

Clone the repository and install Rust stable. On Windows, install the MSVC C++
build tools and `just` (`cargo install just`). Frontends need Node.js 20.19+
or another supported version. Solana/Anchor tooling is needed for program work,
not for ordinary offline gameplay.

```text
git clone https://github.com/trilltino/XFChess.git
cd XFChess
cargo check --bin xfchess
cargo run --bin xfchess
```

On Windows, `just dev` launches the local stack and `just dev2` launches two
clients. Configure local secrets from `.env.example`; never commit real keys.
The just launch recipes use Windows PowerShell. On other platforms, use the
component Cargo/npm commands and install the native libraries from CI.

[Development](docs/development/README.md) maps components to guides and checks.
[Installation](docs/INSTALL.md) covers prebuilt releases.

## Changes and validation

- Keep code idiomatic and functions focused; follow adjacent module conventions.
- Format changed Rust files and run relevant checks/tests. Avoid incidental
  workspace-wide formatting or generated-output changes.
- Test feature-gated paths that your change touches.
- Use meaningful tests for behavior and regressions; visual/lifecycle changes
  also need a manual pass.
- Explain non-obvious invariants, safety requirements and public API contracts.
  Comments should add information; do not require boilerplate on every item.
- Update the canonical component guide when behavior changes. Put implementation
  history in commits and unresolved work in issues.
- Do not change deployed account layouts or instruction contracts without a
  coordinated migration/compatibility plan.

Mark unvalidated work as draft. Include reproduction steps and platform details
for bugs, and a concrete user need for feature proposals. Clear commit subjects
such as `fix: ...` or `docs: ...` help reviewers scan changes.

## AI-assisted contributions

Review and understand generated code, and be able to explain it. Include the
tool and relevant prompts with the pull request or commits. Provide manual-test
evidence, such as screenshots/video, where the change affects user interaction.

## Artwork and documentation

Use [design references](docs/design/README.md) and existing runtime asset folders.
Keep docs concise, link to one canonical guide and check paths/commands against
the current tree. [The documentation index](docs/README.md) lists contributor,
architecture and operator material.
