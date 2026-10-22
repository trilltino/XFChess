import { afterEach, expect, it, vi } from "vitest";
const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));
vi.mock("@tauri-apps/plugin-http", () => ({ fetch: fetchMock }));
import { apiClient } from "./api";
afterEach(() => { apiClient.clearCredentials(); fetchMock.mockReset(); vi.useRealTimers(); });

it("rejects unsafe monetary values before making a request", async () => {
  for (const amount of [Number.NaN, Infinity, -1, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
    expect((await apiClient.fundTournamentPrize(1, amount)).error?.code).toBe("INVALID_AMOUNT");
  }
  expect(fetchMock).not.toHaveBeenCalled();
});

it("retains working credentials when a generated replacement is not installed", async () => {
  apiClient.setCredentials("working", "http://127.0.0.1:8090");
  fetchMock.mockResolvedValue(new Response("Unauthorized", { status: 401 }));
  const save = vi.fn();
  await expect(apiClient.activateAdminToken("candidate", save)).rejects.toThrow();
  expect(save).not.toHaveBeenCalled();
  expect(fetchMock.mock.calls[0][0]).toContain("/admin/audit-log");
  expect(fetchMock.mock.calls[0][1].headers.get("X-API-Key")).toBe("candidate");
  fetchMock.mockResolvedValue(new Response("{}", { headers: { "content-type": "application/json" } }));
  await apiClient.getAuditLog(1);
  expect(fetchMock.mock.lastCall?.[1].headers.get("X-API-Key")).toBe("working");
});

it("saves and activates a replacement only after protected verification", async () => {
  apiClient.setCredentials("working", "http://127.0.0.1:8090");
  fetchMock.mockImplementation(async () => new Response("{}", { headers: { "content-type": "application/json" } }));
  const save = vi.fn(async () => {});
  await apiClient.activateAdminToken("candidate", save);
  expect(save).toHaveBeenCalledOnce();
  await apiClient.getAuditLog(1);
  expect(fetchMock.mock.lastCall?.[1].headers.get("X-API-Key")).toBe("candidate");
});

it("does not save or activate a candidate after changing environments", async () => {
  let finish!: (response: Response) => void;
  fetchMock.mockImplementation(() => new Promise<Response>(resolve => { finish = resolve; }));
  apiClient.setCredentials("working", "http://127.0.0.1:8090");
  const save = vi.fn();
  const pending = apiClient.activateAdminToken("candidate", save);
  apiClient.setCredentials("production", "http://127.0.0.1:8091");
  finish(new Response("{}", { headers: { "content-type": "application/json" } }));
  await expect(pending).rejects.toThrow("Connection changed");
  expect(save).not.toHaveBeenCalled();
});

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
