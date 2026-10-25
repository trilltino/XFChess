"""Assemble Anchor's actual IDL print fragments and declare trailing payout records.

Input is the output of __anchor_private_print_idl with --test-threads=1.
Normal generation must leave ANCHOR_IDL_BUILD_SKIP_LINT unset. For historical
logs generated with that bypass, explicitly pass --lint-skipped.
"""
import json
from pathlib import Path
import re
import sys
import subprocess
import argparse

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("log", type=Path)
parser.add_argument("--lint-skipped", action="store_true")
args = parser.parse_args()
raw = args.log.read_bytes()
text = raw.decode("utf-16" if raw.startswith(b"\xff\xfe") else "utf-8-sig").replace("\r", "")
if "test result: ok." not in text:
    raise RuntimeError("IDL print tests did not pass")
blocks = re.findall(r"--- IDL begin (program|event|errors|const) ---\n(.*?)--- IDL end \1 ---", text, re.S)
programs = [json.loads(payload) for kind, payload in blocks if kind == "program"]
if len(programs) != 1:
    raise ValueError(f"Expected one program fragment, found {len(programs)}")
idl = programs[0]
types = {item["name"]: item for item in idl.get("types", [])}
idl["events"], idl["constants"] = [], []
for kind, payload in blocks:
    value = json.loads(payload)
    if kind == "event":
        idl["events"].append(value["event"])
        types.update((item["name"], item) for item in value["types"])
    elif kind == "errors":
        idl["errors"] = value
    elif kind == "const":
        idl["constants"].append(value)
idl["types"] = list(types.values())
address = re.search(r'--- IDL begin address ---\n([^\n]+)', text)
if not address:
    raise ValueError("Missing generated program address")
idl["address"] = re.sub(r'[^A-Za-z0-9]', '', address[1])
if idl["address"] != "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU":
    raise ValueError("Unexpected generated program address")
# Same unambiguous module-path normalization as anchor-lang-idl's builder.
encoded = json.dumps(idl)
for qualified, short in re.findall(r'"((?:\w+::)+(\w+))"', encoded):
    changed = encoded.replace(f'"{qualified}"', f'"{short}"')
    if not re.search(r'"(?:\w+::)+' + re.escape(short) + r'"', changed):
        encoded = changed
idl = json.loads(encoded)
for kind in ["accounts", "constants", "events", "instructions", "types"]:
    idl[kind] = sorted(idl.get(kind, []), key=lambda item: item["name"])
for kind in ["accounts", "events", "instructions"]:
    discriminators = [tuple(item["discriminator"]) for item in idl[kind]]
    if len(discriminators) != len(set(discriminators)):
        raise ValueError(f"Duplicate {kind} discriminator")

destination = root / "xfchessdotcom/src/lib/xfchess_game.json"
previous = json.loads(destination.read_text(encoding="utf-8"))
old_types = {item["name"]: item for item in previous["types"]}
new_types = {item["name"]: item for item in idl["types"]}
for name in ["Game", "PlayerProfile"]:
    if old_types[name]["type"] != new_types[name]["type"]:
        raise ValueError(f"Unexpected serialized layout change: {name}")
old_instructions = {item["name"]: item for item in previous["instructions"]}
preexisting_stale_args = []
for instruction in idl["instructions"]:
    old = old_instructions.get(instruction["name"])
    if old and old["discriminator"] != instruction["discriminator"]:
        raise ValueError(f'Unexpected old instruction ABI change: {instruction["name"]}')
    if old and old["args"] != instruction["args"]:
        # The checked-in IDL still had the removed elo argument. Verify that
        # source HEAD already used the generated one-argument ABI before this work.
        deployed_source = subprocess.check_output(["git", "show", "HEAD:programs/xfchess-game/src/lib.rs"], cwd=root).decode()
        if instruction["name"] != "register_player" or not re.search(
            r"pub fn register_player\(\s*ctx: Context<RegisterPlayer>,\s*tournament_id: u64,?\s*\)", deployed_source):
            raise ValueError(f'Unexpected old instruction argument change: {instruction["name"]}')
        preexisting_stale_args.append(instruction["name"])
    if instruction["name"] in {"finalize_game", "cancel_game", "withdraw_expired_wager", "resolve_dispute", "claim_stale_dispute"}:
        # These are runtime-validated remaining_accounts; Anchor cannot infer them.
        instruction["accounts"].append({"name": "recovery_identity", "writable": True,
            "pda": {"seeds": [{"kind": "const", "value": list(b"recovery_identity")},
                              {"kind": "arg", "path": "game_id"}]}})
    elif instruction["name"] in {"create_game", "join_game", "global_create_game", "global_join_game",
                                  "create_tournament_game", "session_create_game", "session_join_game"}:
        instruction["accounts"].append({"name": "recovery_identity",
            "pda": {"seeds": [{"kind": "const", "value": list(b"recovery_identity")},
                              {"kind": "arg", "path": "game_id"}]}})
for old in previous["errors"]:
    if not any(new["code"] == old["code"] and new["name"] == old["name"] for new in idl["errors"]):
        raise ValueError(f"Existing error code changed: {old}")
registry = json.loads((root / "docs/security/instruction-registry-20261023.json").read_text())
if {item["name"] for item in idl["instructions"]} != {item["name"] for item in registry}:
    raise ValueError("IDL and source instruction registry differ")
if previous.get("metadata", {}).get("description") and not idl["metadata"].get("description"):
    idl["metadata"]["description"] = previous["metadata"]["description"]
destination.write_text(json.dumps(idl, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"instructions": len(idl["instructions"]), "types": len(idl["types"]),
                  "old_game_profile_layouts_preserved": True, "old_instruction_args_discriminators_preserved": True,
                  "preexisting_stale_idl_args_corrected_against_HEAD": preexisting_stale_args,
                  "safety_comment_lint": "SKIPPED_FOR_GENERATION" if args.lint_skipped else "ENABLED_DURING_GENERATION"}))
