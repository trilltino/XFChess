# Staging deploy (run on the VPS)

Ready-to-use staging artifacts. These must run **on the server** (they touch systemd +
nginx), so they can't be applied from a dev machine. See [../../docs/ENVIRONMENTS.md](../../docs/ENVIRONMENTS.md).

```bash
# 1. Isolated user + dirs
sudo useradd -r xfchess-staging || true
sudo mkdir -p /opt/xfchess-staging/data /opt/xfchess-staging/web
sudo chown -R xfchess-staging:xfchess-staging /opt/xfchess-staging

# 2. Config (fill in SEPARATE secrets from prod; openssl rand -hex 32)
sudo cp deploy/staging/.env.staging.example /opt/xfchess-staging/.env
sudo chmod 600 /opt/xfchess-staging/.env && sudo nano /opt/xfchess-staging/.env

# 3. Build once, deploy the same artifact to staging
cargo build -p backend --bin signing-server --release
sudo cp target/release/signing-server /opt/xfchess-staging/signing-server-http
# (build the web bundle and copy xfchessdotcom/dist → /opt/xfchess-staging/web)

# 4. systemd unit
sudo cp deploy/staging/xfchess-staging.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now xfchess-staging

# 5. nginx (edit YOUR_DOMAIN, get a cert first)
sudo certbot --nginx -d staging.YOUR_DOMAIN
sudo cp deploy/staging/nginx-staging.conf /etc/nginx/sites-available/xfchess-staging
sudo ln -s /etc/nginx/sites-available/xfchess-staging /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx

# 6. Smoke test
curl -fsS https://staging.YOUR_DOMAIN/health   # status:ok + git_sha
curl -fsS -o /dev/null -w '%{http_code}\n' https://staging.YOUR_DOMAIN/readyz  # 200
```

Promotion: verify on staging, then run the prod deploy **from the same commit** (the
`git_sha` on `/health` proves parity). See ENVIRONMENTS.md §"Same-artifact promotion".
