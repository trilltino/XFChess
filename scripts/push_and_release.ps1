# XFChess push + release script
# Usage: .\scripts\push_and_release.ps1                # auto-bump patch, push branch + tag to private
#        .\scripts\push_and_release.ps1 -Version v0.5.0 # cut a specific version
#        .\scripts\push_and_release.ps1 -DryRun         # compute the version and show what would happen, push nothing
#
# Pushes the current branch and a release tag to `private` only.
#
# `origin` is intentionally excluded: since the open-core split
# (d0eb3bffe, "chore: split backend into private repo"), local HEAD tracks
# `private/main`, which still contains backend/, the anti-cheat crate, and
# other private-only paths. Pushing a *branch* built from that history to
# `origin` would just be rejected (non-fast-forward, origin has its own
# divergent split commit) — but pushing a *tag* would not be rejected the
# same way, and would upload the tagged commit's full tree (backend and
# all) to the public repo as reachable objects. Cutting a public installer
# release on `origin` needs a tag created from origin's own tree, not from
# private HEAD — that's a separate, not-yet-built flow. See docs/PUBLISHING.md.

param(
    [string]$Version,
    [switch]$DryRun,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$ROOT = Split-Path $PSScriptRoot -Parent
Set-Location $ROOT

$REMOTES = @("private")

# -- Determine branch --
$branch = git rev-parse --abbrev-ref HEAD
if ($LASTEXITCODE -ne 0 -or -not $branch) {
    throw "Not in a git repository, or HEAD is detached."
}

# -- Determine version --
if (-not $Version) {
    Write-Host "No -Version given, fetching tags to auto-bump patch..." -ForegroundColor Cyan
    foreach ($remote in $REMOTES) {
        git fetch $remote --tags --quiet
    }

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

# -- Summary --
Write-Host ""
Write-Host "Branch:  $branch" -ForegroundColor Cyan
Write-Host "Version: $Version" -ForegroundColor Cyan
Write-Host "Remotes: $($REMOTES -join ', ')" -ForegroundColor Cyan

$dirty = git status --porcelain
if ($dirty) {
    Write-Host "Note: working tree has uncommitted changes - they will NOT be included (only committed history is pushed)." -ForegroundColor Yellow
}

$tagExists = [bool](git tag --list $Version)
if ($tagExists) {
    Write-Host "Tag $Version already exists locally - will push as-is, not recreate it." -ForegroundColor Yellow
}

if ($DryRun) {
    Write-Host ""
    Write-Host "-DryRun set - computed version only, nothing pushed." -ForegroundColor Yellow
    exit 0
}

if (-not $Force) {
    $confirm = Read-Host "Push branch '$branch' and tag '$Version' to $($REMOTES -join ' + ')? [y/N]"
    if ($confirm -notmatch '^[Yy]') {
        Write-Host "Aborted." -ForegroundColor Red
        exit 1
    }
}

# -- Create tag --
if (-not $tagExists) {
    git tag -a $Version -m "Release $Version"
    if ($LASTEXITCODE -ne 0) { throw "git tag failed" }
}

# -- Push branch + tag to every remote --
foreach ($remote in $REMOTES) {
    Write-Host ""
    Write-Host "Pushing branch to $remote..." -ForegroundColor Cyan
    git push $remote $branch
    if ($LASTEXITCODE -ne 0) { throw "git push $remote $branch failed" }

    Write-Host "Pushing tag $Version to $remote..." -ForegroundColor Cyan
    git push $remote $Version
    if ($LASTEXITCODE -ne 0) { throw "git push $remote $Version failed" }
}

Write-Host ""
Write-Host "Done. $Version pushed to $($REMOTES -join ', ')." -ForegroundColor Green
Write-Host "If private has its own release.yml, this tag triggers it there (gates on verify-backend, then builds windows/linux/macos)." -ForegroundColor Green
Write-Host "Watch it: gh run --repo trilltino/xfchess-private list --workflow=release.yml --limit 5" -ForegroundColor Green
Write-Host ""
Write-Host "NOTE: this does NOT publish to the public trilltino/XFChess Releases page end users" -ForegroundColor Yellow
Write-Host "download from (docs/INSTALL.md). Public installer releases need a tag built from" -ForegroundColor Yellow
Write-Host "origin's own tree (no backend/), which this script does not do yet." -ForegroundColor Yellow
