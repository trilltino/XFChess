/**
 * Shared HTTP client and base URL resolution for XFChess backend calls.
 *
 * All feature modules under `lib/api/*` use `request()` from here so we
 * get consistent JSON headers, error handling, and base URL resolution.
 * The base URL is taken from `VITE_BACKEND_URL` at build time and falls
 * back to `http://localhost:8090` for local dev.
 */

export const BACKEND_URL: string =
  (import.meta.env.VITE_BACKEND_URL as string | undefined) ||
  'http://localhost:8090';

/**
 * JSON-aware fetch wrapper. Throws on non-2xx with the response body (or
 * a generic `Request failed: <status>` if the body can't be read).
 */
export async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const res = await fetch(`${BACKEND_URL}${path}`, {
    headers: { 'Content-Type': 'application/json', ...(init.headers || {}) },
    ...init,
  });
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(text || `Request failed: ${res.status}`);
  }
  return (await res.json()) as T;
}
