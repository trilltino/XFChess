"""Record the local SBF artifact and the exact program sources used to review it."""
from pathlib import Path
import argparse
import hashlib
import json
import re

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--artifact", default="target/deploy/xfchess_game.so")
parser.add_argument("--date", default="20261023")
args = parser.parse_args()
if not re.fullmatch(r"[0-9]{8}", args.date):
    parser.error("--date must be YYYYMMDD")
binary = (root / args.artifact).resolve()
sources = sorted((root / "programs/xfchess-game/src").rglob("*.rs"))
sources += sorted((root / "programs/xfchess-game-sbf/src").rglob("*.rs"))
sources += [root / "programs/xfchess-game/Cargo.toml",
            root / "programs/xfchess-game-sbf/Cargo.toml", root / "Cargo.toml", root / "Cargo.lock"]
newer = [str(path.relative_to(root)) for path in sources
         if path.stat().st_mtime > binary.stat().st_mtime
         and path.suffix == ".rs"]
if newer:
    raise RuntimeError(f"Sources newer than the SBF artifact; rebuild first: {newer}")
data = binary.read_bytes()
registry = json.loads((root / "docs/security/instruction-registry-20261023.json").read_text())
missing = [item["name"] for item in registry if bytes(item["discriminator"]) not in data]
if missing:
    raise RuntimeError(f"Instruction discriminators absent from SBF artifact: {missing}")
manifest = {"artifact": str(binary.relative_to(root)),
            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
            "deployment": False, "instruction_count": len(registry),
            "missing_discriminators": missing,
            "source_files": {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
                             for path in sources}}
destination = root / f"docs/security/artifact-manifest-{args.date}.json"
destination.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print(json.dumps({key: value for key, value in manifest.items() if key != "source_files"}))
