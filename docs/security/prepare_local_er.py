"""Download a small pinned local ER stack, never a live key or chain snapshot.

Usage: py -3 docs/security/prepare_local_er.py <temporary-directory>
Reuses the installed ubuntu:24.04 image. Network payload is capped at 256 MiB.
The downloaded npm fixtures are public genesis accounts, not signing keypairs.
"""
from pathlib import Path
import base64
import hashlib
import io
import json
import sys
import tarfile
import urllib.request

target = Path(sys.argv[1]).resolve()
target.mkdir(parents=True, exist_ok=True)
limit = 256 * 1024 * 1024
used = 0
manifest = []


def fetch(url, maximum=limit):
    global used
    request = urllib.request.Request(url, headers={"User-Agent": "XFChess-local-ER-tests"})
    with urllib.request.urlopen(request, timeout=90) as response:
        length = int(response.headers.get("Content-Length", 0))
        if length and (length > maximum or used + length > limit):
            raise RuntimeError(f"Download budget exceeded: {url} ({length} bytes)")
        raw = response.read(min(maximum, limit - used) + 1)
    if len(raw) > maximum or used + len(raw) > limit:
        raise RuntimeError("Download budget exceeded")
    used += len(raw)
    manifest.append({"url": url, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
    return raw


for repository, version, asset_name, destination in [
    ("anza-xyz/agave", "v4.3.2", "solana-release-x86_64-unknown-linux-gnu.tar.bz2", "solana.tar.bz2"),
    ("magicblock-labs/magicblock-validator", "v1.1.1", "ephemeral-validator-linux-x64-glibc", "ephemeral-validator"),
]:
    release = json.loads(fetch(f"https://api.github.com/repos/{repository}/releases/tags/{version}", 2_000_000))
    asset = next(item for item in release["assets"] if item["name"] == asset_name)
    path = target / destination
    if not path.exists():
        print(f"Downloading {asset_name}: {asset['size'] / 1024**2:.1f} MiB", flush=True)
        raw = fetch(asset["browser_download_url"], asset["size"])
        digest = asset.get("digest")
        if digest:
            assert digest == "sha256:" + hashlib.sha256(raw).hexdigest()
        path.write_bytes(raw)
    else:
        raw = path.read_bytes()
        manifest.append({"url": asset["browser_download_url"], "bytes": len(raw),
                         "sha256": hashlib.sha256(raw).hexdigest(), "cached": True})
        if asset.get("digest"):
            assert asset["digest"] == "sha256:" + hashlib.sha256(raw).hexdigest()
    if destination.endswith("bz2"):
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:bz2") as archive:
            member = next(item for item in archive.getmembers() if item.name.endswith("/bin/solana-test-validator"))
            executable = archive.extractfile(member)
            assert executable is not None
            (target / "solana-test-validator").write_bytes(executable.read())

npm = json.loads(fetch("https://registry.npmjs.org/@magicblock-labs/ephemeral-validator/1.1.1", 2_000_000))
raw = fetch(npm["dist"]["tarball"], 5_000_000)
assert npm["dist"]["integrity"] == "sha512-" + base64.b64encode(hashlib.sha512(raw).digest()).decode()
with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
    archive.extractall(target / "npm", filter="data")
dumps = target / "npm/package/bin/local-dumps"
if not dumps.exists():
    dumps = next((target / "npm").rglob("local-dumps"))
args = []
for path in sorted(dumps.glob("*.so")):
    args += ["--bpf-program", path.stem, "/audit/" + path.relative_to(target).as_posix()]
for path in sorted(dumps.glob("*.json")):
    args += ["--account", path.stem, "/audit/" + path.relative_to(target).as_posix()]
# Program ID is checked against the source before running; no wallet config is read.
root = Path(__file__).resolve().parents[2]
import re
text = (root / "programs/xfchess-game/src/lib.rs").read_text()
match = re.search(r'#\[cfg\(any\(feature = "localnet", feature = "devnet"\)\)\]\s*declare_id!\("([^"]+)"\)', text)
if not match:
    raise RuntimeError("Cannot identify the devnet test program")
# Copy the explicitly built localnet artifact to this path before starting the
# stack. A devnet artifact uses different routing and is not interchangeable.
args += ["--bpf-program", match[1], "/audit/xfchess_game-localnet.so"]
import shlex
base = '/audit/solana-test-validator --reset --quiet --ledger /tmp/xfchess-base-ledger --rpc-port 18899 --bind-address "$audit_bind" --limit-blockstore-size 50000 ' + shlex.join(args)
(target / "start-base.sh").write_text('#!/bin/sh\nset -eu\naudit_bind="$(hostname -I)"\naudit_bind="${audit_bind%% *}"\nexec ' + base + "\n", encoding="utf-8", newline="\n")
(target / "download-manifest.json").write_text(json.dumps({"budget_bytes": limit, "downloaded_bytes": used, "files": manifest}, indent=2) + "\n")
print(json.dumps({"downloaded_MiB": round(used / 1024**2, 2), "genesis_programs": len(list(dumps.glob('*.so'))), "target": str(target)}))
