"""Fetch the hash-pinned public devnet MagicBlock Delegation Program for local SBF CPI tests.

The rollback recipient tests (programs/xfchess-game/tests/rollback_recipient_tests.rs)
run the real Delegation Program binary. No wallet or signing keys are accessed. An
upstream redeployment changes the hash and fails closed until the new binary's
rollback-after-timeout checks have been reviewed against the program source.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
from verify_deployed_program import account, base58

PROGRAM = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh"
PROGRAMDATA = "Ew1j4p6jU82qmLFLJe2SVp5ZKoNokMP6J1Bf5LaZ6GyE"
EXPECTED_SHA256 = "1bc71771d324fa6c4bbf06651a9065ac6686d63e5b37779e8170e9dc8627b00f"
EXPECTED_BYTES = 502_240  # code plus the zero allocation padding
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[2] / "target/deploy/dlp.so")
args = parser.parse_args()
if args.output.exists() and hashlib.sha256(args.output.read_bytes()).hexdigest() == EXPECTED_SHA256:
    print(json.dumps({"fixture": str(args.output), "sha256": EXPECTED_SHA256, "cached": True}))
else:
    program = account(PROGRAM)["value"]
    raw = base64.b64decode(program["data"][0], validate=True)
    if not (program["executable"] and program["owner"] == LOADER and len(raw) == 36
            and int.from_bytes(raw[:4], "little") == 2 and base58(raw[4:]) == PROGRAMDATA):
        raise RuntimeError("Unexpected delegation program ownership or ProgramData address")
    result = account(PROGRAMDATA)
    info = result["value"]
    raw = base64.b64decode(info["data"][0], validate=True)
    if info["owner"] != LOADER or len(raw) != 45 + EXPECTED_BYTES or int.from_bytes(raw[:4], "little") != 3:
        raise RuntimeError("Unexpected delegation ProgramData layout")
    code = raw[45:]
    digest = hashlib.sha256(code).hexdigest()
    if digest != EXPECTED_SHA256 or not code.startswith(b"\x7fELF"):
        raise RuntimeError(f"Delegation program changed: expected {EXPECTED_SHA256}, got {digest}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(code)
    print(json.dumps({"fixture": str(args.output), "program": PROGRAM, "programdata": PROGRAMDATA,
                      "deployment_slot": int.from_bytes(raw[4:12], "little"), "sha256": digest,
                      "bytes": len(code), "cached": False}))
