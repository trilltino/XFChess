//! `signing-server` — the local-dev bin name. See `backend::server` for the
//! actual server; `signing-server-http` is the identical deploy/prod alias.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    backend::server::run()
}
