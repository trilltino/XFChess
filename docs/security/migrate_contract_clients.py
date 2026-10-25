"""Explicit contract-builder migration; preserves every existing account prefix."""
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parents[2]
versioned = {"create_game", "join_game", "global_create_game", "global_join_game",
             "session_create_game", "session_join_game", "create_tournament_game",
             "delegate_game", "recover_stuck_delegation"}
payouts = {"finalize_game", "withdraw_expired_wager", "cancel_game", "resolve_dispute", "claim_stale_dispute"}
paths = [root / "src/solana/program_interface/instructions.rs",
         root / "src/multiplayer/solana/global_session_manager.rs",
         root / "backend/src/signing/solana/instructions.rs"]
paths += list((root / "tools/cu_benchmark/src/chain/instructions").rglob("*.rs"))


def end_of(source, start, opening, closing):
    depth = 1
    i = start + 1
    while depth:
        if source.startswith("//", i):
            i = source.find("\n", i)
            if i < 0:
                raise ValueError("Unclosed body")
        elif source[i] == '"':
            i += 1
            while source[i] != '"':
                i += 2 if source[i] == "\\" else 1
        elif source[i] == opening:
            depth += 1
        elif source[i] == closing:
            depth -= 1
        i += 1
    return i


for path in paths:
    original = path.read_text(encoding="utf-8")
    edits = []
    for match in re.finditer(r"pub fn \w+\s*\(", original):
        start = original.find("{", match.end())
        end = end_of(original, start, "{", "}")
        signature = original[match.start():start]
        body = original[start:end]
        discriminator = re.search(r'anchor_discriminator\("([a-z_]+)"\)', body)
        if not discriminator or discriminator[1] not in versioned | payouts:
            continue
        operation = discriminator[1]
        if 'b"recovery_identity"' in body:
            continue
        program_ref = "program_id" if re.search(r"program_id:\s*&", signature) else "&program_id"
        meta = ('AccountMeta::new(Pubkey::find_program_address('
                '&[b"recovery_identity", &game_id.to_le_bytes()], ' + program_ref + ').0, false)')
        if re.search(r"let mut accounts =", body):
            position = body.rfind("    Instruction {")
            if position < 0:
                raise ValueError(f"No final Instruction in {path}: {operation}")
            body = body[:position] + f"    accounts.push({meta});\n" + body[position:]
        else:
            accounts = re.search(r"(?:accounts:|let accounts =)\s*vec!\[", body)
            if not accounts:
                raise ValueError(f"No explicit account vector in {path}: {operation}")
            bracket = accounts.end() - 1
            vector_end = end_of(body, bracket, "[", "]") - 1
            body = body[:vector_end] + f"            {meta},\n        " + body[vector_end:]
        if operation in versioned:
            body = body.replace(f'anchor_discriminator("{operation}")', f'anchor_discriminator("{operation}_v2")', 1)
        edits.append((start, end, body, operation))
    for start, end, body, operation in reversed(edits):
        original = original[:start] + body + original[end:]
    if edits:
        print(f'{path.relative_to(root)}: {", ".join(edit[3] for edit in edits)}')
        if "--write" in sys.argv:
            path.write_text(original, encoding="utf-8")
