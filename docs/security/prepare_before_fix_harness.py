"""Build a comparison fixture, retaining v2 but reverting only the four audit fixes.

This operates solely in a requested temporary directory. Expected test failures
demonstrate guard regressions; it is not an artifact suitable for deployment.
"""
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
target = Path(sys.argv[1]).resolve()
target.mkdir(parents=True, exist_ok=True)
subprocess.run([sys.executable, str(Path(__file__).with_name("prepare_native_harness.py")), str(target)], check=True)
shutil.copytree(root / "programs/xfchess-game/src", target / "src", dirs_exist_ok=True)
relative = "programs/xfchess-game/src/"
for name in ["account_ix/profile.rs", "account_ix/withdraw.rs", "delegation_ix/undelegation.rs", "governance_ix/recover_stuck_delegation.rs"]:
    original = subprocess.check_output(["git", "show", f"HEAD:{relative}{name}"], cwd=root)
    (target / "src" / name).write_bytes(original)
original = subprocess.check_output(["git", "show", f"HEAD:{relative}lib.rs"], cwd=root).decode()
current_path = target / "src/lib.rs"
current = current_path.read_text(encoding="utf-8")
marker = "    pub fn process_undelegation("
following = "    #[allow("
original_start = original.index(marker)
current_start = current.index(marker)
original_end = original.index(following, original_start)
current_end = current.index(following, current_start)
current_path.write_text(current[:current_start] + original[original_start:original_end] + current[current_end:], encoding="utf-8")
manifest = target / "Cargo.toml"
text = manifest.read_text(encoding="utf-8")
text = text.replace((root / "programs/xfchess-game/src/lib.rs").as_posix(), (target / "src/lib.rs").as_posix())
manifest.write_text(text, encoding="utf-8")
print("Comparison harness prepared: EXPECT SECURITY REGRESSION TEST FAILURES")
