// Single source of truth for where the admin panel talks to the backend.
//
// LOCAL      → a backend you run yourself on this machine (http://127.0.0.1:8090).
// PRODUCTION → the Hetzner VPS backend, reached ONLY through an SSH tunnel
//              (ssh -L 8091:127.0.0.1:8090). The admin API is never exposed on
//              the public HTTPS path (nginx returns 444 for /admin/*), so the
//              panel forwards a local port to the backend's loopback port over
//              SSH. See docs/plans/admin-panel-and-production-hardening.md §1.
//
// This is the ONLY place the VPS IP and tunnel parameters appear in the panel.

export type EnvId = "local" | "production";

export interface TunnelConfig {
  /** Local port the panel connects to (forwarded end). */
  localPort: number;
  /** Host the VPS forwards to — the backend's own loopback. */
  remoteHost: string;
  /** Backend port on the VPS. */
  remotePort: number;
  /** Restricted, no-shell SSH user that may only forward to the backend port. */
  sshUser: string;
  /** VPS address. */
  sshHost: string;
  /** Identity file. OpenSSH expands the leading ~ itself. */
  sshKey: string;
}

export interface EnvConfig {
  id: EnvId;
  label: string;
  /** Base URL every API/health/metrics call in the panel must derive from. */
  backendUrl: string;
  isProduction: boolean;
  /** Present only for environments reached through an SSH tunnel. */
  tunnel?: TunnelConfig;
}

/** The production VPS. Referenced nowhere else in the panel. */
export const VPS_HOST = "178.104.55.19";

export const ENVIRONMENTS: Record<EnvId, EnvConfig> = {
  local: {
    id: "local",
    label: "LOCAL",
    backendUrl: "http://127.0.0.1:8090",
    isProduction: false,
  },
  production: {
    id: "production",
    label: "PRODUCTION",
    backendUrl: "http://127.0.0.1:8091",
    isProduction: true,
    tunnel: {
      localPort: 8091,
      remoteHost: "127.0.0.1",
      remotePort: 8090,
      sshUser: "tunnel",
      sshHost: VPS_HOST,
      sshKey: "~/.ssh/xfchess_vps",
    },
  },
};

// Interactive ops terminal (Hetzner SSH panel) connects as the restricted
// `deploy` user — NOT root. deploy has NOPASSWD sudo for only
// `systemctl restart/reload/status` (see deploy/scripts/deploy.ps1 Step 2a).
export const OPS_SSH = {
  user: "deploy",
  host: VPS_HOST,
  key: "~/.ssh/xfchess_vps",
};

export function envById(id: EnvId): EnvConfig {
  return ENVIRONMENTS[id];
}

/** True for any http:// URL whose host is not loopback — rejected by the panel. */
export function isInsecureRemoteUrl(url: string): boolean {
  try {
    const u = new URL(url);
    if (u.protocol !== "http:") return false;
    return !(u.hostname === "127.0.0.1" || u.hostname === "localhost");
  } catch {
    return true;
  }
}
