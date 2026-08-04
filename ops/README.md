# XFChess Ops

`ops/` contains production and operations material. It is not gameplay code.

Use this folder when you are deploying the VPS, changing nginx/systemd,
running monitoring, backing up databases, rolling back, or rotating secrets.

## Map

| Path | Purpose |
| --- | --- |
| `scripts/` | VPS deploy and rollback PowerShell scripts. |
| `backend/` | systemd service file and backend environment template. |
| `nginx/` | Reverse proxy, TLS, and rate-limit config. |
| `monitoring/` | Prometheus, Grafana, Alertmanager, and alert rules. |
| `backup/` | Database backup/restore scripts and systemd timer units. |
| `staging/` | Staging service and nginx examples. |
| `docs/` | Deployment remediation checklists and rollback notes. |
| `SECRETS_ROTATION.md` | Runbook for rotating backend and authority secrets. |

## Common Commands

Local monitoring only:

```bash
cd ops/monitoring
./setup-local.sh
```

Production deploy:

```powershell
powershell -ExecutionPolicy Bypass -File ops\scripts\deploy.ps1 -Server 178.104.55.19 -User root
```

Rollback:

```powershell
powershell -ExecutionPolicy Bypass -File ops\scripts\rollback.ps1 -Server 178.104.55.19 -User root
```

## Deploy vs Release

Deploy updates the VPS backend, web frontend, nginx, and monitoring config.

Release publishes installable builds through GitHub Releases. The release
pipeline is tag-driven; use `scripts\push_and_release.ps1` from the repo root
to push the branch and release tag in one command.

## Secrets

Tracked files under `ops/` are templates and runbooks. Real values belong in
untracked `.env` files or the server's `/opt/xfchess/.env`, never in git.
