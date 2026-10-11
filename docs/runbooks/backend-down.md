# Backend unhealthy

Use this when health/readiness fails, the service crash-loops or API errors rise.

## Diagnose

On the server, inspect `systemctl status xfchess-backend` and
`journalctl -u xfchess-backend -n 100 --no-pager`. The restricted `deploy`
account can read journals; restart needs `sudo -n`.

Check private `http://127.0.0.1:8090/health` and `/readyz`, then public HTTPS.
A production validation error needs a corrected environment; readiness failure
needs database diagnosis. Check [disk](disk-full.md) and OOM/service logs.

## Recover and verify

- Restart a transient failure through the service manager.
- For a bad release, use [rollback](../../ops/docs/ROLLBACK_GUIDE.md); it restores
  backend, website and saved configuration, not database migrations.
- For corrupt/missing data, use the stopped-service recovery procedure.
- Verify expected health SHA, readiness and a representative authenticated read.
- After a restart, inspect persisted casual participants and relay rooms rather
  than treating an empty transient connection as a cancelled game.

Preserve redacted logs and incident evidence before cleanup.
