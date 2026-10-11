---
name: ops
description: Maintain XFChess Hetzner VPS deployment, Nginx proxying, systemd services, Prometheus/Grafana/Alertmanager and GitHub Actions. Use for deployment, monitoring, TLS and CI operations.
metadata:
  provenance: docs-derived
---

# Deployment and monitoring

Read `ops/`, the relevant runbook and `.github/workflows/`. Establish the actual
host, service and release path from configuration before running remote commands.
Use the Docker skills for Dockerfiles and Compose changes.

- Build and validate the concrete artifact before deployment. Reuse the existing
  release-switch/rollback path and migrations; check application and proxy health.
- For Nginx streaming routes, inspect buffering, read timeouts, forwarded headers
  and disconnect behavior. Preserve WebSocket upgrades and Braid/SSE streaming.
- Keep systemd process ownership, working directory, environment files, persistent
  data paths, restart behavior and graceful shutdown consistent with deployment.
- Keep monitoring bound to its configured interfaces. Preserve SSH-tunnel access
  and avoid creating duplicate node exporters or conflicting host-network ports.
- Give Prometheus metrics units and bounded labels. Player/game IDs and signatures
  are unsuitable unbounded labels; test counter reset and missing-series behavior.
- Maintain provisioned Grafana dashboards/datasources and stable UIDs. Verify
  alerts against actual failure/latency/resource metrics and their recovery state.
- In GitHub Actions, preserve pinned dependencies, build targets and artifact paths.
  Minimize token permissions, separate untrusted PR code from release secrets, and
  use structured environment input instead of injecting expressions into scripts.
- For Hetzner sizing/cost work, verify server type and current official regional
  prices, currency, VAT, included resources and traffic. Do not infer a tier from RAM alone.

Validate only affected configuration and workflows. Operational authorization comes
from the user's task; skill installation does not authorize deployment or publication.

Sources: [Nginx proxy module](https://nginx.org/en/docs/http/ngx_http_proxy_module.html),
[systemd services](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html),
[Prometheus naming](https://prometheus.io/docs/practices/naming/),
[Grafana provisioning](https://grafana.com/docs/grafana/latest/administration/provisioning/),
[Actions security](https://docs.github.com/en/actions/reference/security/secure-use),
[Hetzner Cloud](https://www.hetzner.com/cloud/).
