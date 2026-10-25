"""Generate review inputs; syntactic extraction is not a completed trust audit."""
import hashlib
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parents[2]
source = root / "programs/xfchess-game/src"
out = Path(__file__).resolve().parent
lib = (source / "lib.rs").read_text(encoding="utf-8")
program = lib[lib.index("pub mod xfchess_game {"):]
instructions = []
for match in re.finditer(r"pub fn (\w+)(?:<[^>]+>)?\s*\(\s*\w+: Context<", program):
    name = match[1]
    position, depth = match.end(), 1
    end = position
    while depth:
        if program[end] == '<': depth += 1
        if program[end] == '>': depth -= 1
        end += 1
    context = program[position:end - 1]
    instructions.append({"name": name, "context": context,
        "discriminator": list(hashlib.sha256(f"global:{name}".encode()).digest()[:8])})
(out / "instruction-registry-20261023.json").write_text(json.dumps(instructions, indent=2) + "\n", encoding="utf-8")
rows = []
reviewed = json.loads((out / "unchecked-account-review-20261024.json").read_text()) if (out / "unchecked-account-review-20261024.json").exists() else {}


def struct_body(text, start):
    """Match nested constraint blocks as well as the enclosing Accounts struct."""
    depth, index = 1, start
    while depth:
        if text.startswith("//", index):
            newline = text.find("\n", index)
            index = len(text) if newline < 0 else newline
        elif text[index] == '"':
            index += 1
            while text[index] != '"':
                index += 2 if text[index] == "\\" else 1
        elif text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
        index += 1
    return text[start:index - 1]


for path in sorted(source.rglob("*.rs")):
    text = path.read_text(encoding="utf-8")
    for struct in re.finditer(r"pub struct (\w+)<'info>\s*\{", text):
        name, body = struct[1], struct_body(text, struct.end())
        if "Account" not in body and "Signer" not in body:
            continue
        start = 0
        for field in re.finditer(r"pub (\w+): ([^\n]+),", body):
            account, kind = field.groups()
            checks = body[start:field.start()].strip()
            start = field.end()
            line = text[:struct.start()].count("\n") + body[:field.start()].count("\n") + 2
            raw = re.sub(r"\s+", " ", checks).replace("|", "\\|")
            signer = "yes" if "Signer<" in kind or "signer" in checks else "no declared signer"
            mutable = "yes" if re.search(r"\b(mut|init|init_if_needed|close)\b", checks) else "no"
            owner = "XFChess (typed)" if "Account<'info," in kind or "AccountLoader<" in kind else "System" if "SystemAccount<" in kind else "program ID + executable" if "Program<" in kind else "SEE CONSTRAINTS/HANDLER"
            disposition = "NEEDS SEMANTIC REVIEW"
            if name + "." + account in reviewed:
                disposition = reviewed[name + "." + account]
            if name == "InitProfile" and account == "player_profile":
                disposition = "Canonical wallet PDA; manual sponsored initialization; ownership checked before overwrite."
            elif name == "InitializeAfterUndelegation":
                disposition = {"base_account": "Handler pins Game PDA; ownership changes during SDK restore, so cannot deserialize yet.",
                    "buffer": "Canonical delegation PDA; SDK requires its signature and delegation ownership.",
                    "payer": "SDK permits non-signing/non-System payer if no rent transfer is required.",
                    "system_program": "Typed System program; CPI target fixed."}.get(account, disposition)
            elif name == "RecoverStuckDelegation":
                disposition = "Legacy recovery rejects; no money/rent movement without verified identities."
            elif "V2" in name and account == "recovery_identity":
                disposition = "Canonical permanent record; fresh initialization or Anchor owner/discriminator check; handler enforces lifecycle."
            if path.relative_to(source).as_posix() in {"game_ix/record.rs", "game_ix/record_move.rs"}:
                disposition = "ORPHAN SOURCE: not declared by game_ix/mod.rs; compiled move instructions are under moves_ix."
            rows.append((name, account, kind.replace("|", "\\|"), signer, mutable, owner,
                         f"{path.relative_to(root).as_posix()}:{line}", raw, disposition))
header = """# Account trust review registry

Generated from current source. All declarative constraints and CHECK comments are
retained. Signer, mutability, owner and executable summaries are syntactic aids.
PDA seeds, payer, recipient, CPI target and handler checks must be read together;
rows marked NEEDS SEMANTIC REVIEW are not audit clearance. Uncompiled historical
move files are labeled ORPHAN SOURCE. Composite `legacy`
accounts inherit the referenced context's rows. Instruction registry is separate.

| Context | Account | Declared type | Signer | Mutable | Owner/executable | Source | Seeds/payer/recipient/constraints | Handler review |
|---|---|---|---|---|---|---|---|---|
"""
(out / "account-trust-matrix.md").write_text(header + "\n".join("| " + " | ".join(row) + " |" for row in rows) + "\n", encoding="utf-8")
print(json.dumps({"source_files": len(list(source.rglob("*.rs"))), "instructions": len(instructions), "accounts": len(rows)}))
