//! `signing-server-http` — the ops/prod bin name (systemd unit, CI, deploy
//! scripts). See `backend::server` for the actual server; `signing-server` is
//! the identical local-dev alias.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    backend::server::run()
}
