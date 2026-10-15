# Staging

Staging is isolated by user, environment, data directory and backend port 8091.
The desktop panel's production forward also uses local 8091 on the operator's
machine; it is unrelated to the VPS staging listener.

Create `xfchess-staging` and `/opt/xfchess-staging/data` on the VPS. Start from
[the backend environment template](../backend/.env.example), with separate keys,
`SIGNING_PORT=8091`, and both database URLs under `/opt/xfchess-staging/data`.
Never share production data or signing authorities with staging.

Install the built Linux `signing-server-http` binary and complete website under
`/opt/xfchess-staging`, then install [the service](xfchess-staging.service).
The service binds loopback; its user needs write access only to `data/`.

Install the shared nginx snippets and rate zones from `ops/nginx`, configure
`staging.YOUR_DOMAIN` and its certificate in [the staging site](nginx-staging.conf),
and run `nginx -t` before reloading. The shared route table keeps production and
staging Braid/SSE/WebSocket behavior identical and blocks public admin/metrics.

Verify private `/readyz`, JSON `/health`, the expected commit and public HTTPS
readiness. The production PowerShell release scripts target `/opt/xfchess` only;
they do not promise automated staging promotion or identical compiled artifacts.
