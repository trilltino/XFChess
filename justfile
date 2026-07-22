# XFChess dev task runner
# Install: cargo install just  (or winget install Casey.Just)
# Usage:   just dev            — full local stack
#          just build          — build all Rust binaries
#          just kill           — stop all running XFChess processes
#          just backend        — build + run backend only
#          just game           — build + run game only
#          just viz            — standalone Triton/network visualiser (Tauri)

set windows-shell := ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command"]

# ── Environment ───────────────────────────────────────────────────────────────
#
# Secrets are loaded from an untracked `.env` (see `.env.example`). This file is
# TRACKED — never hardcode private keys or API keys here. The fallbacks below let
# `just dev` boot without a `.env` (using public RPC + throwaway crypto), but
# on-chain signing needs real values supplied via `.env`.
set dotenv-load := true

# Public, non-secret config (safe to keep inline)
export BACKEND_URL             := "http://127.0.0.1:8090"
export SIGNING_SERVICE_URL     := "http://127.0.0.1:8090"
export ER_RPC_URL              := "https://devnet.magicblock.app"
export MAGIC_BLOCK_RPC_URL     := "https://devnet.magicblock.app"
export PROGRAM_ID              := "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
export TOURNAMENT_FEE_RECIPIENT    := "uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C"
export RUST_LOG                := "info"

# Secrets (sourced from .env; fallbacks are non-production)
export SOLANA_RPC_URL          := env_var_or_default("SOLANA_RPC_URL", "https://api.devnet.solana.com")
export SOLANA_RPC_FALLBACK_URL := env_var_or_default("SOLANA_RPC_FALLBACK_URL", "https://api.devnet.solana.com")
export HELIUS_API_KEY          := env_var_or_default("HELIUS_API_KEY", "")
export JWT_SECRET              := env_var_or_default("JWT_SECRET", "0000000000000000000000000000000000000000000000000000000000000000")
export IDENTITY_ENCRYPTION_KEY := env_var_or_default("IDENTITY_ENCRYPTION_KEY", "0000000000000000000000000000000000000000000000000000000000000000")
export IDENTITY_SALT           := env_var_or_default("IDENTITY_SALT", "1111111111111111111111111111111111111111111111111111111111111111")
export FEE_PAYER_KEYS          := env_var_or_default("FEE_PAYER_KEYS", "")
export VPS_AUTHORITY_KEY       := env_var_or_default("VPS_AUTHORITY_KEY", "")
export KYC_AUTHORITY_KEY       := env_var_or_default("KYC_AUTHORITY_KEY", "")
export ADMIN_API_KEY           := env_var_or_default("ADMIN_API_KEY", "dev")

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
     Stop-Process -Name "xfchess-viz" -Force; \
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

# Build tournament admin UI dist (only if missing) — served by the Tauri shell
build-admin-ui:
    @if (-not (Test-Path "tauri/tournament-admin/dist")) { \
        Write-Host "[BUILD] Building Tournament Admin UI..." -ForegroundColor Cyan; \
        Set-Location tauri/tournament-admin; npm install; npm run build; Set-Location ../.. \
    } else { \
        Write-Host "[BUILD] Tournament Admin dist exists, skipping (run 'just build-admin-ui-force' to rebuild)" \
    }

# Force-rebuild tournament admin UI
build-admin-ui-force:
    Set-Location tauri/tournament-admin; npm install; if ($?) { npm run build }; Set-Location ../..

# Install web frontend dependencies (only if node_modules is missing)
build-web-ui:
    @if (-not (Test-Path "web-solana/node_modules/.bin/vite")) { \
        Write-Host "[BUILD] Installing Web Frontend dependencies..." -ForegroundColor Cyan; \
        Set-Location web-solana; npm install; Set-Location .. \
    } else { \
        Write-Host "[BUILD] Web Frontend node_modules exists, skipping" \
    }

# Force-rebuild wallet UI
build-wallet-ui-force:
    Set-Location tauri/wallet-ui; npm install; if ($?) { npm run build }; Set-Location ../..

# Build web frontend (production)
build-web:
    Set-Location web-solana; npm install; if ($?) { npm run build }

# ── Run individual services ───────────────────────────────────────────────────

# Run backend only (builds first)
backend: build-backend
    @Write-Host "[BACKEND] Starting signing-server on :8090" -ForegroundColor Cyan
    Set-Location backend; ../{{bin}}/signing-server.exe

