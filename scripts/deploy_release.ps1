# XFChess combined push + release + deploy script
# Usage: .\scripts\deploy_release.ps1                    # auto-bump patch, do everything
#        .\scripts\deploy_release.ps1 -Version v0.5.0     # cut a specific version
#        .\scripts\deploy_release.ps1 -DryRun             # compute version + plan, push/deploy nothing
#        .\scripts\deploy_release.ps1 -SkipBuild          # pass-through: skip deploy.ps1's frontend/backend rebuild
#        .\scripts\deploy_release.ps1 -Domain ""          # pass-through: force deploy.ps1's self-signed cert (default -Domain is xfchess.com)
#
# Does four things in order:
#   1. Pushes the current branch + a version tag to `private` (full source of truth).
#   2. Syncs the public-safe subset of private's tree (everything except
#      backend/, ops/, the anti-cheat crate, deploy.yml, and this pair of
#      scripts) onto origin/main as one new commit, and pushes it. Without
#      this, origin/main never moves and installer builds get compiled from
#      a permanently stale tree.
#   3. Tags origin's newly-synced tip with the same version and pushes that
#      tag, triggering release.yml to build and publish public
#      Windows/macOS/Linux installers.
#   4. Waits for that release.yml run to finish, then runs ops\scripts\deploy.ps1
#      to build and ship the backend + web frontend to the live Hetzner VPS.
#
# The installer build (step 3) and the VPS deploy (step 4) are unrelated
# artifacts -- the VPS deploy builds backend/frontend from source directly and
# doesn't consume anything from the installer build. Waiting is a deliberate
# choice (not a dependency) so one command reports a single pass/fail for a
# release cycle. This step alone can take 15-30+ minutes.

param(
    [string]$Version,
    [string]$Server = "178.104.55.19",
    [string]$Domain = "xfchess.com",   # pass -Domain "" explicitly to force deploy.ps1's self-signed path
    [switch]$SkipBuild,
    [switch]$Force,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$ROOT = Split-Path $PSScriptRoot -Parent
Set-Location $ROOT

$PUBLIC_REPO = "trilltino/XFChess"

# Paths that never leave the private repo. Relative to repo root, forward
# slashes (used both as PowerShell paths after -replace and as git pathspecs).
$PRIVATE_ONLY_PATHS = @(
    "backend",
    "ops",
    "crates/shared/xfchess-anticheat",
    ".github/workflows/deploy.yml",
    "scripts/push_and_release.ps1",
    "scripts/deploy_release.ps1",
    "keys"
)

# Cargo.toml lines that reference the excluded paths above (workspace member
# entries and workspace.dependencies path entries aren't picked up by just
# deleting directories -- an explicit "backend" member with no backend/ dir
# breaks `cargo` immediately at manifest-load, before Cargo.lock is even
# read). Regexes, matched per-line against the synced Cargo.toml.
$CARGO_TOML_STRIP_PATTERNS = @(
    '^\s*"backend",?\s*$',
    '^xfchess-anticheat\s*='
)

# -- Preflight: repo, branch, clean tree --
$branch = git rev-parse --abbrev-ref HEAD
if ($LASTEXITCODE -ne 0 -or -not $branch) {
    throw "Not in a git repository, or HEAD is detached."
}

$dirty = git status --porcelain
if ($dirty) {
    Write-Host "ABORT: working tree has uncommitted changes. Commit or stash first:" -ForegroundColor Red
    Write-Host $dirty
    exit 1
}

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "gh CLI not found on PATH -- required to watch the release.yml run."
}

