#!/usr/bin/env pwsh
# Load test for Triton One RPC integration.
# Simulates the burst pattern of 100 tournaments/day without real players.
#
# Usage:
#   .\scripts\test_triton_load.ps1 -BaseUrl http://localhost:3000 -AdminKey your_admin_key
#   .\scripts\test_triton_load.ps1 -BaseUrl http://localhost:3000 -AdminKey xf_admin_... -Tournaments 10 -Players 16

param(
    [string]$BaseUrl     = "http://localhost:3000",
    [string]$AdminKey    = $env:ADMIN_API_KEY,
    [int]   $Tournaments = 3,    # How many tournaments to spin up
    [int]   $Players     = 16,   # Bot count per tournament (8/16/32/64)
    [int]   $Concurrency = 8     # Parallel requests per burst
)

if (-not $AdminKey) {
    Write-Error "Set -AdminKey or export ADMIN_API_KEY"
    exit 1
}

$headers = @{ "X-Admin-Key" = $AdminKey; "Content-Type" = "application/json" }

function Invoke-Api($method, $path, $body = $null) {
    $uri = "$BaseUrl$path"
    $params = @{ Method = $method; Uri = $uri; Headers = $headers; ErrorAction = "Stop" }
    if ($body) { $params.Body = ($body | ConvertTo-Json -Compress) }
    try {
        $r = Invoke-RestMethod @params
        return $r
    } catch {
        Write-Warning "  FAIL $method $path — $($_.Exception.Message)"
        return $null
    }
}

function Measure-Burst($label, $jobs) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $results = $jobs | ForEach-Object -Parallel {
        $fn = $_
        & $fn
    } -ThrottleLimit $using:Concurrency
    $sw.Stop()
    $ok  = ($results | Where-Object { $_ -ne $null }).Count
    $fail = $results.Count - $ok
    Write-Host ("  {0,-40} {1,6}ms   ok={2} fail={3}" -f $label, $sw.ElapsedMilliseconds, $ok, $fail)
    return $sw.ElapsedMilliseconds
}

Write-Host ""
Write-Host "=== XFChess Triton One Load Test ===" -ForegroundColor Cyan
Write-Host "Target : $BaseUrl"
Write-Host "Tournaments : $Tournaments x $Players players"
Write-Host ""

# ── 1. Health check ───────────────────────────────────────────────────────────
Write-Host "[ 1 ] Health check" -ForegroundColor Yellow
$h = Invoke-Api GET "/health"
if (-not $h) { Write-Error "Backend not reachable"; exit 1 }
Write-Host "  OK — backend is up"

# ── 2. Baseline: single wallet balance (1 RPC call) ──────────────────────────
Write-Host ""
Write-Host "[ 2 ] Baseline RPC latency (single feepayer-balance)" -ForegroundColor Yellow
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Invoke-Api GET "/admin/feepayer-balance" | Out-Null
$sw.Stop()
Write-Host ("  Single get_balance: {0}ms" -f $sw.ElapsedMilliseconds)

# ── 3. Burst: wallet-balances (4 sequential RPC calls) ───────────────────────
Write-Host ""
Write-Host "[ 3 ] Burst: $Concurrency concurrent wallet-balance requests" -ForegroundColor Yellow
$walletJobs = 1..$Concurrency | ForEach-Object {
    { Invoke-RestMethod -Method GET -Uri "$using:BaseUrl/admin/wallet-balances" -Headers $using:headers -ErrorAction SilentlyContinue }
}
Measure-Burst "wallet-balances x$Concurrency" $walletJobs | Out-Null

# ── 4. Create tournaments ─────────────────────────────────────────────────────
Write-Host ""
Write-Host "[ 4 ] Creating $Tournaments tournaments ($Players max players each)" -ForegroundColor Yellow
$tournamentIds = @()
$baseId = [int](Get-Date -UFormat %s) % 100000