# Run game client only (builds first)
game: build-game
    @Write-Host "[GAME] Starting XFChess" -ForegroundColor Cyan
    ./{{bin}}/xfchess.exe

# Build game with Tracy profiling instrumentation (debug + trace_tracy)
build-profile:
    cargo build --bin xfchess --features profile

# Launch Tracy profiler then the instrumented game (they auto-connect)
# Tracy download: C:\Users\isich\Downloads\windows-0.13.1\Tracy.exe
profile: build-profile
    @Write-Host "[PROFILE] Starting Tracy profiler..." -ForegroundColor Magenta
    @Start-Process "C:\Users\isich\Downloads\windows-0.13.1\tracy-profiler.exe"
    @Start-Sleep -Seconds 2
    @Write-Host "[PROFILE] Starting instrumented game (trace_tracy)..." -ForegroundColor Magenta
    @Write-Host "[PROFILE] Make a move to reproduce the stutter — watch for the spike in Tracy" -ForegroundColor Yellow
    ./{{bin}}/xfchess.exe

# Run web frontend dev server
web:
    Set-Location web-solana; npm run dev

# Open the tournament admin desktop window (starts a backend if :8090 is down,
# reuses a running Tauri shell, else launches one)
admin: build-admin-ui build-tauri build-backend
    @$root = (Get-Location).Path; \
     $bin = "{{bin}}"; \
     if (-not (netstat -aon | Select-String ":8090.*LISTENING")) { \
         Write-Host "[ADMIN] No backend on :8090 - starting one in a new window" -ForegroundColor Yellow; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -Command Set-Location '$root\backend'; & '$root\$bin\signing-server.exe'" -WindowStyle Normal; \
         Start-Sleep -Seconds 2 \
     }; \
     try { \
        Invoke-RestMethod -Method Post -Uri "http://localhost:7454/api/open-tournament-admin" -TimeoutSec 2 -ErrorAction Stop | Out-Null; \
        Write-Host "[ADMIN] Opened admin window in the running Tauri shell" -ForegroundColor Green \
    } catch { \
        Write-Host "[ADMIN] Launching Tauri shell with admin window" -ForegroundColor Cyan; \
        $env:XFCHESS_OPEN_ADMIN = '1'; \
        & "./{{bin}}/xfchess-tauri.exe" \
    }

# Run backend + web frontend only (for e2e web testing, e.g. sign up)
web-stack: kill build-backend build-web-ui
    @Write-Host "" -ForegroundColor White
    @Write-Host "========================================" -ForegroundColor Cyan
    @Write-Host " XFChess Web Stack (backend + web)" -ForegroundColor Cyan
    @Write-Host "========================================" -ForegroundColor Cyan
    @$wt = (Get-Command wt -ErrorAction SilentlyContinue).Source; \
     if (-not $wt) { $cand = "$env:LOCALAPPDATA\Microsoft\WindowsApps\wt.exe"; if (Test-Path $cand) { $wt = $cand } }; \
     $bin = "{{bin}}"; \
     $root = (Get-Location).Path; \
     if ($wt) { \
         Write-Host "[LAUNCH] Using Windows Terminal tabs" -ForegroundColor Green; \
         & $wt -w 0 nt --title "Backend" -d "$root/backend" powershell -NoProfile -NoExit -Command "& '$root/$bin/signing-server.exe'"; \
         Start-Sleep -Seconds 2; \
         & $wt -w 0 nt --title "Web Frontend" -d "$root/web-solana" powershell -NoProfile -NoExit -Command "npm run dev" \
     } else { \
         Write-Host "[LAUNCH] Windows Terminal not found, using separate windows" -ForegroundColor Yellow; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -Command Set-Location '$root/backend'; & '$root/$bin/signing-server.exe'" -WindowStyle Normal; \
         Start-Sleep -Seconds 2; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -Command Set-Location '$root/web-solana'; npm run dev" -WindowStyle Normal \
     }
    @Write-Host ""
    @Write-Host "Backend:      http://127.0.0.1:8090" -ForegroundColor White
    @Write-Host "Web Frontend: http://localhost:5173" -ForegroundColor White
    @Write-Host "========================================" -ForegroundColor Cyan

# ── Visualiser ────────────────────────────────────────────────────────────────

# Install visualiser deps (only if node_modules is missing)
build-viz-ui:
    @if (-not (Test-Path "viz/node_modules/.bin/vite")) { \
        Write-Host "[BUILD] Installing Visualiser dependencies..." -ForegroundColor Cyan; \
        Set-Location viz; npm install; Set-Location .. \
    } else { \
        Write-Host "[BUILD] Visualiser node_modules exists, skipping" \
    }

