"""Prepare an isolated host harness without the program-test dev graph.

Uses the actual source files by path and the same pinned workspace dependencies.
The harness has its own lockfile; it does not update the application lockfile.
"""
from pathlib import Path
import json
import sys
import tomllib

root = Path(__file__).resolve().parents[2]
target = Path(sys.argv[1]).resolve()
target.mkdir(parents=True, exist_ok=True)
workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
manifest = (root / "programs/xfchess-game/Cargo.toml").read_text(encoding="utf-8")
runtime = manifest[manifest.index("[features]"):manifest.index("# Run record_move")]


def toml(value):
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, list):
        return "[" + ", ".join(toml(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {toml(v)}" for k, v in value.items()) + " }"
    raise TypeError(value)


# Resolve the actual workspace declarations; never silently fall back to the
# historical audit pins when generating evidence for a newer workspace.
program = tomllib.loads(manifest)
for name, declaration in program["dependencies"].items():
    if not isinstance(declaration, dict) or not declaration.get("workspace"):
        continue
    inherited = workspace["workspace"]["dependencies"][name]
    merged = {"version": inherited} if isinstance(inherited, str) else dict(inherited)
    for key, value in declaration.items():
        if key == "workspace":
            continue
        if key == "features":
            merged[key] = list(dict.fromkeys(merged.get(key, []) + value))
        else:
            merged[key] = value
    import re
    runtime = re.sub(r"(?m)^" + re.escape(name) + r" = .+$",
                     lambda _: f"{name} = {toml(merged)}", runtime)
source = (root / "programs/xfchess-game/src/lib.rs").as_posix()
tests = (root / "programs/xfchess-game/tests/account_security_tests.rs").as_posix()
layout = (root / "programs/xfchess-game/tests/instruction_layout_tests.rs").as_posix()
profile = (root / "programs/xfchess-game/tests/profile_session_tests.rs").as_posix()
text = f'''[workspace]
[package]
name = "xfchess-game"
version = "0.1.0"
edition = "2021"
autotests = false
[lib]
name = "xfchess_game"
path = "{source}"
{runtime}
[dev-dependencies]
anyhow = {toml(workspace['workspace']['dependencies']['anyhow'])}
solana-sdk = {toml(workspace['workspace']['dependencies']['solana-sdk'])}
solana-system-interface = {toml(program['dev-dependencies']['solana-system-interface'])}
sha2 = {toml(program['dev-dependencies']['sha2'])}
[[test]]
name = "instruction_layout_tests"
path = "{layout}"
[[test]]
name = "profile_session_tests"
path = "{profile}"
[[test]]
name = "client_layout_tests"
path = "{(root / 'programs/xfchess-game/tests/client_layout_tests.rs').as_posix()}"
[lints.rust]
unexpected_cfgs = "allow"
[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
undocumented_unsafe_blocks = "deny"
'''
(target / "Cargo.toml").write_text(text, encoding="utf-8")
print(target / "Cargo.toml")
