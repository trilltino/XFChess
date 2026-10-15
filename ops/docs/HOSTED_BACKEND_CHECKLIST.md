# Hosted backend verification

- Commit the intended source; deployment refuses a dirty checkout.
- Confirm the SSH host fingerprint and deployment identity before using strict SSH.
- Provision dependencies separately; verify the `xfchess` Rust toolchain and
  `/opt/xfchess/.env` against [the template](../backend/.env.example).
- Keep session/vault databases under `/opt/xfchess/data`. Match the program,
  authorities, RPC, router and USDC mint to one Solana network.
- Obtain a trusted certificate and enable the nginx site. Only SSH and HTTP/HTTPS
  should be public; backend, exporters and monitoring listen on loopback.
- Run [deploy.ps1](../scripts/deploy.ps1). It checks private health/readiness,
  the build commit and public HTTPS readiness before reporting success.
- Check that the website, multiplayer Braid resume and tournament feeds work.
  Public `/admin`, `/admin/*` and `/metrics` must close the connection.
- In Tauri, choose the correct environment and verify the displayed backend commit.
  Production admin calls use the restricted SSH tunnel; resource readings come
  through the authenticated backend API.
- Check Prometheus targets and Alertmanager. Telegram delivery is optional and
  reports explicitly when unconfigured.
- Verify an encrypted offsite backup with `restore-db.sh --check`. Confirm the
  backup timer is enabled and the age identity/vault secrets are recoverable offline.
- Before a schema change, assess [rollback compatibility](ROLLBACK_GUIDE.md).