# Run the standalone Tauri network visualiser (Triton RPC benchmark + topology)
viz: build-viz-ui
    @Write-Host "[VIZ] Launching XFChess Network Visualiser..." -ForegroundColor Cyan
    Set-Location viz; npm run tauri dev

# Build the visualiser as a distributable installer (bundle under viz/src-tauri/target)
viz-build: build-viz-ui
    Set-Location viz; npm run tauri build

# Run the CLI RPC benchmark — text output, no GUI (uses $SOLANA_RPC_URL from .env)
viz-bench:
    cargo run -p er-cu-benchmark --bin triton-bench -- read-load

# ── Full dev stack ────────────────────────────────────────────────────────────

# Build everything then launch full local stack (uses Windows Terminal tabs if available)
dev: kill build build-wallet-ui build-admin-ui build-web-ui
    @Write-Host "" -ForegroundColor White
    @Write-Host "========================================" -ForegroundColor Cyan
    @Write-Host " XFChess Local Dev Stack" -ForegroundColor Cyan
    @Write-Host "========================================" -ForegroundColor Cyan
    @$wt = (Get-Command wt -ErrorAction SilentlyContinue).Source; \
     if (-not $wt) { $cand = "$env:LOCALAPPDATA\Microsoft\WindowsApps\wt.exe"; if (Test-Path $cand) { $wt = $cand } }; \
     $bin = "{{bin}}"; \
     $root = (Get-Location).Path; \
     New-Item -ItemType Directory -Force -Path "$root\tmp" | Out-Null; \
     $env_common = "`$env:SIGNING_SERVICE_URL='{{SIGNING_SERVICE_URL}}'; `$env:BACKEND_URL='{{BACKEND_URL}}'; `$env:RUST_LOG='{{RUST_LOG}}'; `$env:HELIUS_API_KEY='{{HELIUS_API_KEY}}'; `$env:SOLANA_RPC_URL='{{SOLANA_RPC_URL}}'; `$env:ER_RPC_URL='{{ER_RPC_URL}}'; `$env:PROGRAM_ID='{{PROGRAM_ID}}'; `$env:XFCHESS_WEB_URL='http://localhost:5173'"; \
     Set-Content -Path "$root\tmp\dev-tauri.ps1" -Value "$env_common; `$env:XFCHESS_OPEN_ADMIN='1'; Set-Location '$root'; & '$root/$bin/xfchess-tauri.exe'" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev-game.ps1"  -Value "$env_common; Set-Location '$root'; & '$root/$bin/xfchess.exe'" -Encoding utf8; \
     if ($wt) { \
         Write-Host "[LAUNCH] Using Windows Terminal tabs" -ForegroundColor Green; \
         & $wt -w 0 nt --title "Backend" -d "$root/backend" powershell -NoProfile -NoExit -Command "& '$root/$bin/signing-server.exe'"; \
         Start-Sleep -Seconds 2; \
         & $wt -w 0 nt --title "Wallet UI" -d "$root/tauri/wallet-ui" powershell -NoProfile -NoExit -Command "npm run dev"; \
         Start-Sleep -Seconds 1; \
         & $wt -w 0 nt --title "Tauri" -d "$root" powershell -NoProfile -NoExit -File "$root\tmp\dev-tauri.ps1"; \
         Start-Sleep -Seconds 1; \
         & $wt -w 0 nt --title "Game" -d "$root" powershell -NoProfile -NoExit -File "$root\tmp\dev-game.ps1"; \
         & $wt -w 0 nt --title "Web Frontend" -d "$root/web-solana" powershell -NoProfile -NoExit -Command "npm run dev" \
     } else { \
         Write-Host "[LAUNCH] Windows Terminal not found, using separate windows" -ForegroundColor Yellow; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -Command Set-Location '$root/backend'; & '$root/$bin/signing-server.exe'" -WindowStyle Normal; \
         Start-Sleep -Seconds 2; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -Command Set-Location '$root/tauri/wallet-ui'; npm run dev" -WindowStyle Normal; \
         Start-Sleep -Seconds 1; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev-tauri.ps1'" -WindowStyle Minimized; \
         Start-Sleep -Seconds 1; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev-game.ps1'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -Command Set-Location '$root/web-solana'; npm run dev" -WindowStyle Normal \
     }
    @Write-Host ""
    @Write-Host "Backend:          http://127.0.0.1:8090" -ForegroundColor White
    @Write-Host "Wallet UI (dev):  http://localhost:5174" -ForegroundColor White
    @Write-Host "Wallet Bridge:    http://localhost:7454" -ForegroundColor White
    @Write-Host "Web Frontend:     http://localhost:5173" -ForegroundColor White
    @Write-Host "Tournament Admin: desktop window opens automatically — local token: dev" -ForegroundColor White
    @Write-Host "Program ID:       {{PROGRAM_ID}}" -ForegroundColor White
    @Write-Host "========================================" -ForegroundColor Cyan

