import { afterEach, expect, it, vi } from "vitest";
const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));
vi.mock("@tauri-apps/plugin-http", () => ({ fetch: fetchMock }));
import { apiClient } from "./api";
afterEach(() => { apiClient.clearCredentials(); fetchMock.mockReset(); vi.useRealTimers(); });

it("rejects a response from a previous environment", async () => {
  let finish!: (response: Response) => void;
  fetchMock.mockImplementation(() => new Promise<Response>(resolve => { finish = resolve; }));
  apiClient.setCredentials("local-key", "http://127.0.0.1:8090");
  const pending = apiClient.getHealth();
  apiClient.setCredentials("production-key", "http://127.0.0.1:8091");
  finish(new Response(JSON.stringify({ status: "ok" }), { headers: { "content-type": "application/json" } }));
  expect((await pending).error?.code).toBe("STALE_RESPONSE");
  expect(fetchMock.mock.calls[0][0]).toBe("http://127.0.0.1:8090/health");
});

it("aborts stalled requests at the deadline", async () => {
  vi.useFakeTimers();
  fetchMock.mockImplementation((_url, options) => new Promise((_resolve, reject) => {
    options.signal.addEventListener("abort", () => reject(new Error("timeout")));
  }));
  const pending = apiClient.getHealth();
  await vi.advanceTimersByTimeAsync(30000);
  expect((await pending).ok).toBe(false);
  expect(fetchMock.mock.calls[0][1].signal.aborted).toBe(true);
});
