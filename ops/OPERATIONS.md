# Operations

| Path | Responsibility |
| --- | --- |
| `scripts/` | Separate provisioning, committed releases, rollback and shared SSH/Linux helpers |
| `backend/`, `staging/` | Native systemd services and environment/proxy templates |
| `nginx/` | Shared public routes, streaming, TLS and per-IP rate limits |
| `monitoring/` | Private Prometheus, Grafana, Alertmanager and rule tests |
| `backup/` | SQLite snapshots, age encryption, offsite upload and stopped-service restore |
| `docs/` | Hosted verification, website deployment and recovery |

[Environments](docs/ENVIRONMENTS.md), [capacity](docs/CAPACITY.md) and
[reliability objectives](docs/RELIABILITY.md) describe cross-cutting operating constraints.

## Server setup

Confirm the VPS fingerprint through a trusted channel and add it to OpenSSH's
known hosts. Scripts use strict host checking, an existing identity and BatchMode;
they never regenerate keys or bypass authentication prompts. An encrypted identity
can be used through an SSH agent.

```powershell
powershell -File ops\scripts\provision.ps1 -Server 178.104.55.19
```

Provisioning installs Linux build/backup dependencies and restricted operator
accounts. It preserves existing firewall rules and root SSH authentication. Finish
these once per server:

1. Install the workspace-compatible Rust stable toolchain for `xfchess`, with
   `CARGO_HOME=/opt/xfchess/.cargo` and `RUSTUP_HOME=/opt/xfchess/.rustup`.
2. Create `/opt/xfchess/.env` from [backend/.env.example](backend/.env.example).
   Set real keys and match every chain endpoint, program and mint to one network.
   The template uses devnet; `APP_ENV=production` controls application security.
   Keep the file root-owned and mode 600; systemd passes its environment to `xfchess`.
3. For a new server, install `nginx/{routes,proxy}.conf` as
   `/etc/nginx/snippets/xfchess-{routes,proxy}.conf`, install
   `xfchess_rate_limit.conf` under `/etc/nginx/conf.d/`, and enable
   `nginx-http.conf` as `/etc/nginx/sites-enabled/xfchess`. Remove Ubuntu's default
   site only if it conflicts. Run `nginx -t` before reloading.
4. Point the site's DNS to this VPS. Obtain a certificate using
   `certbot certonly --webroot -w /var/www/certbot -d xfchess.com` and enable
   the distribution's certbot renewal timer. The release installs the HTTPS site.
5. Allow SSH and 80/443 in the Hetzner/host firewall. Keep 8090, 9100, 9090,
   9093 and 3000 private. A single native node exporter should listen on
   `127.0.0.1:9100`; monitoring does not install a duplicate exporter.
6. Install Docker and Compose v2+ using supported distribution/vendor packages.
   The existing VPS uses standalone Compose; both standalone and plugin work.

## Releases

```powershell
powershell -File ops\scripts\deploy.ps1 -Server 178.104.55.19 -Domain xfchess.com
powershell -File ops\scripts\rollback.ps1 -Server 178.104.55.19
```

Both commands require the deployment identity authorized for root. The restricted
`deploy` user is for status/logs/restarts and monitoring tunnels, not provisioning
or release installation. Override `-IdentityFile` when needed.

Deployment refuses a dirty checkout, archives HEAD, builds the website locally
and the locked backend on Linux, then snapshots databases before activation.
One server lock serializes deployment and rollback. `/opt/xfchess/current` selects
`releases/<id>`; the stable binary and web paths link into it. Readiness, JSON
health, the build commit and public HTTPS readiness must pass. Failed activation
restores the preceding release/configuration. Database migrations are not reversed.

The first upgrade imports the old installation as a `legacy-*` release. First
installation supports an empty binary/web layout. Release directories, incoming
archives and private pre-release snapshots remain available for inspection; prune
only releases outside `current`/`previous`/`ops` and recovery points you have retired.
The `ops` link retains the latest recovery tools across application rollbacks;
an existing tooling directory is preserved before replacing it with that link.
See [rollback and database recovery](docs/ROLLBACK_GUIDE.md).

## Monitoring and backups

Local monitoring: `bash ops/monitoring/setup-local.sh`. Production monitoring:
`bash /opt/xfchess/ops/monitoring/setup.sh`, or deploy with `-UpdateMonitoring`.
Back up the existing Grafana data volume before upgrading from the older stack;
application release rollback does not downgrade monitoring databases. See the
[Grafana upgrade guide](https://grafana.com/docs/grafana/latest/upgrade-guide/).

All dashboards bind loopback. Forward them with
`ssh -N -L 3000:127.0.0.1:3000 -L 9090:127.0.0.1:9090 deploy@SERVER`.
Grafana's password stays in `monitoring/.env`. Optional Telegram settings in that
file enable delivery; absent settings leave alerts visible in Alertmanager.

Set `BACKUP_AGE_RECIPIENT`, `BACKUP_REMOTE` and `RCLONE_CONFIG` in the backend env.
Make rclone credentials readable by `xfchess`; keep the age private identity
offline. Deployment installs the backup units and enables the timer when the
backup settings exist. It runs at 03:17 UTC with up to ten minutes of jitter;
encrypted local copies default to 14 days. Offsite retention belongs to the bucket.
After a verified encrypted backup, retire the old root cron entry invoking
`/opt/xfchess/backup.sh`; it predates encrypted backups. JSONL/archive data and the
secret environment need their own recovery policy; this timer covers SQLite only.

## Validation

```bash
docker run --rm -v "$PWD/ops:/work:ro" debian:bookworm-slim bash -c \
  'apt-get update -qq && apt-get install -y -qq python3 sqlite3 age nginx openssl >/dev/null && python3 /work/tests/integration.py'
docker run --rm --network none --entrypoint promtool -v "$PWD/ops/monitoring:/etc/prometheus:ro" \
  prom/prometheus:v3.15.0 test rules /etc/prometheus/tests/rules.yml
```

The Linux fixture refuses to run outside a disposable root Docker container.
See [the validation coverage](docs/E2E_REMEDIATION.md) and
[the hosted checklist](docs/HOSTED_BACKEND_CHECKLIST.md).
