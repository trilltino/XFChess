"""Inspect direct frame-pointer accesses in a linked SBF ELF, not archive warnings.

This supplements runtime verification; it does not prove bounds of aliased or
dynamically computed addresses, or replace the Solana loader/verifier.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("artifact", type=Path)
parser.add_argument("--objdump", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
text = subprocess.check_output([str(args.objdump), "-d", str(args.artifact)], text=True)
accesses = []
for match in re.finditer(r"\[r10\s*([+-])\s*(0x[0-9a-f]+|[0-9]+)\]", text):
    accesses.append(int(match[2], 0) * (-1 if match[1] == "-" else 1))
if not accesses:
    raise RuntimeError("No frame-pointer accesses parsed; inspection is inconclusive")
invalid = [offset for offset in accesses if offset < -4096 or offset >= 0]
report = {
    "artifact": str(args.artifact),
    "sha256": hashlib.sha256(args.artifact.read_bytes()).hexdigest(),
    "direct_frame_accesses": len(accesses),
    "minimum_offset": min(accesses),
    "maximum_offset": max(accesses),
    "out_of_frame_accesses": invalid,
    "scope": "Direct r10-relative memory accesses only; not a full verifier",
}
args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report))
if invalid:
    raise RuntimeError("Linked ELF contains direct memory accesses outside its stack frame")
