param(
  [int]$Port = 7454,
  [string]$LogRoot = "$env:LOCALAPPDATA\xfchess\logs"
)

$logPath = Join-Path $LogRoot "wallet-bridge.log.$((Get-Date).ToString('yyyy-MM-dd'))"

Write-Host "XFChess wallet signing trace"
Write-Host "Bridge port: $Port"
Write-Host "Log file:    $logPath"
Write-Host "Browser:    open DevTools for the XFChess wallet popup"
Write-Host "Press Ctrl+C to stop."
Write-Host ""

if (-not (Test-Path $logPath)) {
  Write-Warning "Log file does not exist yet. Waiting for it: $logPath"
  while (-not (Test-Path $logPath)) {
    Start-Sleep -Milliseconds 500
  }
}

Get-Content -Path $logPath -Wait | Where-Object {
  $_ -match 'SIGN_|WalletBridge|WalletPopup|Lifecycle|request_id|sid='
}
