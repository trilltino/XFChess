"""Fetch the hash-pinned public devnet pass program for local SBF CPI tests.

No wallet or signing keys are accessed. An upstream deployment change fails
closed until the fixture hash and corresponding integration behavior are reviewed.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
from verify_deployed_program import account, base58

PROGRAM = "4sFBJCjU3QvoWZCfxdHJcGBCPjToTy2BzNK23wmRPaw9"
PROGRAMDATA = "F954fgacEimqRJQr9Sx1zUi7C9QeLMPpXQ438Ns3VBwB"
EXPECTED_SHA256 = "06a784b08d55cdff44c7b38272bfbf8d1e0f7ad368e4dbd1ed6b494ecd4b57ff"
EXPECTED_BYTES = 230_000
LOADER = "BPFLoaderUpgradeab1e11111111111111111111111"

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[2] / "target/deploy/xfchess_pass.so")
args = parser.parse_args()
if args.output.exists() and hashlib.sha256(args.output.read_bytes()).hexdigest() == EXPECTED_SHA256:
    print(json.dumps({"fixture": str(args.output), "sha256": EXPECTED_SHA256, "cached": True}))
else:
    program = account(PROGRAM)["value"]
    raw = base64.b64decode(program["data"][0], validate=True)
    if not (program["executable"] and program["owner"] == LOADER and len(raw) == 36
            and int.from_bytes(raw[:4], "little") == 2 and base58(raw[4:]) == PROGRAMDATA):
        raise RuntimeError("Unexpected pass program ownership or ProgramData address")
    result = account(PROGRAMDATA)
    info = result["value"]
    raw = base64.b64decode(info["data"][0], validate=True)
    if info["owner"] != LOADER or len(raw) != 45 + EXPECTED_BYTES or int.from_bytes(raw[:4], "little") != 3:
        raise RuntimeError("Unexpected pass ProgramData layout")
    code = raw[45:]
    digest = hashlib.sha256(code).hexdigest()
    if digest != EXPECTED_SHA256 or not code.startswith(b"\x7fELF"):
        raise RuntimeError(f"Pass program changed: expected {EXPECTED_SHA256}, got {digest}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(code)
    print(json.dumps({"fixture": str(args.output), "program": PROGRAM, "programdata": PROGRAMDATA,
                      "deployment_slot": int.from_bytes(raw[4:12], "little"), "sha256": digest,
                      "bytes": len(code), "cached": False}))
