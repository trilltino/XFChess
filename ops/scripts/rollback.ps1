param(
    [string]$Server = '178.104.55.19',
    [ValidateSet('root')][string]$User = 'root',
    [string]$IdentityFile = "$env:USERPROFILE\.ssh\xfchess_vps",
    [string]$Release = '',
    [switch]$RestoreDb
)
. "$PSScriptRoot\ssh.ps1"
if ($RestoreDb) { throw 'Database recovery is separate: stop the backend and use ops/backup/restore-db.sh with an explicit snapshot.' }
if ($Release -and $Release -notmatch '^[a-zA-Z0-9_-]+$') { throw 'Invalid release identifier.' }
$connection = New-OpsConnection $Server $User $IdentityFile
$incoming = '/opt/xfchess/incoming/rollback-' + [guid]::NewGuid().ToString('N')
Invoke-OpsRemote $connection "install -d -m 700 $incoming"
foreach ($file in @('release.sh', 'rollback.sh')) { Send-OpsFile $connection "$PSScriptRoot\$file" "$incoming/$file" }
$target = if ($Release) { $Release } else { 'previous' }
Invoke-OpsRemote $connection "bash $incoming/rollback.sh $target"