# Rebuilds origin/main's tree to equal $SourceRef's tree minus $ExcludePaths,
# as one new commit on top of whatever origin/main currently is. A full-tree
# replace rather than a cherry-pick/rebase: origin and private are meant to
# diverge permanently on the excluded paths, so replaying private's commit
# history onto origin would either conflict or (worse) briefly reintroduce
# private-only content into intermediate commits. Runs in a scratch worktree
# so it never touches the caller's checked-out branch.
function Sync-OriginPublicSafe {
    param(
        [Parameter(Mandatory)][string]$RepoRoot,
        [Parameter(Mandatory)][string[]]$ExcludePaths,
        [Parameter(Mandatory)][string]$SourceRef,
        [string[]]$CargoTomlStripPatterns = @()
    )

    # Resolve to a concrete SHA in $RepoRoot's context BEFORE entering the
    # worktree below. "HEAD" (or any other symbolic ref) is worktree-relative
    # -- read from inside the scratch worktree, "HEAD" would mean *that*
    # worktree's own (detached, origin-tip) HEAD, not $RepoRoot's, silently
    # turning the sync into a no-op.
    $sourceSha = git -C $RepoRoot rev-parse $SourceRef
    if ($LASTEXITCODE -ne 0) { throw "could not resolve SourceRef '$SourceRef' in $RepoRoot" }

    $tmpDir = Join-Path $env:TEMP "xfchess-origin-sync"

    if (Test-Path $tmpDir) {
        git -C $RepoRoot worktree remove --force $tmpDir
        if (Test-Path $tmpDir) { Remove-Item -Recurse -Force $tmpDir }
    }
    git -C $RepoRoot worktree prune

    git -C $RepoRoot worktree add --detach $tmpDir origin/main
    if ($LASTEXITCODE -ne 0) { throw "git worktree add failed" }

    git -C $tmpDir read-tree $sourceSha
    if ($LASTEXITCODE -ne 0) { throw "git read-tree failed" }
    git -C $tmpDir checkout-index -a -f
    if ($LASTEXITCODE -ne 0) { throw "git checkout-index failed" }
    git -C $tmpDir clean -fd -q

    git -C $tmpDir rm -r --cached --ignore-unmatch -q -- $ExcludePaths
    foreach ($p in $ExcludePaths) {
        $full = Join-Path $tmpDir ($p -replace '/', '\')
        if (Test-Path $full) { Remove-Item -Recurse -Force $full }
    }

    if ($CargoTomlStripPatterns.Count -gt 0) {
        $cargoTomlPath = Join-Path $tmpDir "Cargo.toml"
        if (Test-Path $cargoTomlPath) {
            $lines = Get-Content $cargoTomlPath
            $filtered = $lines | Where-Object {
                $line = $_
                -not ($CargoTomlStripPatterns | Where-Object { $line -match $_ })
            }
            if ($filtered.Count -ne $lines.Count) {
                # BOM-less UTF-8: Set-Content/Out-File -Encoding utf8 add a BOM in
                # PS5.1 (see ops/scripts/deploy.ps1's own note on this) -- untested
                # whether cargo tolerates one in Cargo.toml, not worth risking.
                $text = ($filtered -join "`n") + "`n"
                [System.IO.File]::WriteAllText($cargoTomlPath, $text, (New-Object System.Text.UTF8Encoding($false)))
                Write-Host "  Stripped $($lines.Count - $filtered.Count) private-only line(s) from Cargo.toml" -ForegroundColor DarkGray
            }
        }
    }

    git -C $tmpDir add -A
    $changes = git -C $tmpDir status --porcelain
    if (-not $changes) {
        Write-Host "No public-safe changes to sync to origin/main." -ForegroundColor DarkGray
        git -C $RepoRoot worktree remove --force $tmpDir
        return $false
    }

    $srcShort = git -C $RepoRoot rev-parse --short $sourceSha
    git -C $tmpDir commit -q -m "sync: apply public-safe changes from private@$srcShort"
    if ($LASTEXITCODE -ne 0) { throw "commit in sync worktree failed" }
    git -C $tmpDir push origin HEAD:main
    if ($LASTEXITCODE -ne 0) { throw "push to origin main failed" }

    git -C $RepoRoot worktree remove --force $tmpDir
    Write-Host "origin/main updated with public-safe changes from private@$srcShort" -ForegroundColor Green
    return $true
}

# -- Determine version (max across private + origin tags, then bump patch) --
if (-not $Version) {
    Write-Host "No -Version given, fetching tags from private + origin to auto-bump patch..." -ForegroundColor Cyan
    git fetch private --tags --quiet
    git fetch origin --tags --quiet

    $allTags = git tag --list "v*"
    $versions = $allTags | Where-Object { $_ -match '^v(\d+)\.(\d+)\.(\d+)$' } | ForEach-Object {
        [PSCustomObject]@{
            Tag   = $_
            Major = [int]$Matches[1]
            Minor = [int]$Matches[2]
            Patch = [int]$Matches[3]
        }
    }

    if ($versions.Count -eq 0) {
        $Version = "v0.1.0"
        Write-Host "No existing vX.Y.Z tags found - defaulting to $Version" -ForegroundColor Yellow
    } else {
        $latest = $versions | Sort-Object Major, Minor, Patch | Select-Object -Last 1
        $Version = "v$($latest.Major).$($latest.Minor).$($latest.Patch + 1)"
        Write-Host "Latest tag: $($latest.Tag) -> bumping to $Version" -ForegroundColor Green
    }
} elseif ($Version -notmatch '^v\d+\.\d+\.\d+$') {
    throw "Version must look like vX.Y.Z (got '$Version')"
}

git fetch origin --quiet
$originTipShort = git rev-parse --short origin/main

Write-Host ""
Write-Host "Branch:        $branch" -ForegroundColor Cyan
Write-Host "Version:       $Version" -ForegroundColor Cyan
Write-Host "private push:  branch + tag, from local HEAD" -ForegroundColor Cyan
Write-Host "origin sync:   public-safe subset of HEAD -> origin/main (currently $originTipShort)" -ForegroundColor Cyan
Write-Host "origin tag:    $Version -> origin/main's new tip after sync" -ForegroundColor Cyan
Write-Host "Then:          wait for release.yml on $PUBLIC_REPO, then deploy to $Server" -ForegroundColor Cyan

$excludePathspecs = $PRIVATE_ONLY_PATHS | ForEach-Object { ":!$_" }
Write-Host ""
Write-Host "Public-safe diff preview (origin/main vs HEAD, excluding private-only paths):" -ForegroundColor Cyan
git diff --stat origin/main HEAD -- . $excludePathspecs

if ($DryRun) {
    Write-Host ""
    Write-Host "-DryRun set - computed plan only, nothing pushed, nothing deployed." -ForegroundColor Yellow
    exit 0
}

if (-not $Force) {
    $confirm = Read-Host "Proceed with push + origin sync + public release + Hetzner deploy? [y/N]"
    if ($confirm -notmatch '^[Yy]') {
        Write-Host "Aborted." -ForegroundColor Red
        exit 1
    }
}

# -- Step 1: push branch + tag to private --
Write-Host ""
Write-Host "=== Step 1: pushing to private ===" -ForegroundColor Magenta
git push private $branch
if ($LASTEXITCODE -ne 0) { throw "git push private $branch failed" }

$privateTagExists = [bool](git tag --list $Version)
if (-not $privateTagExists) {
    git tag -a $Version -m "Release $Version"
    if ($LASTEXITCODE -ne 0) { throw "git tag failed" }
}
git push private $Version
if ($LASTEXITCODE -ne 0) { throw "git push private $Version failed" }

# -- Step 2: sync public-safe changes onto origin/main --
Write-Host ""
Write-Host "=== Step 2: syncing public-safe changes to origin/main ===" -ForegroundColor Magenta
Sync-OriginPublicSafe -RepoRoot $ROOT -ExcludePaths $PRIVATE_ONLY_PATHS -SourceRef "HEAD" -CargoTomlStripPatterns $CARGO_TOML_STRIP_PATTERNS | Out-Null

# -- Step 3: tag origin's (now-synced) tip and push, triggering the public release build --
Write-Host ""
Write-Host "=== Step 3: tagging origin's tip and triggering release.yml on $PUBLIC_REPO ===" -ForegroundColor Magenta
git fetch origin --quiet
$originTipShort = git rev-parse --short origin/main
git push origin "origin/main:refs/tags/$Version"
if ($LASTEXITCODE -ne 0) { throw "git push origin origin/main:refs/tags/$Version failed" }
Write-Host "Pushed $Version -> $originTipShort on origin." -ForegroundColor Green

# -- Step 4: find and wait for the triggered run --
Write-Host ""
Write-Host "=== Step 4: waiting for release.yml ($Version) on $PUBLIC_REPO ===" -ForegroundColor Magenta
$runId = $null
$findElapsed = 0
while (-not $runId -and $findElapsed -lt 120) {
    Start-Sleep -Seconds 5
    $findElapsed += 5
    $runsJson = gh run list --repo $PUBLIC_REPO --workflow=release.yml --json databaseId,headBranch,status,createdAt --limit 10
    if ($LASTEXITCODE -ne 0) { throw "gh run list failed" }
    $runs = $runsJson | ConvertFrom-Json
    $match = $runs | Where-Object { $_.headBranch -eq $Version } | Select-Object -First 1
    if ($match) { $runId = $match.databaseId }
}
if (-not $runId) {
    throw "Could not find a release.yml run for tag $Version after ${findElapsed}s. Check manually: gh run list --repo $PUBLIC_REPO --workflow=release.yml"
}
Write-Host "Found run $runId. Polling..." -ForegroundColor Cyan

# 90 min, not the 40 min originally guessed: a real run observed a ~15 min
# queue wait plus 25+ min of active Bevy/Tauri/Solana release compilation on
# macOS/Windows alone, with Linux starting later still. Cold-cache CI runners
# building this workspace are just slow -- this isn't a hang.
$maxWaitSeconds = 5400
$pollInterval = 20
$waitElapsed = 0
$run = $null
do {
    Start-Sleep -Seconds $pollInterval
    $waitElapsed += $pollInterval
    $runJson = gh run view $runId --repo $PUBLIC_REPO --json status,conclusion
    if ($LASTEXITCODE -ne 0) { throw "gh run view failed" }
    $run = $runJson | ConvertFrom-Json
    Write-Host "  [$waitElapsed s] status=$($run.status)" -ForegroundColor DarkGray
} while ($run.status -ne "completed" -and $waitElapsed -lt $maxWaitSeconds)

if ($run.status -ne "completed") {
    throw "Timed out after ${waitElapsed}s waiting for run $runId. Check: gh run view $runId --repo $PUBLIC_REPO"
}
if ($run.conclusion -ne "success") {
    throw "release.yml run $runId finished with conclusion '$($run.conclusion)'. Not deploying. Logs: gh run view $runId --repo $PUBLIC_REPO --log-failed"
}
Write-Host "release.yml succeeded ($Version, $waitElapsed s)." -ForegroundColor Green

# -- Step 5: deploy to Hetzner --
Write-Host ""
Write-Host "=== Step 5: deploying to Hetzner ($Server) ===" -ForegroundColor Magenta
$deployArgs = @("-Server", $Server, "-Domain", $Domain)
if ($SkipBuild) { $deployArgs += "-SkipBuild" }
& "$ROOT\ops\scripts\deploy.ps1" @deployArgs
if ($LASTEXITCODE -ne 0) { throw "deploy.ps1 failed" }

Write-Host ""
Write-Host "=== deploy_release complete: $Version pushed, origin synced, released publicly, deployed to $Server ===" -ForegroundColor Green
