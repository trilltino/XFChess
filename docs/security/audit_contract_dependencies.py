"""Read-only crates.io inventory for the contract lockfile closure (Python 3.11+).

Usage: py -3 docs/security/audit_contract_dependencies.py
No manifests/lockfiles are changed. Registry failures are recorded, never passes.
"""
import concurrent.futures
import argparse
import csv
import json
from pathlib import Path
import re
import sys
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent


def stable_key(version):
    version = version.split("+")[0]
    if "-" in version:
        return None
    parts = version.split("+")[0].split(".")
    return tuple(map(int, parts)) if all(p.isdigit() for p in parts) else None


def latest(name):
    name_path = name.lower()
    if len(name_path) == 1:
        path = f"1/{name_path}"
    elif len(name_path) == 2:
        path = f"2/{name_path}"
    elif len(name_path) == 3:
        path = f"3/{name_path[0]}/{name_path}"
    else:
        path = f"{name_path[:2]}/{name_path[2:4]}/{name_path}"
    url = f"https://index.crates.io/{path}"
    try:
        request = urllib.request.Request(url, headers={"User-Agent": "XFChess-contract-security-review"})
        with urllib.request.urlopen(request, timeout=45) as response:
            versions = [json.loads(line) for line in response.read().decode().splitlines()]
        stable = [v for v in versions if not v.get("yanked") and stable_key(v["vers"]) is not None]
        if not stable:
            return name, {"latest_stable": None, "registry_url": url,
                          "no_stable_release": True, "error": None}
        selected = max(reversed(stable), key=lambda v: stable_key(v["vers"]))
        return name, {"latest_stable": selected["vers"], "registry_url": url,
                      "latest_dependencies": selected["deps"],
                      "latest_features": selected.get("features", {}) | selected.get("features2", {}),
                      "rust_version": selected.get("rust_version"), "error": None}
    except Exception as error:
        return name, {"latest_stable": None, "registry_url": url, "error": str(error)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--date", default="20261023")
    parser.add_argument("--retry-failed", action="store_true")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]{8}", args.date):
        parser.error("--date must be YYYYMMDD")
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    packages = lock["package"]
    by_name = {}
    for i, package in enumerate(packages):
        by_name.setdefault(package["name"], []).append(i)

    def resolve(dependency):
        parts = dependency.split()
        candidates = by_name[parts[0]]
        if len(parts) > 1:
            candidates = [i for i in candidates if packages[i]["version"] == parts[1]]
        if len(parts) > 2:
            source = " ".join(parts[2:]).strip("()")
            candidates = [i for i in candidates if packages[i].get("source") == source]
        if len(candidates) != 1:
            raise ValueError(f"Ambiguous lockfile dependency: {dependency}")
        return candidates[0]

    roots = [i for i, p in enumerate(packages) if p["name"] in
             {"xfchess-game", "xfchess-game-sbf", "xfchess-rules", "nimzovich_engine", "xfchess-client-compat"}]
    included, parents, direct = set(), {}, set()
    # A root can also be visited transitively before its own traversal. Classify
    # every root's dependencies up front instead of relying on visit order.
    for root in roots:
        direct.update(resolve(dep) for dep in packages[root].get("dependencies", []))

    def visit(i):
        if i in included:
            return
        included.add(i)
        for dependency in packages[i].get("dependencies", []):
            child = resolve(dependency)
            parents.setdefault(child, set()).add(f'{packages[i]["name"]}@{packages[i]["version"]}')
            if i in roots:
                direct.add(child)
            visit(child)

    for root in roots:
        visit(root)
    names = sorted({packages[i]["name"] for i in included if packages[i].get("source", "").startswith("registry+")})
    registry_path = OUT / f"dependency-registry-{args.date}.json"
    registry = json.loads(registry_path.read_text(encoding="utf-8")) if args.retry_failed and registry_path.exists() else {}
    pending = [name for name in names if name not in registry or registry[name].get("error")]
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        registry.update(pool.map(latest, pending))
    registry_path.write_text(json.dumps(registry, indent=2) + "\n", encoding="utf-8")
    rows = []
    for i in sorted(included, key=lambda j: (packages[j]["name"], packages[j]["version"])):
        package = packages[i]
        current = registry.get(package["name"], {})
        target = current.get("latest_stable")
        status = "AT_LATEST" if target == package["version"] else "UPGRADE_REQUIRED" if target else "UNVERIFIED"
        if current.get("no_stable_release"):
            status = "NO_STABLE_RELEASE"
        if "source" not in package:
            status = "LOCAL_PACKAGE"
        elif package["source"].startswith("git+"):
            status = "GIT_REVISION_REVIEW_REQUIRED"
        rows.append({"package": package["name"], "version": package["version"],
                     "source": package.get("source", "workspace"),
                     "kind": "root" if i in roots else "direct" if i in direct else "transitive",
                     "parents": ";".join(sorted(parents.get(i, []))),
                     "latest_stable": target or "", "selected_latest_compatible": "NOT_RESOLVED",
                     "features": "LOCKFILE_DOES_NOT_RECORD_FEATURES",
                     "advisory_status": "NOT_AUDITED", "migration_status": status,
                     "registry_url": current.get("registry_url", ""), "error": current.get("error") or ""})
    with (OUT / f"dependency-inventory-{args.date}.csv").open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    print(json.dumps({"packages": len(rows), "registry_packages": len(names),
                      "outdated": sum(r["migration_status"] == "UPGRADE_REQUIRED" for r in rows),
                      "registry_errors": sum(bool(v.get("error")) for v in registry.values())}))


if __name__ == "__main__":
    main()
