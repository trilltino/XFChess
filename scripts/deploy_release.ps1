# XFChess combined push + release + deploy script
# Usage: .\scripts\deploy_release.ps1                    # auto-bump patch, do everything
#        .\scripts\deploy_release.ps1 -Version v0.5.0     # cut a specific version
#        .\scripts\deploy_release.ps1 -DryRun             # compute version + plan, push/deploy nothing
#        .\scripts\deploy_release.ps1 -SkipBuild          # pass-through: skip deploy.ps1's frontend/backend rebuild
#        .\scripts\deploy_release.ps1 -Domain ""          # pass-through: force deploy.ps1's self-signed cert (default -Domain is xfchess.com)
#
# Does three things in order (there is no private/public repo split anymore —
# `origin` (trilltino/XFChess) is the one source of truth, backend/ and ops/
# included):
#   1. Pushes the current branch + a version tag to `origin`. The tag push
#      triggers release.yml, which builds and publishes public
#      Windows/macOS/Linux installers plus the Chrome OS (Crostini) tarball.
#   2. Waits for that release.yml run to finish.
#   3. Runs ops\scripts\deploy.ps1 to build and ship the backend + web
#      frontend to the live Hetzner VPS.
#
# The installer build (step 1) and the VPS deploy (step 3) are unrelated
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
$REMOTE = "origin"

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

# -- Determine version (max across origin tags, then bump patch) --
if (-not $Version) {
    Write-Host "No -Version given, fetching tags from $REMOTE to auto-bump patch..." -ForegroundColor Cyan
    git fetch $REMOTE --tags --quiet

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

git fetch $REMOTE --quiet
$remoteTipShort = git rev-parse --short "$REMOTE/main"

Write-Host ""
Write-Host "Branch:   $branch" -ForegroundColor Cyan
Write-Host "Version:  $Version" -ForegroundColor Cyan
Write-Host "Push:     branch + tag $Version -> $REMOTE (currently $remoteTipShort)" -ForegroundColor Cyan
Write-Host "Then:     wait for release.yml on $PUBLIC_REPO, then deploy to $Server" -ForegroundColor Cyan

Write-Host ""
Write-Host "Diff preview ($REMOTE/main vs HEAD):" -ForegroundColor Cyan
git diff --stat "$REMOTE/main" HEAD

if ($DryRun) {
    Write-Host ""
    Write-Host "-DryRun set - computed plan only, nothing pushed, nothing deployed." -ForegroundColor Yellow
    exit 0
}

if (-not $Force) {
    $confirm = Read-Host "Proceed with push + public release + Hetzner deploy? [y/N]"
    if ($confirm -notmatch '^[Yy]') {
        Write-Host "Aborted." -ForegroundColor Red
        exit 1
    }
}

# -- Step 1: push branch + tag to origin, triggering the public release build --
Write-Host ""
Write-Host "=== Step 1: pushing to $REMOTE and triggering release.yml on $PUBLIC_REPO ===" -ForegroundColor Magenta
git push $REMOTE $branch
if ($LASTEXITCODE -ne 0) { throw "git push $REMOTE $branch failed" }

$tagExists = [bool](git tag --list $Version)
if (-not $tagExists) {
    git tag -a $Version -m "Release $Version"
    if ($LASTEXITCODE -ne 0) { throw "git tag failed" }
}
git push $REMOTE $Version
if ($LASTEXITCODE -ne 0) { throw "git push $REMOTE $Version failed" }
Write-Host "Pushed $branch + $Version to $REMOTE." -ForegroundColor Green

# -- Step 2: find and wait for the triggered run --
Write-Host ""
Write-Host "=== Step 2: waiting for release.yml ($Version) on $PUBLIC_REPO ===" -ForegroundColor Magenta
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

# A successful workflow can still produce an incomplete GitHub Release if an
# asset-attachment job was skipped or failed independently. Verify the Chrome
# OS asset before moving on to the unrelated VPS deployment.
$releaseAssets = gh release view $Version --repo $PUBLIC_REPO --json assets --jq '.assets[].name'
if ($LASTEXITCODE -ne 0) { throw "Could not inspect GitHub Release $Version for expected assets." }
$chromeOsAsset = "XFChess-chromeos-x86_64-$($Version.TrimStart('v')).tar.gz"
if ($releaseAssets -notcontains $chromeOsAsset) {
    throw "GitHub Release $Version is missing the Chrome OS asset '$chromeOsAsset'. Not deploying."
}
Write-Host "Chrome OS release asset verified: $chromeOsAsset" -ForegroundColor Green

# -- Step 3: deploy to Hetzner --
Write-Host ""
Write-Host "=== Step 3: deploying to Hetzner ($Server) ===" -ForegroundColor Magenta
$deployArgs = @("-Server", $Server, "-Domain", $Domain)
if ($SkipBuild) { $deployArgs += "-SkipBuild" }
& "$ROOT\ops\scripts\deploy.ps1" @deployArgs
if ($LASTEXITCODE -ne 0) { throw "deploy.ps1 failed" }

Write-Host ""
Write-Host "=== deploy_release complete: $Version pushed, released publicly, deployed to $Server ===" -ForegroundColor Green
