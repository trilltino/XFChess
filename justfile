# XFChess dev task runner
# Install: cargo install just  (or winget install Casey.Just)
# Usage:   just dev            — full local stack
#          just build          — build all Rust binaries
#          just kill           — stop all running XFChess processes
#          just backend        — build + run backend only
#          just game           — build + run game only

set windows-shell := ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command"]

# ── Environment ───────────────────────────────────────────────────────────────

export BACKEND_URL             := "http://127.0.0.1:8090"
export SIGNING_SERVICE_URL     := "http://127.0.0.1:8090"
export SOLANA_RPC_URL          := "https://beta.helius-rpc.com/?api-key=5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5"
export HELIUS_API_KEY          := "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5"
export ER_RPC_URL              := "https://devnet.magicblock.app"
export MAGIC_BLOCK_RPC_URL     := "https://devnet.magicblock.app"
export PROGRAM_ID              := "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
export JWT_SECRET              := "137a895ebd9506dad79ba1f6c7d1119ad1446f7214710d93a0743f72deb5b5f3"
export IDENTITY_ENCRYPTION_KEY := "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
export IDENTITY_SALT           := "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
export FEE_PAYER_KEYS          := "61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi"
export VPS_AUTHORITY_KEY       := "61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi"
export KYC_AUTHORITY_KEY       := "61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi"
export HOST_TREASURY_PUBKEY    := "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C"
export RUST_LOG                := "info"

# Debug build dir (fast local iteration)
bin := "target/debug"

# ── Default ───────────────────────────────────────────────────────────────────

# List all available recipes
default:
    @just --list

# ── Cleanup ───────────────────────────────────────────────────────────────────

# Stop all running XFChess processes (PID file first, then port owner, then name)
kill:
    @$ErrorActionPreference = 'SilentlyContinue'; \
     Write-Host "[CLEANUP] Stopping XFChess processes..." -ForegroundColor Cyan; \
     $pidFile = "backend/.backend.pid"; \
     if (Test-Path $pidFile) { \
         $oldPid = Get-Content $pidFile; \
         Stop-Process -Id $oldPid -Force; \
         Remove-Item $pidFile; \
         Write-Host "  Killed backend PID $oldPid" \
     }; \
     netstat -aon 2>$null | Select-String " :5174 " | ForEach-Object { \
         $parts = ($_ -replace '\s+', ' ').Trim().Split(' '); \
         $ownerPid = $parts[-1]; \
         if ($ownerPid -match '^\d+$') { Stop-Process -Id $ownerPid -Force } \
     }; \
     netstat -aon 2>$null | Select-String " :8090 " | ForEach-Object { \
         $parts = ($_ -replace '\s+', ' ').Trim().Split(' '); \
         $ownerPid = $parts[-1]; \
         if ($ownerPid -match '^\d+$') { \
             Stop-Process -Id $ownerPid -Force; \
             Write-Host "  Killed port-8090 owner PID $ownerPid" \
         } \
     }; \
     Stop-Process -Name "signing-server" -Force; \
     Stop-Process -Name "xfchess" -Force; \
     Stop-Process -Name "xfchess-tauri" -Force; \
     Start-Sleep -Milliseconds 500; \
     Write-Host "[CLEANUP] Done" -ForegroundColor Green

# ── Build ─────────────────────────────────────────────────────────────────────

# Build backend signing-server (debug)
build-backend:
    cargo build -p backend --bin signing-server

# Build game client with Solana features (debug)
build-game:
    cargo build --bin xfchess --features solana

# Build Tauri host (debug)
build-tauri:
    cargo build -p xfchess-tauri

# Build all Rust binaries (debug)
build: build-backend build-game build-tauri

# Release build of everything
build-release:
    cargo build -p backend --bin signing-server --release
    cargo build --bin xfchess --features solana --release
    cargo build -p xfchess-tauri --release

# Build wallet UI (only if dist is missing or forced)
build-wallet-ui:
    @if (-not (Test-Path "tauri/wallet-ui/dist")) { \
        Write-Host "[BUILD] Building Wallet UI..." -ForegroundColor Cyan; \
        Set-Location tauri/wallet-ui; npm install; npm run build; Set-Location ../.. \
    } else { \
        Write-Host "[BUILD] Wallet UI dist exists, skipping (run 'just build-wallet-ui-force' to rebuild)" \
    }

