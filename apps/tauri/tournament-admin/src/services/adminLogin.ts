// Admin sign-in: a username and password are exchanged for a session token at the backend's
// /admin/auth/login (services/backend/src/infrastructure/admin_session.rs). The panel sends the
// token in the X-Admin-Session header. The password is never stored.

import { fetch } from "@tauri-apps/plugin-http";

const USERNAME_KEY = "admin_username";

/** The name last used to sign in, to prefill the form. Not a secret. */
export function savedUsername(): string {
  try {
    return localStorage.getItem(USERNAME_KEY) ?? "";
  } catch {
    return "";
  }
}

export function rememberUsername(name: string) {
  try {
    localStorage.setItem(USERNAME_KEY, name);
  } catch {
    // Prefilling the name is a convenience only.
  }
}

/** A plain-language reason for a refused sign-in. */
export function signInProblem(status: number, body: string): string {
  switch (status) {
    case 401:
      return "Wrong username or password.";
    case 429:
      return "Too many wrong passwords. Wait five minutes and try again.";
    case 503:
      return "Admin sign-in is not set up on this backend. Run  just admin-password <name>, put the printed ADMIN_USERS line in the backend's .env and restart it.";
    default:
      return body.trim() || `The backend refused the sign-in (HTTP ${status}).`;
  }
}

/** Signs in and returns the session token, which lasts eight hours. */
export async function passwordSessionToken(baseUrl: string, username: string, password: string): Promise<string> {
  const response = await fetch(`${baseUrl}/admin/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: username.trim(), password }),
  });
  const text = await response.text();
  if (!response.ok) throw new Error(signInProblem(response.status, text));
  let token: unknown;
  try {
    token = (JSON.parse(text) as { token?: unknown }).token;
  } catch {
    token = undefined;
  }
  if (typeof token !== "string" || !token) throw new Error("The backend did not return a session.");
  return token;
}