for ($i = 0; $i -lt $Tournaments; $i++) {
    $tid = $baseId + $i
    $body = @{
        tournament_id        = $tid
        name                 = "LoadTest_$tid"
        max_players          = $Players
        min_players          = 2
        entry_fee_lamports   = 0
        platform_fee_lamports = 0
        format               = "SingleElimination"
    }
    $r = Invoke-Api POST "/admin/tournament/create" $body
    if ($r -and $r.ok) {
        $tournamentIds += $tid
        Write-Host "  Created tournament $tid"
    } else {
        Write-Warning "  Failed to create tournament $tid"
    }
}

if ($tournamentIds.Count -eq 0) {
    Write-Warning "No tournaments created — check the create endpoint path"
    $tournamentIds = @($baseId)  # fall through to fill-bots anyway for manual testing
}

# ── 5. Fill with bots (simulates burst of players joining) ────────────────────
Write-Host ""
Write-Host "[ 5 ] Filling tournaments with $Players bots each (burst start)" -ForegroundColor Yellow
$fillJobs = $tournamentIds | ForEach-Object {
    $tid = $_
    {
        $b = @{ count = $using:Players; elo = 1200 } | ConvertTo-Json -Compress
        Invoke-RestMethod -Method POST `
            -Uri "$using:BaseUrl/admin/tournament/$tid/fill-bots" `
            -Headers $using:headers `
            -Body $b `
            -ErrorAction SilentlyContinue
    }
}
Measure-Burst "fill-bots burst ($($tournamentIds.Count) tournaments)" $fillJobs | Out-Null

# ── 6. Escrow balance burst (100 admin checks) ───────────────────────────────
Write-Host ""
Write-Host "[ 6 ] Burst: escrow-balance for $($tournamentIds.Count * 10) tournament IDs" -ForegroundColor Yellow
$escrowJobs = (1..($tournamentIds.Count * 10)) | ForEach-Object {
    $tid = $baseId + ($_ % [math]::Max(1, $tournamentIds.Count))
    { Invoke-RestMethod -Method GET -Uri "$using:BaseUrl/admin/tournament/$tid/escrow-balance" -Headers $using:headers -ErrorAction SilentlyContinue }
}
Measure-Burst "escrow-balance burst" $escrowJobs | Out-Null

# ── 7. Simulated admin dashboard poll (what a real operator would do) ─────────
Write-Host ""
Write-Host "[ 7 ] Simulated admin dashboard — 30s of polling every 2s" -ForegroundColor Yellow
$pollErrors = 0
for ($tick = 0; $tick -lt 15; $tick++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $jobs = @(
        { Invoke-RestMethod -Method GET -Uri "$using:BaseUrl/admin/wallet-balances" -Headers $using:headers -ErrorAction SilentlyContinue },
        { Invoke-RestMethod -Method GET -Uri "$using:BaseUrl/admin/feepayer-balance" -Headers $using:headers -ErrorAction SilentlyContinue },
        { Invoke-RestMethod -Method GET -Uri "$using:BaseUrl/admin/active-sessions"  -Headers $using:headers -ErrorAction SilentlyContinue },
        { Invoke-RestMethod -Method GET -Uri "$using:BaseUrl/admin/audit-log"        -Headers $using:headers -ErrorAction SilentlyContinue }
    )
    $r = $jobs | ForEach-Object -Parallel { & $_ } -ThrottleLimit 4
    $sw.Stop()
    $failed = ($r | Where-Object { $_ -eq $null }).Count
    $pollErrors += $failed
    Write-Host ("  tick {0,2}: {1,5}ms  rpc_errors={2}" -f ($tick+1), $sw.ElapsedMilliseconds, $failed)
    Start-Sleep -Milliseconds 2000
}

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
Write-Host "Tournaments started : $($tournamentIds.Count)"
Write-Host "Total poll errors   : $pollErrors / 60 requests"
if ($pollErrors -eq 0) {
    Write-Host "Result: PASS — no RPC failures under simulated load" -ForegroundColor Green
} else {
    Write-Host "Result: DEGRADED — $pollErrors RPC errors detected (check rate limits)" -ForegroundColor Red
}
Write-Host ""
