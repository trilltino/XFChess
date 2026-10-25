"""Archive concise excerpts and hashes of this session's local verification logs.

Only explicitly named build/test logs are read; no keys or application logs.
Full temporary logs contain large Windows OpenSSL linker diagnostics. This
archive preserves test results and failures without copying those diagnostics.
"""
from pathlib import Path
import hashlib
import os
import re

root = Path(__file__).resolve().parents[2]
temporary = Path(os.environ["TEMP"])
names = [
    "verified-tests", "clippy-final", "all-targets-final", "sbf-funding-fix",
    "program-tests-final", "settlement-tests", "wallet-lifecycle-tests",
    "tournament-session-tests", "rules-tests", "engine-standalone",
    "before-fixes", "program-tests", "production", "format-final",
    "backend-check",
]
result = ["Verification output excerpts — 23 October 2026",
          "Repeated test runs are not additional coverage. Expected-failure logs are labeled.",
          "Full-log SHA-256 values bind the captured temporary output; excerpts omit linker noise.", ""]
for name in names:
    path = temporary / f"xfchess-security-{name}.log"
    if not path.exists():
        result.append(f"MISSING LOG: {path.name}")
        continue
    raw = path.read_bytes()
    text = raw.decode("utf-16" if raw.startswith(b"\xff\xfe") else "utf-8-sig").replace("\r", "")
    result.extend([f"=== {path.name} ===", f"Full-log SHA-256: {hashlib.sha256(raw).hexdigest()}"])
    if name == "before-fixes":
        result.append("EXPECTED FAILURE: intentionally vulnerable comparison source, never deployed.")
    if name == "program-tests":
        result.append("INITIAL FAILURE: vault funding order; fixed by sbf-funding-fix and later passing runs.")
    if name in {"production", "format-final"}:
        result.append("EXPECTED OPEN RELEASE GATE: see verification report; not a passing check.")
    if name in {"production", "format-final", "all-targets-final", "clippy-final", "backend-check"}:
        lines = [line for line in text.splitlines()
                 if re.search(r"Finished |^error|^warning|^Diff in |panicked|evaluation panicked", line)]
    else:
        lines = [line for line in text.splitlines()
                 if re.search(r"^test |^running \d+ tests|^failures:|^---- .* ----|^Error:|^Caused by:|^error|Finished |^     Running ", line)]
    result.extend(lines)
    result.append("")
destination = root / "docs/security/verification-output-20261023.txt"
destination.write_text("\n".join(result), encoding="utf-8")
print(f"Archived {len(names)} named log excerpts to {destination}")
