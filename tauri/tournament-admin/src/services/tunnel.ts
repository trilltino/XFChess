// Rust owns the SSH child lifetime and health polling. Invoke its tunnel API
// so closing the window or app also closes the tunnel.

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
 * Ensure a tunnel and resolve after backend health succeeds. Reuse healthy
 * tunnels, no-op for LOCAL, and propagate specific Rust failures.
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
