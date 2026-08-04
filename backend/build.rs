// Emit the current git commit SHA as a compile-time env var (`GIT_SHA`) so the
// running backend can report exactly which commit it was built from (deploy →
// commit traceability; see /health). Best-effort: "unknown" if git is unavailable.
use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_SHA={sha}");
    // Re-run if HEAD moves so the baked SHA stays current.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/heads");
}
