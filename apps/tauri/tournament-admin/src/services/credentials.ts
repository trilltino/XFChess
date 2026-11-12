// The admin API key lives in the OS credential store, owned by Rust
// (src/windows/tournament_admin.rs). The webview only holds it in memory.

import { invoke } from "@tauri-apps/api/core";
import type { EnvId } from "../config/environments";

// Keys earlier panel versions wrote to localStorage.
const legacyKeys = (env: EnvId) => [`admin_token_${env}`, "admin_token"];

/**
 * The saved key for `env`. A key still sitting in localStorage from an earlier
 * version is moved into the credential store on first read, then erased.
 */
export async function loadAdminToken(env: EnvId): Promise<string | null> {
  const saved = await invoke<string | null>("admin_token_load", { env });
  if (saved) return saved;
  for (const key of legacyKeys(env)) {
    const legacy = localStorage.getItem(key);
    if (!legacy) continue;
    await saveAdminToken(env, legacy);
    legacyKeys(env).forEach(k => localStorage.removeItem(k));
    return legacy;
  }
  return null;
}

export async function saveAdminToken(env: EnvId, token: string): Promise<void> {
  await invoke("admin_token_save", { env, token });
  legacyKeys(env).forEach(k => localStorage.removeItem(k));
}

export async function clearAdminToken(env: EnvId): Promise<void> {
  await invoke("admin_token_clear", { env });
}
