// SSH tunnel manager for PRODUCTION mode.
//
// Spawns `ssh -N -L <localPort>:127.0.0.1:8090 tunnel@<vps>` and owns its
// lifetime: one tunnel at a time, killed on logout / app exit. The panel then
// talks to http://127.0.0.1:<localPort> exactly as if the backend were local.
//
// Nothing here reaches the public internet — the admin API is 444 on the public
// path by design; this forwards to the backend's loopback port over SSH.

import { Command, Child } from "@tauri-apps/plugin-shell";
import type { EnvConfig } from "../config/environments";

export type TunnelState = "down" | "connecting" | "up" | "error";

let child: Child | null = null;
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

async function waitForHealth(baseUrl: string, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(`${baseUrl}/health`);
      if (r.ok) return true;
    } catch {
      // forwarded port not answering yet
    }
    await new Promise((res) => setTimeout(res, 400));
  }
  return false;
}

/**
 * Ensure a working tunnel for `env`. No-op for environments without a tunnel
 * (LOCAL). Resolves once the backend answers /health through the forward, or
 * throws with a clear message on failure (and leaves state "error"/"down").
 */
export async function ensureTunnel(env: EnvConfig): Promise<void> {
  lastError = null;
  if (!env.tunnel) {
    // LOCAL — nothing to forward.
    await killTunnel();
    return;
  }
  if (child && state === "up") return;

  await killTunnel();
  const t = env.tunnel;
  setState("connecting");

  const args = [
    "-i", t.sshKey,
    "-o", "BatchMode=yes",
    "-o", "ExitOnForwardFailure=yes",
    "-o", "ServerAliveInterval=30",
    "-o", "StrictHostKeyChecking=accept-new",
    "-N",
    "-L", `${t.localPort}:${t.remoteHost}:${t.remotePort}`,
    `${t.sshUser}@${t.sshHost}`,
  ];

  const cmd = Command.create("ssh", args);
  cmd.on("close", () => {
    child = null;
    if (state !== "error") setState("down");
  });
  cmd.on("error", (e) => {
    child = null;
    lastError = String(e);
    setState("error");
  });

  child = await cmd.spawn();

  const ok = await waitForHealth(env.backendUrl, 8000);
  if (!ok) {
    lastError =
      "Tunnel opened but the backend did not answer /health. Check that the " +
      "'tunnel' SSH user exists on the VPS and your key is authorized.";
    await killTunnel();
    setState("error");
    throw new Error(lastError);
  }
  setState("up");
}

export async function killTunnel(): Promise<void> {
  if (child) {
    try {
      await child.kill();
    } catch {
      // already gone
    }
    child = null;
  }
  if (state !== "error") setState("down");
}
