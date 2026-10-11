param(
    [string]$Server = '178.104.55.19',
    [ValidateSet('root')][string]$User = 'root',
    [string]$Domain = 'xfchess.com',
    [string]$IdentityFile = "$env:USERPROFILE\.ssh\xfchess_vps",
    [switch]$SkipBuild,
    [switch]$UpdateMonitoring
)
. "$PSScriptRoot\ssh.ps1"
if ($SkipBuild) { throw 'SkipBuild is no longer supported. Each release must build and verify its own commit.' }
if ($Domain -notmatch '^(?=.{1,253}$)[a-zA-Z0-9][a-zA-Z0-9.-]*[a-zA-Z0-9]$') { throw 'Set a valid TLS domain.' }
$connection = New-OpsConnection $Server $User $IdentityFile
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$workspace = Join-Path ([IO.Path]::GetTempPath()) ("xfchess-deploy-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $workspace | Out-Null
Push-Location $root
try {
    $dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Commit or stash changes before deploying; the release uses exactly HEAD.' }
    $sha = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $sha -notmatch '^[a-f0-9]{40}$') { throw 'Cannot identify the release commit.' }
    $release = "$sha-$([DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ'))-$([guid]::NewGuid().ToString('N').Substring(0,8))"
    & git archive --format=tar.gz "--output=$workspace\source.tar.gz" HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Source archive failed.' }
    Push-Location "$root\xfchessdotcom"
    $previousBackend = $env:VITE_BACKEND_URL
    $previousSigning = $env:SIGNING_SERVICE_URL
    try {
        $env:VITE_BACKEND_URL = "https://$Domain"
        $env:SIGNING_SERVICE_URL = "https://$Domain"
        & npm ci
        if ($LASTEXITCODE -ne 0) { throw 'Frontend dependency install failed.' }
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
        & tar -czf "$workspace\web.tar.gz" -C dist .
        if ($LASTEXITCODE -ne 0) { throw 'Frontend archive failed.' }
    } finally {
        $env:VITE_BACKEND_URL = $previousBackend
        $env:SIGNING_SERVICE_URL = $previousSigning
        Pop-Location
    }
    $incoming = "/opt/xfchess/incoming/$release"
    Invoke-OpsRemote $connection "test -f /opt/xfchess/.env && install -d -m 700 $incoming"
    foreach ($file in @('source.tar.gz', 'web.tar.gz')) { Send-OpsFile $connection "$workspace\$file" "$incoming/$file" }
    foreach ($file in @('release.sh', 'deploy.sh')) { Send-OpsFile $connection "$PSScriptRoot\$file" "$incoming/$file" }
    $monitor = if ($UpdateMonitoring) { '1' } else { '0' }
    Invoke-OpsRemote $connection "bash $incoming/deploy.sh $release $sha $Domain $monitor"
    Write-Host "Deployed $sha to https://$Domain. Release: $release" -ForegroundColor Green
} finally {
    Pop-Location
    foreach ($file in @('source.tar.gz', 'web.tar.gz')) {
        $path = Join-Path $workspace $file
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    }
    Remove-Item -LiteralPath $workspace
}
