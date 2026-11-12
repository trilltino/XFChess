# XFChess community documentation and GitHub workflow plan

## Goal

Adopt the contributor journey and evidence-oriented structure used by
`trilltino/raydium-transfer-hook`, while writing XFChess-specific material for
an AGPL Rust/Solana monorepo with optional wagering, wallet, settlement, KYC,
and personal-data risks. Do not copy the reference repository's MIT wording,
Raydium-specific guidance, commands, or security assumptions.

## Decisions to confirm before drafting

- Current audit and deployment status for each Solana program and environment.
- Whether GitHub private vulnerability reporting and Discussions are enabled.
- Monitored security, conduct, privacy, and support contacts.
- Supported local, devnet, and mainnet workflows and their verified commands.
- Minimum age, restricted jurisdictions, eligibility owner, and KYC terminology.
- Whether any bounty, compensation fund, refund policy, or response SLA exists.
- Whether contributions use ordinary AGPL-3.0-or-later terms only (recommended)
  or require a DCO/CLA.

Until confirmed, documentation must not promise an audit, bounty, compensation,
refund, legal availability, response deadline, or guaranteed settlement.

## Phase 1: Community entry point

### Rewrite `README.md`

Use this order:

1. Outcome-first description and honest maturity/status statement.
2. Plain-language overview of offline chess, online play, tournaments, and
   optional on-chain wagering.
3. Capability summary and current limitations.
4. A task-oriented quick-start table with `Goal`, `Command`, and `Needs`.
5. Current repository map covering `apps/`, `crates/`, `services/`, `programs/`,
   `infra/`, `docs/`, and `tools/`.
6. Starting points for non-code, client, website, backend, protocol, Solana,
   testing, accessibility, and design contributions.
7. Testing/evidence expectations by subsystem.
8. Contribution, conduct, private security, disclaimer, and licence links.
9. A short digital-asset/wager warning linked to `DISCLAIMER.md`.

Derive every command from current manifests and `justfile`; do not copy commands
from the reference repository. Verify syntax without launching the cancelled
full build.

### Reorganize `CONTRIBUTING.md`

Preserve its strong offline/deterministic test and AI-disclosure policies, then
add:

- newcomer contribution lanes and a concrete first action;
- small-fix versus issue/design-first guidance;
- subsystem prerequisites and targeted check/test commands;
- evidence levels: static, unit, integration, UI/manual, devnet;
- compatibility rules for APIs, protocols, migrations, IDLs, PDAs, account
  layouts, program IDs, and persisted client state;
- special review requirements for funds, wallets, identity, and privacy;
- a compact pull-request checklist and AGPL contribution terms.

Fix active stale links, including `docs/development/`, `docs/INSTALL.md`, and
`docs/design/`. Update other contributor-facing paths moved from root `src/`,
`tests/`, `backend/`, `tauri/`, `libs/`, and `xfchessdotcom/`. Preserve dated
audit evidence as historical material and add a path-mapping note instead of
silently rewriting its historical claims.

## Phase 2: Conduct, security, and risk policy

### Rename conduct documentation

Rename `CODE_OF_CONDUCT.md` to `CONDUCT.md` with history preserved, retitle it
`Community Conduct`, retain its substantive Contributor Covenant terms, and
update all links.

GitHub only auto-discovers the conventional `CODE_OF_CONDUCT.md` filename. The
requested rename therefore requires prominent explicit links from the README,
contributing guide, and issue chooser. If automatic Community Standards
recognition is required, make a separate maintainer decision about a minimal
`.github/CODE_OF_CONDUCT.md` compatibility pointer; do not add it silently.

### Add `DISCLAIMER.md`

Create one canonical, plain-language notice covering:

- no legal, financial, investment, tax, or wagering advice;
- asset volatility, fees, irreversible transactions, and key/wallet custody;
- smart-contract, client, backend, session signer, administrator, RPC, wallet,
  oracle/rate, upgrade, availability, and settlement risks;
- no guaranteed winnings, value, liquidity, availability, refund, recovery, or
  settlement time;
- age, identity, KYC, account, geographic, and local-law eligibility;
- devnet/test assets having no intended monetary value;
- the controlling AGPL warranty and liability terms.

Do not turn this into acceptance language or pretend it replaces counsel-drafted
Terms of Use or a Privacy Notice.

### Tighten `SECURITY.md`

- Update scope to the current client, website, Tauri bridge, backend, programs,
  infrastructure, and third-party boundaries.
