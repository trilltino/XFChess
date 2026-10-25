"""Prepare a reproducible latest-release CPI type-boundary compilation spike."""
import json
from pathlib import Path
import sys

out = Path(__file__).resolve().parent
registry = json.loads((out / "dependency-registry-20261023.json").read_text(encoding="utf-8"))
target = Path(sys.argv[1]).resolve()
(target / "src").mkdir(parents=True, exist_ok=True)
names = ["anchor-lang", "anchor-spl", "ephemeral-rollups-sdk", "solana-program"]
versions = {name: registry[name]["latest_stable"] for name in names}
if any(v is None for v in versions.values()):
    raise RuntimeError("Missing registry data; cannot select release")
deps = "\n".join(f'{name} = {{ version = "={version}"' +
    (', features = ["anchor", "instruction"]' if name == "ephemeral-rollups-sdk" else '') + ' }'
    for name, version in versions.items())
(target / "Cargo.toml").write_text(f'''[workspace]
[package]
name = "xfchess-latest-contract-spike"
version = "0.0.0"
edition = "2021"
[dependencies]
{deps}
''', encoding="utf-8")
(target / "src/lib.rs").write_text('''use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn callback_boundary<'a>(
    game: &AccountInfo<'a>, owner: &Pubkey, buffer: &AccountInfo<'a>,
    payer: &AccountInfo<'a>, system: &AccountInfo<'a>, seeds: Vec<Vec<u8>>,
) {
    let _ = ephemeral_rollups_sdk::cpi::undelegate_account(game, owner, buffer, payer, system, seeds);
}
''', encoding="utf-8")
print(json.dumps(versions))
