"""Resolve older workspace pins that prevent the real contract migration.

Only updates an existing direct workspace declaration named by Cargo's resolver,
using its freshly queried latest stable release. Does not patch upstream crates,
lower versions, alter feature flags, or suppress compiler errors.
"""
from pathlib import Path
import json
import os
import re
import subprocess
import sys
import urllib.request

root = Path(__file__).resolve().parents[2]
manifest = root / "Cargo.toml"
log = Path(os.environ["TEMP"]) / "xfchess-latest-actual-check.log"
record = root / "docs/security/latest-direct-targets-20261024.json"
for attempt in range(15):
    with log.open("wb") as stream:
        result = subprocess.run(["cargo", "check", "-p", "xfchess-game", "--lib"],
                                cwd=root, stdout=stream, stderr=subprocess.STDOUT)
    output = log.read_text(encoding="utf-8", errors="replace")
    if result.returncode == 0:
        print(output[-2500:])
        sys.exit(0)
    failure = re.search(r'failed to select a version for `([^`]+)`', output)
    if not failure:
        print(output[-7000:])
        sys.exit(result.returncode)
    name = failure[1]
    text = manifest.read_text()
    start = text.index("[workspace.dependencies]")
    end = text.find("\n[", start + 1)
    if end < 0:
        end = len(text)
    section = text[start:end]
    pattern = r'(?m)^(' + re.escape(name) + r'\s*=\s*)("[^"]+"|\{[^\n]+\})'
    declaration = re.search(pattern, section)
    if not declaration or any(word in declaration[0] for word in ["path =", "git ="]):
        print(output[-5000:])
        sys.exit(result.returncode)
    request = urllib.request.Request("https://crates.io/api/v1/crates/" + name,
                                     headers={"User-Agent": "XFChess-latest-migration"})
    with urllib.request.urlopen(request, timeout=25) as response:
        version = json.load(response)["crate"]["max_stable_version"]
    current = declaration[0]
    if "{" in current:
        updated = re.sub(r'version\s*=\s*"[^"]+"', 'version = "=' + version + '"', current)
    else:
        updated = declaration[1] + '"=' + version + '"'
    if updated == current:
        # Cargo resolves the shared workspace together. Align another local
        # package's exact pin when it excludes the selected stable release.
        aligned = []
        paths = subprocess.check_output(["rg", "--files", "-g", "Cargo.toml",
            "libs", "backend", "programs", "tools/cu_benchmark"], cwd=root, text=True).splitlines()
        for relative in paths:
            path = root / relative
            local = path.read_text()
            def align(match):
                line = match[0]
                if any(word in line for word in ["workspace", "path =", "git ="]):
                    return line
                if "{" in line:
                    return re.sub(r'version\s*=\s*"[^"]+"', 'version = "=' + version + '"', line)
                return match[1] + '"=' + version + '"'
            changed = re.sub(pattern, align, local)
            if changed != local:
                path.write_text(changed, newline="\n")
                aligned.append(relative)
        if aligned:
            print(f"Aligned {name} -> {version} in {aligned}", flush=True)
            continue
        print(output[-5000:])
        sys.exit(result.returncode)
    section = section[:declaration.start()] + updated + section[declaration.end():]
    manifest.write_text(text[:start] + section + text[end:], newline="\n")
    targets = json.loads(record.read_text())
    targets[name] = version
    record.write_text(json.dumps(targets, indent=2) + "\n", newline="\n")
    print(f"Updated actual workspace pin: {name} -> {version}", flush=True)
raise RuntimeError("Resolver migration exceeded its bounded iteration count")
