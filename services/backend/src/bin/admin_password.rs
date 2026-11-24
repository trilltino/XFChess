//! Prints the `ADMIN_USERS` entry for an admin account.
//!
//!   cargo run -p backend --bin admin_password -- <name>
//!
//! Asks for the password twice without showing it, then prints a line for the backend's `.env`.
//! Several accounts share one value, separated by `;`. Restart the backend after changing it;
//! a changed password ends that account's open sessions.

use anyhow::{bail, Context as _};
use backend::infrastructure::admin_session::{hash_password, valid_name};

const MIN_PASSWORD_CHARS: usize = 12;
const MAX_PASSWORD_BYTES: usize = 1024;

fn main() -> anyhow::Result<()> {
    let name = std::env::args()
        .nth(1)
        .context("usage: admin_password <name>")?;
    if !valid_name(&name) {
        bail!("an admin name is 1-32 letters, digits, '_', '-' or '.'");
    }

    let term = console::Term::stderr();
    term.write_str("Password: ")?;
    let password = term.read_secure_line()?;
    term.write_str("Repeat it: ")?;
    if term.read_secure_line()? != password {
        bail!("the two passwords are different");
    }
    if password.chars().count() < MIN_PASSWORD_CHARS {
        bail!("use at least {MIN_PASSWORD_CHARS} characters");
    }
    if password.len() > MAX_PASSWORD_BYTES {
        bail!("use at most {MAX_PASSWORD_BYTES} bytes");
    }

    let hash = hash_password(&password).map_err(anyhow::Error::msg)?;
    eprintln!("\nPut this line in the backend's .env (keep the single quotes), then restart it:\n");
    println!("ADMIN_USERS='{name}:{hash}'");
    Ok(())
}
