# XFChess Rollback Guide

This document provides detailed instructions for rolling back a deployment on Hetzner in case of issues after a deployment.

## Prerequisites
- SSH access to the Hetzner server (default: `root@178.104.55.19`)
- PowerShell or a terminal with `ssh` command available

## Rollback Steps

### 1. Verify the Need for Rollback
Before proceeding with a rollback, confirm that there is an issue with the current deployment that cannot be resolved through other means. Check logs with:
```bash
ssh root@178.104.55.19 journalctl -u xfchess-backend -n 50
```

### 2. Execute Rollback Script
The rollback script restores the previous backend binary and, optionally, the databases to their state before the last deployment.

#### Rollback Binary Only
Run the following command to rollback only the backend binary:
```powershell
powershell -ExecutionPolicy Bypass -File deploy\rollback.ps1 -Server 178.104.55.19 -User root
```

#### Rollback Binary and Databases
If the issue involves data corruption or incompatible database changes, rollback both the binary and databases:
```powershell
powershell -ExecutionPolicy Bypass -File deploy\rollback.ps1 -Server 178.104.55.19 -User root -RestoreDb
```
**Warning**: Restoring databases will result in loss of any user registrations or KYC data added since the last backup snapshot.

### 3. Verify Rollback Success
After the rollback script completes, it will attempt to verify if the backend is responding. Ensure the API and health endpoints are operational:
- API: `http://178.104.55.19/api/user/status/11111111111111111111111111111111`
- Health: `http://178.104.55.19/health`

You can manually check logs if issues persist:
```bash
ssh root@178.104.55.19 journalctl -u xfchess-backend -f
```

### 4. Troubleshooting Rollback Failures
If the rollback fails:
- **No Backup Binary**: The script checks for `/opt/xfchess/signing-server-http.prev`. If not found, it means this might be the first deployment or the backup was not created. Manual intervention is required to install a known good binary.
- **Service Restart Issues**: If the `xfchess-backend` service fails to restart, check logs with `journalctl -u xfchess-backend -n 50`.
- **Database Backup Missing**: If `-RestoreDb` is used and no backups are found in `/opt/xfchess/backups/`, the script will skip database restoration. Manual database recovery may be needed.

### 5. Post-Rollback Actions
After a successful rollback, consider:
- Analyzing the cause of the failed deployment to prevent recurrence.
- Testing the deployment process in a staging environment before retrying.
- Updating any documentation or scripts if issues were found in the rollback process.

## Additional Notes
- Backups are created automatically during deployment and kept for 7 days for databases.
- Nightly cron jobs at 3am UTC create additional database snapshots, retained for 14 days.
- Always backup critical data manually if you suspect a deployment might be risky.