# Force-rebuild wallet UI
build-wallet-ui-force:
    Set-Location tauri/wallet-ui && npm install && npm run build && Set-Location ../..

# Build web frontend
build-web:
    Set-Location web-solana && npm install && npm run build && Set-Location ..

# ── Run individual services ───────────────────────────────────────────────────

# Run backend only (builds first)
backend: build-backend
    @Write-Host "[BACKEND] Starting signing-server on :8090" -ForegroundColor Cyan
    Set-Location backend && ../{{bin}}/signing-server.exe

# Run game client only (builds first)
game: build-game
    @Write-Host "[GAME] Starting XFChess" -ForegroundColor Cyan
    ./{{bin}}/xfchess.exe

# Run web frontend dev server
web:
    Set-Location web-solana && npm run dev

# Run tournament admin dev server
admin:
    Set-Location tauri/tournament-admin && npm run dev -- --port 7455

# ── Full dev stack ────────────────────────────────────────────────────────────

# Build everything then launch full local stack (uses Windows Terminal tabs if available)
dev: kill build build-wallet-ui
    @Write-Host "" -ForegroundColor White
    @Write-Host "========================================" -ForegroundColor Cyan
    @Write-Host " XFChess Local Dev Stack" -ForegroundColor Cyan
    @Write-Host "========================================" -ForegroundColor Cyan
    @$hasWT = Get-Command wt -ErrorAction SilentlyContinue; \
     $bin = "{{bin}}"; \
     $root = (Get-Location).Path; \
     if ($hasWT) { \
         Write-Host "[LAUNCH] Using Windows Terminal tabs" -ForegroundColor Green; \
         wt -w 0 nt --title "Backend" -d "$root/backend" powershell -NoProfile -Command "$root/$bin/signing-server.exe"; \
         Start-Sleep -Seconds 2; \
         wt -w 0 nt --title "Wallet UI" -d "$root/tauri/wallet-ui" powershell -NoProfile -Command "npm run dev"; \
         Start-Sleep -Seconds 1; \
         wt -w 0 nt --title "Tauri" -d "$root" powershell -NoProfile -Command "$root/$bin/xfchess-tauri.exe"; \
         Start-Sleep -Seconds 1; \
         wt -w 0 nt --title "Game" -d "$root" powershell -NoProfile -Command "$root/$bin/xfchess.exe"; \
         wt -w 0 nt --title "Web Frontend" -d "$root/web-solana" powershell -NoProfile -Command "npm run dev"; \
         wt -w 0 nt --title "Tournament Admin" -d "$root/tauri/tournament-admin" powershell -NoProfile -Command "npm run dev -- --port 7455" \
     } else { \
         Write-Host "[LAUNCH] Windows Terminal not found, using separate windows" -ForegroundColor Yellow; \
         Start-Process powershell -ArgumentList "-NoProfile -Command Set-Location '$root/backend'; '$root/$bin/signing-server.exe'" -WindowStyle Normal; \
         Start-Sleep -Seconds 2; \
         Start-Process powershell -ArgumentList "-NoProfile -Command Set-Location '$root/tauri/wallet-ui'; npm run dev" -WindowStyle Normal; \
         Start-Sleep -Seconds 1; \
         Start-Process powershell -ArgumentList "-NoProfile -Command Set-Location '$root'; '$root/$bin/xfchess-tauri.exe'" -WindowStyle Minimized; \
         Start-Sleep -Seconds 1; \
         Start-Process powershell -ArgumentList "-NoProfile -Command Set-Location '$root'; '$root/$bin/xfchess.exe'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -Command Set-Location '$root/web-solana'; npm run dev" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -Command Set-Location '$root/tauri/tournament-admin'; npm run dev -- --port 7455" -WindowStyle Normal \
     }
    @Write-Host ""
    @Write-Host "Backend:          http://127.0.0.1:8090" -ForegroundColor White
    @Write-Host "Wallet UI (dev):  http://localhost:5174" -ForegroundColor White
    @Write-Host "Wallet Bridge:    http://localhost:7454" -ForegroundColor White
    @Write-Host "Web Frontend:     http://localhost:5173" -ForegroundColor White
    @Write-Host "Tournament Admin: http://localhost:7454/tournament-admin/" -ForegroundColor White
    @Write-Host "Program ID:       {{PROGRAM_ID}}" -ForegroundColor White
    @Write-Host "========================================" -ForegroundColor Cyan

# Launch two game instances sharing one backend — P1 window and P2 window, each with tabs
dev2: kill build build-wallet-ui
    @$root = (Get-Location).Path; \
     $bin = ("{{bin}}" -replace '/', '\'); \
     $env_common = "`$env:SIGNING_SERVICE_URL='{{SIGNING_SERVICE_URL}}'; `$env:BACKEND_URL='{{BACKEND_URL}}'; `$env:RUST_LOG='{{RUST_LOG}}'; `$env:HELIUS_API_KEY='{{HELIUS_API_KEY}}'"; \
     Set-Content -Path "$root\dev2-backend.ps1"   -Value "Set-Location '$root\backend'; `$env:JWT_SECRET='{{JWT_SECRET}}'; `$env:SIGNING_SERVICE_URL='{{SIGNING_SERVICE_URL}}'; & '$root\$bin\signing-server.exe'" -Encoding utf8; \
     Set-Content -Path "$root\dev2-wallet-p1.ps1" -Value "Set-Location '$root\tauri\wallet-ui'; npm run dev" -Encoding utf8; \
     Set-Content -Path "$root\dev2-tauri-p1.ps1"  -Value "$env_common; Set-Location '$root'; & '$root\$bin\xfchess-tauri.exe'" -Encoding utf8; \
     Set-Content -Path "$root\dev2-game-p1.ps1"   -Value "$env_common; Set-Location '$root'; & '$root\$bin\xfchess.exe'" -Encoding utf8; \
     Set-Content -Path "$root\dev2-wallet-p2.ps1" -Value "`$env:VITE_BRIDGE_PORT='7464'; Set-Location '$root\tauri\wallet-ui'; npx vite --port 5175" -Encoding utf8; \
     Set-Content -Path "$root\dev2-tauri-p2.ps1"  -Value "$env_common; `$env:XFCHESS_WALLET_PORT='7464'; `$env:XFCHESS_WALLET_URL='http://localhost:5175'; Set-Location '$root'; & '$root\$bin\xfchess-tauri.exe'" -Encoding utf8; \
     Set-Content -Path "$root\dev2-game-p2.ps1"   -Value "$env_common; `$env:XFCHESS_WALLET_PORT='7464'; Set-Location '$root'; & '$root\$bin\xfchess.exe'" -Encoding utf8; \
     $hasWT = Get-Command wt -ErrorAction SilentlyContinue; \
     if ($hasWT) { \
         Write-Host "[ P1 ] Opening Player 1 window..." -ForegroundColor Cyan; \
         cmd /c "wt -w new nt --title Backend powershell -NoProfile -File `"$root\dev2-backend.ps1`" ; nt --title `"Wallet UI`" powershell -NoProfile -File `"$root\dev2-wallet-p1.ps1`" ; nt --title Tauri powershell -NoProfile -File `"$root\dev2-tauri-p1.ps1`" ; nt --title Game powershell -NoProfile -File `"$root\dev2-game-p1.ps1`""; \
         Start-Sleep -Seconds 2; \
         Write-Host "[ P2 ] Opening Player 2 window..." -ForegroundColor Yellow; \
         cmd /c "wt -w new nt --title `"Wallet UI`" powershell -NoProfile -File `"$root\dev2-wallet-p2.ps1`" ; nt --title Tauri powershell -NoProfile -File `"$root\dev2-tauri-p2.ps1`" ; nt --title Game powershell -NoProfile -File `"$root\dev2-game-p2.ps1`"" \
     } else { \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-backend.ps1'" -WindowStyle Normal; \
         Start-Sleep -Seconds 2; \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-wallet-p1.ps1'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-tauri-p1.ps1'" -WindowStyle Minimized; \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-game-p1.ps1'" -WindowStyle Normal; \
         Start-Sleep -Seconds 1; \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-wallet-p2.ps1'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-tauri-p2.ps1'" -WindowStyle Minimized; \
         Start-Process powershell -ArgumentList "-NoProfile -File '$root\dev2-game-p2.ps1'" -WindowStyle Normal \
     }
    @Write-Host "  [P1] Backend :8090  Wallet UI :5174  Bridge :7454  (window: XFChess P1)" -ForegroundColor Cyan
    @Write-Host "  [P2] Wallet UI :5175  Bridge :7464  (window: XFChess P2)" -ForegroundColor Yellow

# ── Solana program ────────────────────────────────────────────────────────────

# Build Solana program (size-optimized)
build-program:
    anchor build

# Deploy to devnet
deploy-devnet:
    anchor deploy

# ── Monitoring ────────────────────────────────────────────────────────────────

# Start local Prometheus + Grafana monitoring stack
monitoring:
    docker-compose -f deploy/monitoring/docker-compose.local.yml up -d
    @Write-Host "Grafana: http://localhost:3000" -ForegroundColor Green

# Stop monitoring stack
monitoring-down:
    docker-compose -f deploy/monitoring/docker-compose.local.yml down

# ── Lint & test ───────────────────────────────────────────────────────────────

# Fast type-check all workspace crates (no codegen — much faster than build)
check:
    cargo check --workspace

# Fast type-check backend only
check-backend:
    cargo check -p backend

# Run all workspace tests
test:
    cargo test

# Run backend tests only
test-backend:
    cargo test -p backend

# Run Solana program tests
test-program:
    cargo test -p xfchess-game

# Format + clippy
lint:
    cargo fmt
    cargo clippy

# ── Database ──────────────────────────────────────────────────────────────────

# Wipe local SQLite databases (sessions + vault) for a clean dev state
db-reset:
    @Write-Host "[DB] Resetting local databases..." -ForegroundColor Yellow
    @Stop-Process -Name "signing-server" -Force -ErrorAction SilentlyContinue
    @Start-Sleep -Milliseconds 300
    @Remove-Item -Force backend/sessions.db -ErrorAction SilentlyContinue
    @Remove-Item -Force backend/vault.db -ErrorAction SilentlyContinue
    @Remove-Item -Force backend/sessions.db-shm -ErrorAction SilentlyContinue
    @Remove-Item -Force backend/sessions.db-wal -ErrorAction SilentlyContinue
    @Remove-Item -Force backend/vault.db-shm -ErrorAction SilentlyContinue
    @Remove-Item -Force backend/vault.db-wal -ErrorAction SilentlyContinue
    @Write-Host "[DB] Done — databases will be recreated with migrations on next backend start" -ForegroundColor Green

# Run SQLx migrations manually
db-migrate:
    Set-Location backend && sqlx migrate run && Set-Location ..

# ── Watch ─────────────────────────────────────────────────────────────────────

# Auto-rebuild + restart backend on file changes (requires: cargo install cargo-watch)
watch-backend:
    @Write-Host "[WATCH] Watching backend for changes (cargo-watch)..." -ForegroundColor Cyan
    @Write-Host "        Install if missing: cargo install cargo-watch" -ForegroundColor DarkGray
    cargo watch -w backend/src -w crates -x "build -p backend --bin signing-server"

# ── Utilities ─────────────────────────────────────────────────────────────────

# Open all local service URLs in the default browser
open:
    @Start-Process "http://127.0.0.1:8090/health"
    @Start-Process "http://localhost:5173"
    @Start-Process "http://localhost:7454/tournament-admin/"
    @Start-Process "http://localhost:3000"
    @Write-Host "Opened: backend health, web frontend, tournament admin, Grafana" -ForegroundColor Green

# Check what is holding port 8090
port-check:
    @$conns = netstat -aon 2>$null | Select-String ":8090"; \
     if ($conns) { $conns | ForEach-Object { $_.Line } } \
     else { Write-Host "Port 8090 is free" -ForegroundColor Green }

# Tail the backend log (if redirected to file)
logs:
    @if (Test-Path "backend/backend.log") { Get-Content backend/backend.log -Wait -Tail 50 } \
     else { Write-Host "No backend.log found — logs go to the terminal window" }

# Clean all build artifacts
clean:
    cargo clean
    @if (Test-Path "web-solana/dist") { Remove-Item -Recurse -Force web-solana/dist }
    @if (Test-Path "tauri/wallet-ui/dist") { Remove-Item -Recurse -Force tauri/wallet-ui/dist }
    @if (Test-Path "tauri/tournament-admin/dist") { Remove-Item -Recurse -Force tauri/tournament-admin/dist }
