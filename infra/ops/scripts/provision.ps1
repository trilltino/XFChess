param(
    [string]$Server = '178.104.55.19',
    [string]$IdentityFile = "$env:USERPROFILE\.ssh\xfchess_vps"
)
. "$PSScriptRoot\ssh.ps1"
$connection = New-OpsConnection $Server 'root' $IdentityFile
$publicKey = "$IdentityFile.pub"
if (-not (Test-Path -LiteralPath $publicKey)) { throw "Create the matching public-key file first: $publicKey" }
$incoming = '/opt/xfchess/incoming/provision-' + [guid]::NewGuid().ToString('N')
Invoke-OpsRemote $connection "install -d -m 700 $incoming"
Send-OpsFile $connection $publicKey "$incoming/operator.pub"
Send-OpsFile $connection "$PSScriptRoot\provision.sh" "$incoming/provision.sh"
Invoke-OpsRemote $connection "bash $incoming/provision.sh $incoming/operator.pub"
