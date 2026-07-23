# Git & Deploy Workflow — XFChess

Source-control and release discipline (Production Reality Checklist §4). Part of the
[Production Reality Plan](PRODUCTION_REALITY_PLAN.md) WS-G.

## Branching
- **`main`** — always deployable; production is built from here.
- **`develop`** (optional) — integration; CI runs on it.
- **`feat/*`, `fix/*`, `chore/*`** — short-lived branches off `main`; open a PR; delete after merge.
- **Avoid long-lived branches** — rebase/merge frequently to prevent drift.

## Pull requests (required — no direct pushes to `main`)

> **Status: APPLIED** via the GitHub API. Current `main` protection: PRs required,
> required status checks = `Check`, `Test Suite`, `Web (build + audit)`; conversation
> resolution + linear history on; force-push/deletion off. **Solo-safe deviations from
> the list below:** required approvals = **0** and admins **not** hard-enforced — because
> on a solo repo GitHub won't let you approve your own PR, so "require 1 review" +
> "include administrators" would lock you out of merging. Tighten both once you add a
> collaborator. Revert all protection: `gh api -X DELETE repos/trilltino/XFChess/branches/main/protection`.

The full policy to grow into (in **GitHub → Settings → Branches**):
- ✅ Require a pull request before merging
- ✅ Require status checks to pass: `Check`, `Test Suite`, `Web (build + audit)`,
  `Chess Engine Tests`, (and `cargo-audit`/`Clippy` once flipped to gating)
- ✅ Require branches up to date before merging
- ✅ Require conversation resolution
- ✅ (recommended) Require signed commits
- ✅ Include administrators
- ✅ Restrict who can push to `main`

### Dedicated reviewers (CODEOWNERS)
Create `.github/CODEOWNERS` so sensitive areas require review:
```
/backend/migrations/        @trilltino
/programs/                  @trilltino
/ops/                    @trilltino
/backend/src/signing/       @trilltino
```

## Commits
- Conventional style: `type: description` (`feat`, `fix`, `docs`, `refactor`, `test`, `chore`).
- Sign commits if branch protection requires it (`git config commit.gpgsign true`).
- AI-assisted PRs: include the prompt + tool name and manual-test proof (per CLAUDE.md).

## Hotfix & cherry-pick
For an urgent production fix:
1. Branch `fix/<x>` off `main`, make the minimal change, PR + fast review, merge to `main`.
2. Deploy from `main` (see below).
3. If you maintain a release branch, **cherry-pick** the merge commit back:
   ```bash
   git checkout release/x.y
   git cherry-pick -x <commit>   # -x records the original SHA in the message
   ```
   Resolve conflicts, run tests, PR the backport.

## Deploy → commit traceability
- The backend bakes the git SHA at build time (`build.rs` → `GIT_SHA`) and serves it at
  `GET /health` (`git_sha`). After a deploy, `curl https://$SERVER/health` must show the
  SHA you shipped — that's how a production bug is traced to its commit.
- `ops/scripts/deploy.ps1` **aborts on a dirty tree** (WS-G) so prod always maps to a
  committed, pushed SHA.

## Rollback
- `ops/scripts/rollback.ps1` restores the previous binary. Know your **time-to-rollback**
  (measure it once) and record it. See [runbooks/backend-down.md](runbooks/backend-down.md).
