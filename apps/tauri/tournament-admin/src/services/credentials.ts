// The admin session token lives in the OS credential store, owned by Rust
// (src/windows/tournament_admin.rs), one entry per environment, so a restart within the
// session's eight hours signs straight back in. The webview only holds it in memory.

import { invoke } from "@tauri-apps/api/core";
import type { EnvId } from "../config/environments";

export async function loadAdminSession(env: EnvId): Promise<string | null> {
  return invoke<string | null>("admin_token_load", { env });
}

export async function saveAdminSession(env: EnvId, token: string): Promise<void> {
  await invoke("admin_token_save", { env, token });
}

export async function clearAdminSession(env: EnvId): Promise<void> {
  await invoke("admin_token_clear", { env });
}
