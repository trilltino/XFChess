XFChess is a forever-free, open source 3D chess platform

Development commands use `just` (`cargo install just`). Run `just` to list
tasks, `just dev` for the local stack, or `just build-all` for a local release
build. See [scripts/COMMANDS.md](scripts/COMMANDS.md) for packaging and
[infra/ops/OPERATIONS.md](infra/ops/OPERATIONS.md) for deployment.

See [contributing](CONTRIBUTING.md) for setup and
[docs/](docs/) for architecture, verification and operations.

The repository root is a virtual Cargo workspace. Deployable applications live
under `apps/`, the backend under `services/`, deployment configuration under
`infra/`, and reusable Rust libraries under `crates/`.

The desktop/Android Rust host is `apps/xfchess-client`. Its reusable client is
split into domain, network, Solana, platform API, and Bevy composition crates;
see [crates/README.md](crates/README.md).
