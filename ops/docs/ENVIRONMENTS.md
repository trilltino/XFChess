# Environments

| Environment | Backend/data | Chain |
| --- | --- | --- |
| Local | Developer machine; `just dev` on Windows | Local defaults use devnet |
| Staging | Separate user/data/keys, loopback port 8091 | Explicit test-cluster configuration |
| Production | `/opt/xfchess`, `xfchess-backend`, loopback port 8090 | Endpoints, program and mint must belong to one configured cluster |

`APP_ENV=production` enables strict configuration checks; it does not select
Solana mainnet. Do not infer a live cluster from a hostname or this table.

Use [the backend template](../backend/.env.example). Secrets stay in untracked,
root-owned environment files; systemd supplies them to the service user. Staging
must not share production databases or signing authorities.

[Staging](../staging/STAGING.md) is installed and verified separately. Current
release scripts target `/opt/xfchess`; they do not implement `-Environment` or
same-binary promotion. Releases build clean committed source and verify health,
readiness, build SHA and public HTTPS readiness. See [ops](../OPERATIONS.md) and
[rollback](ROLLBACK_GUIDE.md).
