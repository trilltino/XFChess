# Backend down / unhealthy

**Symptom:** `GET /health` fails or times out; users can't log in / load data; 5xx spike.
**Severity:** S1 (full outage) / S2 (degraded).
**Dashboards:** Grafana → API RED panel; `journalctl -u xfchess-backend`.

## Diagnose
1. `ssh $SERVER systemctl status xfchess-backend` — running? crash-looping?
2. `ssh $SERVER journalctl -u xfchess-backend -n 100 --no-pager` — panic? bad config?
   - `FATAL: invalid production configuration` → a required secret/placeholder failed
     `SigningConfig::validate` (see [SLO.md](../SLO.md) / `.env`). Fix `.env`, restart.
3. `curl -s https://$SERVER/readyz` — 503 means process up but **DB unreachable**.
4. Disk full? → [disk-full.md](disk-full.md). OOM? → `dmesg | tail`.

## Mitigate
1. Transient crash: `systemctl restart xfchess-backend` (Restart=on-failure already retries).
2. Bad deploy: **roll back** — `deploy/scripts/rollback.ps1` (restores previous binary).
3. Bad config: fix `/opt/xfchess/.env`, `systemctl restart`.
4. DB corruption: restore per [DR.md](../DR.md) (`restore-db.sh`), then start.

## Verify
1. `curl -s https://$SERVER/health` → `status: ok` and expected `git_sha`.
2. `curl -s https://$SERVER/readyz` → 200 `ready`.
3. Error rate returns to baseline in Grafana.

## Root cause / follow-up
- Capture logs, open a blameless postmortem, add a test/alert so it can't recur silently.
