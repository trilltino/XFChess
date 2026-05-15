# XFChess Hosted Backend Checklist

This checklist ensures that all components and configurations for the XFChess backend on Hetzner are properly set up, deployed, and verified. Use this as a pre-deployment and post-deployment verification tool.

## Pre-Deployment Checklist

- [ ] **Environment Configuration**
  - [ ] `.env.production` file created from `deploy/.env.example`.
  - [ ] Required secrets set: `JWT_SECRET`, `IDENTITY_ENCRYPTION_KEY`, `IDENTITY_SALT`.
  - [ ] `ALLOWED_ORIGINS` set to the correct server address (e.g., `http://178.104.55.19`).
  - [ ] Database URLs point to correct paths: `SESSION_DB_URL` and `VAULT_DB_URL`.
  - [ ] Solana configuration correct: `SOLANA_RPC_URL`, `PROGRAM_ID`, `USDC_MINT`.
  - [ ] Key files available at expected paths: `FEE_PAYER_KEYS`, `VPS_AUTHORITY_KEY`, `KYC_AUTHORITY_KEY`.

- [ ] **Local Build Preparation**
  - [ ] Frontend build completed (`npm run build` in `web-solana`).
  - [ ] Git repository up-to-date and no uncommitted changes unless intentional.
  - [ ] Commit to be deployed is verified and tagged if necessary.

- [ ] **Server Access and Setup**
  - [ ] SSH access confirmed to Hetzner server (`root@178.104.55.19`).
  - [ ] SSH key for deployment set up and working (`~/.ssh/id_xfchess`).

## Deployment Checklist

- [ ] **Run Deployment Script**
  - [ ] Execute `powershell -ExecutionPolicy Bypass -File deploy\deploy.ps1 -Server 178.104.55.19 -User root`.
  - [ ] Monitor script output for errors during server setup, build, and upload phases.

- [ ] **Server-Side Build and Installation**
  - [ ] Linux build dependencies installed on Hetzner server.
  - [ ] Rust and Cargo installed for backend compilation.
  - [ ] Backend source synced to `/opt/xfchess/src/` with correct commit checked out.
  - [ ] Backend binary built on server and installed to `/opt/xfchess/signing-server-http`.

- [ ] **Configuration and Service Setup**
  - [ ] `.env` file uploaded to `/opt/xfchess/.env` with correct permissions.
  - [ ] Systemd service file uploaded to `/etc/systemd/system/xfchess-backend.service`.
  - [ ] Nginx configuration uploaded and enabled at `/etc/nginx/sites-available/xfchess`.
  - [ ] Frontend files uploaded to `/opt/xfchess/web/`.

- [ ] **Service Start and Verification**
  - [ ] `xfchess-backend` service restarted and enabled.
  - [ ] Nginx reloaded and configuration tested.

## Post-Deployment Verification

- [ ] **API and Health Endpoints**
  - [ ] API endpoint responds: `http://178.104.55.19/api/user/status/11111111111111111111111111111111`.
  - [ ] Health endpoint responds with 'OK': `http://178.104.55.19/health`.

- [ ] **Frontend Accessibility**
  - [ ] Frontend loads correctly at `http://178.104.55.19`.

- [ ] **Logs and Service Status**
  - [ ] Check service status with `systemctl status xfchess-backend`.
  - [ ] Review logs for errors: `journalctl -u xfchess-backend -n 50`.

- [ ] **Backup Confirmation**
  - [ ] Database backups created during deployment in `/opt/xfchess/backups/`.
  - [ ] Previous binary backed up as `/opt/xfchess/signing-server-http.prev`.

## Rollback Preparedness

- [ ] **Rollback Script Ready**
  - [ ] `rollback.ps1` script available and understood.
  - [ ] Know how to rollback binary only or binary with databases.

- [ ] **Manual Rollback Options**
  - [ ] Understand manual rollback if script fails: copy previous binary, restore database backups.

## Security and Hardening

- [ ] **Systemd Hardening**
  - [ ] Service runs as non-root user `xfchess`.
  - [ ] Hardening directives in place: `NoNewPrivileges`, `ProtectSystem`, `ReadWritePaths`, etc.
  - [ ] Environment validation checks in `ExecStartPre` ensure required variables are set.

- [ ] **File Permissions**
  - [ ] Sensitive files like `.env` and keys have restricted permissions (`chmod 600`).
  - [ ] Data directories owned by `xfchess` user.

- [ ] **Network Security**
  - [ ] Nginx configuration does not expose unnecessary endpoints.
  - [ ] Only required ports open on Hetzner firewall (80 for HTTP, 22 for SSH).

## Additional Notes
- If any checklist item fails, do not proceed with deployment or usage until resolved.
- Document any deviations or issues encountered during deployment for future reference.
- Consider setting up a staging environment for testing deployments before production.
