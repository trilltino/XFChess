# Operations validation

The former remediation checklist described retired deployment paths. Current
operating instructions live in [ops/README.md](../README.md), the
[hosted verification checklist](HOSTED_BACKEND_CHECKLIST.md) and
[recovery guide](ROLLBACK_GUIDE.md).

`ops/tests/integration.py` runs in a disposable Linux container. It exercises
nginx routing, Braid streaming/resume headers, WebSocket upgrades, private route
blocking, real SQLite/WAL snapshots, age encryption, stopped-service restore,
release switching and rollback. Service control, Cargo and HTTP readiness are
stubbed in release tests; production activation needs its own smoke test.

`ops/monitoring/tests/rules.yml` checks missing/failed scrapes, error ratios with
different labels, zero traffic and disk alerts for a root-mounted data directory.
The admin panel tests cover configured connection commands, unknown resource
readings, request deadlines, environment changes and polling cleanup.
