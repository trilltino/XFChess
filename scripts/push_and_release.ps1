# XFChess push + release script
# Usage: .\scripts\push_and_release.ps1                # auto-bump patch, push branch + tag to origin
#        .\scripts\push_and_release.ps1 -Version v0.5.0 # cut a specific version
#        .\scripts\push_and_release.ps1 -DryRun         # compute the version and show what would happen, push nothing
#
# Pushes the current branch and a release tag to `origin` (trilltino/XFChess,
# the one public repo — there is no private split anymore, everything
# including backend/ and ops/ lives here). Pushing the tag triggers
# .github/workflows/release.yml directly, which builds and publishes the
# Windows/macOS/Linux installers plus the Chrome OS (Crostini) tarball to
# this repo's GitHub Releases page.
#
# This only does the push + tag. For push + wait-for-build + deploy to the
# Hetzner VPS in one command, use .\scripts\deploy_release.ps1 instead.

param(
    [string]$Version,
    [switch]$DryRun,
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$ROOT = Split-Path $PSScriptRoot -Parent
Set-Location $ROOT

$REMOTE = "origin"

# -- Determine branch --
$branch = git rev-parse --abbrev-ref HEAD
if ($LASTEXITCODE -ne 0 -or -not $branch) {
    throw "Not in a git repository, or HEAD is detached."
}

# -- Determine version --
if (-not $Version) {
    Write-Host "No -Version given, fetching tags to auto-bump patch..." -ForegroundColor Cyan
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

# -- Summary --
Write-Host ""
Write-Host "Branch:  $branch" -ForegroundColor Cyan
Write-Host "Version: $Version" -ForegroundColor Cyan
Write-Host "Remote:  $REMOTE" -ForegroundColor Cyan

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
    $confirm = Read-Host "Push branch '$branch' and tag '$Version' to $REMOTE? [y/N]"
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

# -- Push branch + tag --
Write-Host ""
Write-Host "Pushing branch to $REMOTE..." -ForegroundColor Cyan
git push $REMOTE $branch
if ($LASTEXITCODE -ne 0) { throw "git push $REMOTE $branch failed" }

Write-Host "Pushing tag $Version to $REMOTE..." -ForegroundColor Cyan
git push $REMOTE $Version
if ($LASTEXITCODE -ne 0) { throw "git push $REMOTE $Version failed" }

Write-Host ""
Write-Host "Done. $Version pushed to $REMOTE." -ForegroundColor Green
Write-Host "This tag triggers release.yml on trilltino/XFChess directly (gates on verify-backend, then builds windows/linux/macos/chromeos and publishes to Releases)." -ForegroundColor Green
Write-Host "Watch it: gh run list --repo trilltino/XFChess --workflow=release.yml --limit 5" -ForegroundColor Green
