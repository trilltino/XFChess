// LOCAL uses the local backend. PRODUCTION reaches the private admin API
// through an SSH forward to backend loopback; nginx blocks public admin routes.

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
      sshKey: "C:/Users/isich/.ssh/xfchess_vps",
    },
  },
};

// SSH uses restricted deploy credentials with sudo limited to systemctl operations.
export const OPS_SSH = {
  user: "deploy",
  host: VPS_HOST,
  key: "C:/Users/isich/.ssh/xfchess_vps",
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
