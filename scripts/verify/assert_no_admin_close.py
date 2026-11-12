"""Check whether a built program binary contains the devnet reset instruction.

Usage:
  py -3 -I scripts/verify/assert_no_admin_close.py [program.so]            # must NOT contain it
  py -3 -I scripts/verify/assert_no_admin_close.py [program.so] --expect   # must contain it

Run it on every build you deploy: a normal build must not carry `admin_close_account`.
"""
import hashlib
import sys
from pathlib import Path

args = [a for a in sys.argv[1:] if not a.startswith("--")]
expect = "--expect" in sys.argv
path = Path(args[0]) if args else Path("target/deploy/xfchess_game.so")
if not path.is_file():
    print(f"{path} not found")
    sys.exit(2)

blob = path.read_bytes()
discriminator = hashlib.sha256(b"global:admin_close_account").digest()[:8]
found = b"AdminCloseAccount" in blob or discriminator in blob

if found and not expect:
    print(f"FAIL: {path} contains the admin_close_account instruction")
    sys.exit(1)
if not found and expect:
    print(f"FAIL: {path} does not contain the admin_close_account instruction")
    sys.exit(1)
print(f"OK: {path} {'contains' if found else 'does not contain'} admin_close_account")