# Launch two game instances sharing one backend — P1 window and P2 window, each with tabs
dev2: kill build build-wallet-ui build-admin-ui build-web-ui
    @$root = (Get-Location).Path; \
     $bin = ("{{bin}}" -replace '/', '\'); \
     New-Item -ItemType Directory -Force -Path "$root\tmp" | Out-Null; \
     $env_common = "`$env:SIGNING_SERVICE_URL='{{SIGNING_SERVICE_URL}}'; `$env:BACKEND_URL='{{BACKEND_URL}}'; `$env:RUST_LOG='{{RUST_LOG}}'; `$env:HELIUS_API_KEY='{{HELIUS_API_KEY}}'; `$env:SOLANA_RPC_URL='{{SOLANA_RPC_URL}}'; `$env:ER_RPC_URL='{{ER_RPC_URL}}'; `$env:PROGRAM_ID='{{PROGRAM_ID}}'; `$env:XFCHESS_WEB_URL='http://localhost:5173'"; \
     Set-Content -Path "$root\tmp\dev2-backend.ps1"   -Value "Set-Location '$root\backend'; `$env:JWT_SECRET='{{JWT_SECRET}}'; `$env:SIGNING_SERVICE_URL='{{SIGNING_SERVICE_URL}}'; `$env:IDENTITY_ENCRYPTION_KEY='{{IDENTITY_ENCRYPTION_KEY}}'; `$env:IDENTITY_SALT='{{IDENTITY_SALT}}'; `$env:SOLANA_RPC_URL='{{SOLANA_RPC_URL}}'; `$env:ER_RPC_URL='{{ER_RPC_URL}}'; `$env:PROGRAM_ID='{{PROGRAM_ID}}'; `$env:FEE_PAYER_KEYS='{{FEE_PAYER_KEYS}}'; `$env:VPS_AUTHORITY_KEY='{{VPS_AUTHORITY_KEY}}'; `$env:KYC_AUTHORITY_KEY='{{KYC_AUTHORITY_KEY}}'; `$env:TOURNAMENT_FEE_RECIPIENT='{{TOURNAMENT_FEE_RECIPIENT}}'; `$env:ADMIN_API_KEY='{{ADMIN_API_KEY}}'; `$env:RUST_LOG='{{RUST_LOG}}'; & '$root\$bin\signing-server.exe'" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-wallet-p1.ps1" -Value "Set-Location '$root\tauri\wallet-ui'; npm run dev" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-tauri-p1.ps1"  -Value "$env_common; `$env:XFCHESS_OPEN_ADMIN='1'; Set-Location '$root'; & '$root\$bin\xfchess-tauri.exe'" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-game-p1.ps1"   -Value "$env_common; `$env:XFCHESS_NODE_KEY_PATH='$root\tmp\node_key_p1'; Set-Location '$root'; & '$root\$bin\xfchess.exe'" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-web.ps1"        -Value "Set-Location '$root\web-solana'; npm run dev" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-wallet-p2.ps1" -Value "`$env:VITE_BRIDGE_PORT='7464'; Set-Location '$root\tauri\wallet-ui'; npx vite --port 5175" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-tauri-p2.ps1"  -Value "$env_common; `$env:XFCHESS_WALLET_PORT='7464'; `$env:XFCHESS_WALLET_URL='http://localhost:5175'; Set-Location '$root'; & '$root\$bin\xfchess-tauri.exe'" -Encoding utf8; \
     Set-Content -Path "$root\tmp\dev2-game-p2.ps1"   -Value "$env_common; `$env:XFCHESS_WALLET_PORT='7464'; `$env:XFCHESS_NODE_KEY_PATH='$root\tmp\node_key_p2'; Set-Location '$root'; & '$root\$bin\xfchess.exe'" -Encoding utf8; \
     $wt = (Get-Command wt -ErrorAction SilentlyContinue).Source; \
     if (-not $wt) { $cand = "$env:LOCALAPPDATA\Microsoft\WindowsApps\wt.exe"; if (Test-Path $cand) { $wt = $cand } }; \
     if ($wt) { \
         Write-Host "[ P1 ] Opening Player 1 window..." -ForegroundColor Cyan; \
         cmd /c "`"$wt`" -w new nt --title Backend -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-backend.ps1`" ; nt --title `"Wallet UI`" -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-wallet-p1.ps1`" ; nt --title Tauri -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-tauri-p1.ps1`" ; nt --title Game -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-game-p1.ps1`" ; nt --title `"Web Frontend`" -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-web.ps1`""; \
         Start-Sleep -Seconds 2; \
         Write-Host "[ P2 ] Opening Player 2 window..." -ForegroundColor Yellow; \
         cmd /c "`"$wt`" -w new nt --title `"Wallet UI`" -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-wallet-p2.ps1`" ; nt --title Tauri -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-tauri-p2.ps1`" ; nt --title Game -d `"$root`" powershell -NoProfile -NoExit -File `"$root\tmp\dev2-game-p2.ps1`"" \
     } else { \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-backend.ps1'" -WindowStyle Normal; \
         Start-Sleep -Seconds 2; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-wallet-p1.ps1'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-tauri-p1.ps1'" -WindowStyle Minimized; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-game-p1.ps1'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-web.ps1'" -WindowStyle Normal; \
         Start-Sleep -Seconds 1; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-wallet-p2.ps1'" -WindowStyle Normal; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-tauri-p2.ps1'" -WindowStyle Minimized; \
         Start-Process powershell -ArgumentList "-NoProfile -NoExit -File '$root\tmp\dev2-game-p2.ps1'" -WindowStyle Normal \
     }
    @Write-Host "  [P1] Backend :8090  Wallet UI :5174  Bridge :7454  (window: XFChess P1)" -ForegroundColor Cyan
    @Write-Host "  [P2] Wallet UI :5175  Bridge :7464  (window: XFChess P2)" -ForegroundColor Yellow
    @Write-Host "  Web Frontend: http://localhost:5173" -ForegroundColor White
    @Write-Host "  Tournament Admin: desktop window opens automatically from P1 — local token: dev" -ForegroundColor Green

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
    cargo check --workspace --features solana

# Fast type-check backend only
check-backend:
    cargo check -p backend

# Run all workspace tests
test:
    cargo test

# Run backend tests only
test-backend:
    cargo test -p backend

# Backend end-to-end API tests (self-contained — spawns its own in-process server)
test-e2e:
    cargo test -p backend --test e2e_api

# Live RPC smoke tests (Tier T2) — hits whatever SOLANA_RPC_URL is actually set
# to (Triton One in prod/staging). Needs a real RPC URL exported first; each
# test skips itself harmlessly if unset. See backend/tests/e2e_rpc_live.rs.
test-rpc-live:
    cargo test -p backend --test e2e_rpc_live -- --ignored --nocapture

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
    Set-Location backend; sqlx migrate run

# ── Watch ─────────────────────────────────────────────────────────────────────

# Auto-rebuild + restart backend on file changes (requires: cargo install cargo-watch)
watch-backend:
    @Write-Host "[WATCH] Watching backend for changes (cargo-watch)..." -ForegroundColor Cyan
    @Write-Host "        Install if missing: cargo install cargo-watch" -ForegroundColor DarkGray
    cargo watch -w backend/src -w crates -x "build -p backend --bin signing-server"

# ── Utilities ─────────────────────────────────────────────────────────────────

# Open all local service URLs in the default browser (tournament admin is a desktop window: 'just admin')
open:
    @Start-Process "http://127.0.0.1:8090/health"
    @Start-Process "http://localhost:5173"
    @Start-Process "http://localhost:3000"
    @Write-Host "Opened: backend health, web frontend, Grafana (tournament admin: 'just admin')" -ForegroundColor Green

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
    @if (Test-Path "viz/dist") { Remove-Item -Recurse -Force viz/dist }
    @if (Test-Path "tauri/wallet-ui/dist") { Remove-Item -Recurse -Force tauri/wallet-ui/dist }
    @if (Test-Path "tauri/tournament-admin/dist") { Remove-Item -Recurse -Force tauri/tournament-admin/dist }
