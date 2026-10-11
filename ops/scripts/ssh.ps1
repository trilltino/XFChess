Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function New-OpsConnection([string]$Server, [string]$User, [string]$IdentityFile) {
    if ($Server -notmatch '^[a-zA-Z0-9][a-zA-Z0-9.-]*$' -or $User -notmatch '^[a-z_][a-z0-9_-]*$') { throw 'Invalid SSH host or user.' }
    if (-not (Test-Path -LiteralPath $IdentityFile -PathType Leaf)) { throw "SSH identity not found: $IdentityFile" }
    return @{
        Target = "${User}@${Server}"
        Arguments = @('-i', $IdentityFile, '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10',
            '-o', 'StrictHostKeyChecking=yes', '-o', 'ServerAliveInterval=15', '-o', 'ServerAliveCountMax=3')
    }
}

function Invoke-OpsRemote($Connection, [string]$Command) {
    # A disconnected client cannot know whether a mutation completed; never retry automatically.
    $arguments = @($Connection.Arguments) + @($Connection.Target, $Command)
    & ssh @arguments
    if ($LASTEXITCODE -ne 0) { throw "SSH command failed (exit $LASTEXITCODE). Inspect server state before retrying." }
}

function Send-OpsFile($Connection, [string]$Path, [string]$Destination) {
    $arguments = @($Connection.Arguments) + @($Path, "$($Connection.Target):$Destination")
    & scp @arguments
    if ($LASTEXITCODE -ne 0) { throw "Upload failed: $Path" }
}
