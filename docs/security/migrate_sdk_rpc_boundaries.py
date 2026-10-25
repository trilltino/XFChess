"""Apply only the SDK/RPC wire-boundary edits, preserving other working changes.

Run against the source checkout and the isolated review checkout. This avoids
copying unrelated dirty application files into the pull request.
"""
import argparse
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("root", type=Path)
args = parser.parse_args()
root = args.root.resolve()
names = {
    "send_transaction": "send_sdk_transaction",
    "send_transaction_with_config": "send_sdk_transaction_with_config",
    "send_and_confirm_transaction": "send_sdk_and_confirm_transaction",
    "send_and_confirm_transaction_with_spinner_and_config": "send_sdk_and_confirm_transaction_with_spinner_and_config",
    "simulate_transaction": "simulate_sdk_transaction",
    "simulate_transaction_with_config": "simulate_sdk_transaction_with_config",
}
pattern = re.compile(r"\.(" + "|".join(names) + r")(?=\s*\()")
changed = []
for base in ("src", "backend/src", "tools/cu_benchmark/src"):
    for path in (root / base).rglob("*.rs"):
        text = path.read_text(encoding="utf-8-sig")
        updated = pattern.sub(lambda m: "." + names[m[1]], text)
        updated = updated.replace("solana_client::rpc_client::SerializableTransaction", "xfchess_client_compat::SdkTransaction")
        if updated == text:
            continue
        lines = updated.splitlines(keepends=True)
        index = 0
        while index < len(lines):
            line = lines[index].lstrip()
            if not line.strip() or line.startswith("//!") or (line.startswith("//") and not line.startswith("///")):
                index += 1
            elif line.startswith("#!["):
                depth = 0
                while index < len(lines):
                    depth += lines[index].count("[") - lines[index].count("]")
                    index += 1
                    if depth == 0:
                        break
            else:
                break
        if "use xfchess_client_compat::prelude::*;" not in updated:
            lines.insert(index, "use xfchess_client_compat::prelude::*;\n")
        path.write_text("".join(lines), encoding="utf-8", newline="\n")
        changed.append(path.relative_to(root).as_posix())
print(json.dumps({"root": str(root), "changed": changed}, indent=2))
