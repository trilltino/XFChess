/** Shared JSON client and backend URL resolution. */

export const BACKEND_URL: string =
  (import.meta.env.VITE_BACKEND_URL as string | undefined) ||
  'http://localhost:8090';

/** Fetch JSON; throw the response body on non-2xx, or Request failed: <status> if unreadable. */
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
