"""Filter a real cargo-audit JSON result to the recorded contract closure."""
import csv
import json
from pathlib import Path
import sys
import argparse
import re

out = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("audit", type=Path)
parser.add_argument("--date", default="20261023")
args = parser.parse_args()
if not re.fullmatch(r"[0-9]{8}", args.date):
    parser.error("--date must be YYYYMMDD")
raw = args.audit.read_bytes()
text = raw.decode("utf-16" if raw.startswith(b"\xff\xfe") else "utf-8-sig")
audit = json.loads(text[text.index('{'):])
inventory_path = out / f"dependency-inventory-{args.date}.csv"
with inventory_path.open(encoding="utf-8", newline="") as stream:
    rows = list(csv.DictReader(stream))
closure = {(row["package"], row["version"]) for row in rows}
filtered = []
matches = {}
for finding in audit["vulnerabilities"]["list"]:
    package, advisory = finding["package"], finding["advisory"]
    key = (package["name"], package["version"])
    if key not in closure:
        continue
    matches.setdefault(key, []).append(advisory["id"])
    filtered.append({"package": key[0], "version": key[1], "advisory": advisory["id"],
        "title": advisory["title"], "url": advisory.get("url"),
        "patched": finding["versions"]["patched"], "affected": finding.get("affected"),
        "reachability": "NOT_ESTABLISHED: closure includes host/test/build dependencies; no claim of on-chain exploitability"})
warnings = []
for kind, items in audit.get("warnings", {}).items():
    for item in items:
        package = item["package"]
        key = (package["name"], package["version"])
        if key not in closure:
            continue
        advisory = item.get("advisory") or {}
        label = advisory.get("id") or kind
        matches.setdefault(key, []).append(f"{kind}:{label}")
        warnings.append({"package": key[0], "version": key[1], "kind": kind,
            "advisory": advisory.get("id"), "title": advisory.get("title"),
            "url": advisory.get("url"), "reachability": "REVIEW_REQUIRED"})
for row in rows:
    found = matches.get((row["package"], row["version"]), [])
    row["advisory_status"] = ";".join(found) + ":REACHABILITY_REVIEW_REQUIRED" if found else "NO_VULNERABILITY_MATCH_IN_RECORDED_DATABASE"
with inventory_path.open("w", encoding="utf-8", newline="") as stream:
    writer = csv.DictWriter(stream, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows(rows)
report = {"database": audit["database"], "workspace_vulnerabilities": audit["vulnerabilities"]["count"],
    "contract_closure_vulnerabilities": len(filtered), "findings": filtered,
    "contract_warnings": warnings,
    "warning_kinds": {kind: len(items) for kind, items in audit.get("warnings", {}).items()}}
(out / f"contract-advisories-{args.date}.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"workspace": report["workspace_vulnerabilities"], "contract_closure": len(filtered),
                  "packages": [f'{f["package"]}@{f["version"]}:{f["advisory"]}' for f in filtered]}))
