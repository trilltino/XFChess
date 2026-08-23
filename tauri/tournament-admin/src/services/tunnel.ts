// SSH tunnel manager for PRODUCTION mode — thin wrapper over the Rust side.
//
// The tunnel's actual lifetime is owned by Rust (`ensure_admin_tunnel` /
// `kill_admin_tunnel` in tauri/src/main.rs), not by this module. That is
// deliberate: this file used to spawn and supervise `ssh.exe` itself, and had
// no way to guarantee the process died. Closing the admin window or rebuilding
// the UI dropped the JS reference but left the real process running, and those
// orphans squatted port 8091 so every later tunnel failed to bind — which
// surfaced as "tunnel down" even though SSH auth was working perfectly.
//
// Rust ties the child to app state plus window-close and app-exit hooks, so
// orphans are structurally impossible rather than something to clean up by
// hand. It also polls /health from Rust (no CORS) instead of guessing a fixed
// timeout, and reports precise failures ("port held by another process",
// "ssh exited: Permission denied") instead of one generic string.
//
// See docs/plans/tournament-admin-connection-rearchitecture.md §3 Phase 2.

import { invoke } from "@tauri-apps/api/core";
import type { EnvConfig } from "../config/environments";

export type TunnelState = "down" | "connecting" | "up" | "error";

let state: TunnelState = "down";
let lastError: string | null = null;
const listeners = new Set<(s: TunnelState) => void>();

function setState(s: TunnelState) {
  state = s;
  listeners.forEach((l) => l(s));
}

export function getTunnelState(): TunnelState {
  return state;
}

export function getTunnelError(): string | null {
  return lastError;
}

/** Subscribe to tunnel state changes. Returns an unsubscribe fn. */
export function onTunnelState(cb: (s: TunnelState) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/**
 * Ensure a working tunnel for `env`. No-op for environments without a tunnel
 * (LOCAL). Resolves once the backend answers /health through the forward, or
 * throws with the specific reason Rust reported.
 *
 * Idempotent — a healthy existing tunnel is reused rather than duplicated.
 */
export async function ensureTunnel(env: EnvConfig): Promise<void> {
  lastError = null;
  if (!env.tunnel) {
    // LOCAL — nothing to forward.
    await killTunnel();
    return;
  }

  const t = env.tunnel;
  setState("connecting");
  try {
    await invoke<string>("ensure_admin_tunnel", {
      keyPath: t.sshKey,
      sshUser: t.sshUser,
      sshHost: t.sshHost,
      localPort: t.localPort,
      remoteHost: t.remoteHost,
      remotePort: t.remotePort,
    });
    setState("up");
  } catch (e) {
    lastError = typeof e === "string" ? e : String(e);
    setState("error");
    throw new Error(lastError);
  }
}

export async function killTunnel(): Promise<void> {
  try {
    await invoke("kill_admin_tunnel");
  } catch {
    // Nothing running, or the command isn't available in this build.
  }
  if (state !== "error") setState("down");
}