- Keep vulnerability reports private and list the evidence reporters should
  include; explicitly prohibit publishing secrets, seed phrases, tokens, KYC
  data, or live-fund exploits.
- Add supported versions, coordinated disclosure, and severity guidance.
- Describe response times as targets, not guarantees.
- Remove the unverified `community fund available` compensation promise.
- Qualify pause/upgrade/refund/disclosure powers against actual operations.
- Have counsel review broad safe-harbor language before retaining it.

## Phase 3: Structured GitHub intake

Create:

- `.github/ISSUE_TEMPLATE/config.yml`
- `.github/ISSUE_TEMPLATE/bug.yml`
- `.github/ISSUE_TEMPLATE/idea.yml`
- `.github/ISSUE_TEMPLATE/task.yml`
- `.github/PULL_REQUEST_TEMPLATE.md`

Keep blank issues enabled initially for accessibility. Link the chooser to
contribution guidance and private vulnerability reporting; only link Discussions
or support channels that are enabled and monitored. Do not reference labels that
do not exist.

The bug form should collect affected subsystem, expected/actual behavior,
reproduction, OS/device/build, commit, local/devnet/mainnet environment, sanitized
logs or transaction evidence, and funds/privacy/security impact.

The idea form should welcome no-code proposals and collect the user problem,
desired outcome, pass/fail examples, affected surfaces, edge cases, trust and
authority assumptions, accessibility, privacy, wallet/wager, compatibility, and
legal impact.

The task form should link an agreed design, define scope/non-goals, owners,
acceptance evidence, compatibility, migration, security, and documentation work.

The PR template should remain short while requesting: what/why, linked issue,
scope, exact checks and evidence, tested platforms/networks, UI media, API/DB/
protocol compatibility, on-chain IDL/PDA/layout/program impacts, privacy/security/
funds impact, documentation, AI assistance, and reviewer focus.

## Phase 4: Public legal and risk surfaces

A repository disclaimer is not enough for a wagering product. After legal and UX
review:

- add Disclaimer, Privacy, Security, Conduct, and Licence links to the website
  footer and a public legal route;
- qualify absolute marketing claims such as `trustless` and `no middleman` in
  website copy, SEO, and structured data;
- add concise risk/eligibility information beside wager create/join actions;
- replace unverified `CARF/RCASP` and `zero-knowledge vault` claims in the client
  compliance flow with precise, evidenced wording;
- commission a proper Privacy Notice for collected KYC/identity data and hosted
  service Terms of Use. Treat these as release gates, not generic open-source
  templates.

## Phase 5: Governance and repository hygiene

- Expand `.github/CODEOWNERS` across apps, client crates, website, security,
  community, and legal-sensitive files.
- Add `GOVERNANCE.md` or `MAINTAINERS.md` only after maintainers define decision,
  review, and escalation responsibilities.
- Add `.github/SUPPORT.md` when a monitored support route exists.
- Audit internal Cargo manifests for consistent AGPL metadata; keep explicit
  third-party MIT/Apache licences intact. Correct the Tauri manifest's repository
  and licence mismatch.
- Sanitize `keys/KEYS.md` so the public guide contains no internal recovery host,
  operational access path, or stale secret-location details.

## Implementation order

1. Confirm policy facts, contacts, GitHub features, labels, and valid commands.
2. Draft `DISCLAIMER.md`; align `SECURITY.md` and remove unsupported promises.
3. Rename the conduct file and update all inbound links.
4. Rewrite README and contributing guidance against the current monorepo.
5. Add issue forms and the PR template.
6. Fix active stale documentation and licence metadata.
7. Align website/client wager and compliance wording after legal review.
8. Add privacy/terms and governance documents only with named owners and review.

## Acceptance criteria

- Repository search finds no unintended `CODE_OF_CONDUCT.md` or obsolete active
  paths.
- README commands resolve to real packages/scripts; no full build is required for
  this documentation pass.
- Markdown links resolve and issue-form YAML conforms to GitHub's schema.
- The GitHub issue chooser and PR template render correctly.
- Public and repository wording is consistent about audit status, dependencies,
  wagering, settlement, eligibility, privacy, and operational control.
- No document makes unsupported audit, bounty, compensation, refund, SLA,
  legality, compliance, privacy, or settlement claims.
- AGPL-3.0-or-later is used for XFChess-owned code; reference-repo MIT language is
  not imported.
- Security and legal owners record review of wagering, KYC/privacy, jurisdiction,
  safe-harbor, and marketing wording before mainnet promotion.
