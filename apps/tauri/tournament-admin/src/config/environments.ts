// LOCAL uses the local backend. PRODUCTION reaches the private admin API
// through an SSH forward to backend loopback; nginx blocks public admin routes.
//
// The production VPS host and SSH key are built in, so nobody types them to sign in.
// A build can point elsewhere with VITE_XFCHESS_VPS_HOST / VITE_XFCHESS_SSH_KEY
// (see .env.example); there is no per-operator override at runtime.

export type EnvId = "local" | "production";

export interface TunnelConfig {
  /** Local port the panel connects to (forwarded end). */
  localPort: number;
  /** Host the VPS forwards to â€” the backend's own loopback. */
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

/** Operator-specific connection settings (not secrets). */
export interface OpsConfig {
  /** Production VPS address (IPv4). */
  vpsHost: string;
  /** SSH identity file for the tunnel and ops console. */
  sshKeyPath: string;
}

export const DEFAULT_OPS_CONFIG: OpsConfig = {
  vpsHost: (import.meta.env.VITE_XFCHESS_VPS_HOST as string | undefined)?.trim() || "178.104.55.19",
  sshKeyPath: (import.meta.env.VITE_XFCHESS_SSH_KEY as string | undefined)?.trim() || "~/.ssh/xfchess_vps",
};

const IPV4 = /^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$/;
// Mirrors the ssh key validator in capabilities/admin-shell.json.
const KEY_PATH = /^[A-Za-z0-9:_./\\~][A-Za-z0-9:_./\\~-]*$/;

/** Why `config` cannot reach production, or null when it can. */
export function opsConfigProblem(config: OpsConfig): string | null {
  if (!IPV4.test(config.vpsHost) || config.vpsHost.split(".").some(part => Number(part) > 255)) return "Set the production VPS host (an IPv4 address).";
  if (!KEY_PATH.test(config.sshKeyPath)) return "Set an SSH key path (no spaces; must not start with '-').";
  return null;
}

/** The built-in VPS host and SSH key. Values older builds saved in localStorage are ignored. */
export function getOpsConfig(): OpsConfig {
  return { ...DEFAULT_OPS_CONFIG };
}

export function envById(id: EnvId, ops: OpsConfig = getOpsConfig()): EnvConfig {
  if (id === "local") {
    return { id, label: "LOCAL", backendUrl: "http://127.0.0.1:8090", isProduction: false };
  }
  return {
    id,
    label: "PRODUCTION",
    backendUrl: "http://127.0.0.1:8091",
    isProduction: true,
    tunnel: {
      localPort: 8091,
      remoteHost: "127.0.0.1",
      remotePort: 8090,
      sshUser: "tunnel",
      sshHost: ops.vpsHost,
      sshKey: ops.sshKeyPath,
    },
  };
}

export const ENV_IDS: EnvId[] = ["local", "production"];

/** The ops console's SSH login: restricted deploy user, sudo limited to systemctl. */
export function opsSsh(ops: OpsConfig = getOpsConfig()) {
  return { user: "deploy", host: ops.vpsHost, key: ops.sshKeyPath };
}

/** True for any http:// URL whose host is not loopback â€” rejected by the panel. */
export function isInsecureRemoteUrl(url: string): boolean {
  try {
    const u = new URL(url);
    if (u.protocol !== "http:") return false;
    return !(u.hostname === "127.0.0.1" || u.hostname === "localhost");
  } catch {
    return true;
  }
}
