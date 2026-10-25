"""Read-only devnet program provenance check; never reads authority keyfiles."""
import base64
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
import re

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def base58(raw):
    value = int.from_bytes(raw, "big")
    result = ""
    while value:
        value, digit = divmod(value, 58)
        result = ALPHABET[digit] + result
    return "1" * (len(raw) - len(raw.lstrip(b"\0"))) + result


def account(key):
    payload = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "getAccountInfo",
        "params": [key, {"encoding": "base64", "commitment": "finalized"}]}).encode()
    request = urllib.request.Request("https://api.devnet.solana.com", data=payload,
        headers={"Content-Type": "application/json", "User-Agent": "XFChess-security-review"})
    with urllib.request.urlopen(request, timeout=45) as response:
        result = json.load(response)
    if "error" in result or not result.get("result", {}).get("value"):
        raise RuntimeError(str(result))
    return result["result"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact", type=Path)
    parser.add_argument("--date", default="20261023")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]{8}", args.date):
        parser.error("--date must be YYYYMMDD")
    program = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
    result = account(program)
    info = result["value"]
    raw = base64.b64decode(info["data"][0], validate=True)
    assert info["executable"] and info["owner"] == "BPFLoaderUpgradeab1e11111111111111111111111"
    assert len(raw) == 36 and int.from_bytes(raw[:4], "little") == 2
    programdata = base58(raw[4:])
    result = account(programdata)
    info = result["value"]
    raw = base64.b64decode(info["data"][0], validate=True)
    assert info["owner"] == "BPFLoaderUpgradeab1e11111111111111111111111"
    assert int.from_bytes(raw[:4], "little") == 3 and raw[12] in (0, 1)
    code = raw[45:]
    expected_length = 1_094_544
    expected_hash = "f9a6d1c3f417c33e602a036fccdd8a01c970d092978319dd401c3192bc7f70c3"
    if args.artifact:
        artifact = args.artifact.read_bytes()
        expected_length = len(artifact)
        expected_hash = hashlib.sha256(artifact).hexdigest()
    digest = hashlib.sha256(code[:expected_length]).hexdigest()
    report = {"cluster": "devnet", "program": program, "programdata": programdata,
        "read_slot": result["context"]["slot"], "deployment_slot": int.from_bytes(raw[4:12], "little"),
        "upgrade_authority": base58(raw[13:45]) if raw[12] else None,
        "programdata_bytes": len(raw), "expected_code_bytes": expected_length,
        "code_sha256": digest, "padding_all_zero": not any(code[expected_length:]),
        "expected_code_sha256": expected_hash,
        "matches_expected_code": digest == expected_hash and not any(code[expected_length:])}
    destination = Path(__file__).resolve().parent / f"deployed-program-{args.date}.json"
    destination.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    if not report["matches_expected_code"]:
        raise RuntimeError("Deployed program does not match the expected artifact")


if __name__ == "__main__":
    main()
