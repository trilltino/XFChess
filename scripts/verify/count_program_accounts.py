import base64
import json
import sys
import urllib.request
from collections import Counter

"""Count a program's on-chain accounts by type.

Usage: py -3 -I scripts/verify/count_program_accounts.py [program_id] [rpc_url]
"""
from pathlib import Path

PROGRAM = sys.argv[1] if len(sys.argv) > 1 else "6idpQeH5YzXUT4sg8mG8dzxdK7ytmxfEyWkzUcNJJv4N"
RPC = sys.argv[2] if len(sys.argv) > 2 else "https://api.devnet.solana.com"
IDL = Path(__file__).resolve().parents[2] / "apps/website/src/lib/xfchess_game.json"

idl = json.load(open(IDL, encoding="utf-8"))
names = {bytes(a["discriminator"]): a["name"] for a in idl.get("accounts", [])}

body = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getProgramAccounts",
    "params": [
        PROGRAM,
        {
            "encoding": "base64",
            "dataSlice": {"offset": 0, "length": 8},
            "commitment": "confirmed",
        },
    ],
}
req = urllib.request.Request(
    RPC, json.dumps(body).encode(), {"Content-Type": "application/json"}
)
with urllib.request.urlopen(req, timeout=60) as resp:
    data = json.load(resp)
if "error" in data:
    print("RPC error:", data["error"])
    sys.exit(1)
counts = Counter()
for item in data["result"]:
    disc = base64.b64decode(item["account"]["data"][0])
    counts[names.get(disc, "unknown:" + disc.hex())] += 1
print("total program accounts:", sum(counts.values()))
for name, n in counts.most_common():
    print(f"  {name}: {n}")
