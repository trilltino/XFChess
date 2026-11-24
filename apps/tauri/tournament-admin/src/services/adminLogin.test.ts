import { afterEach, expect, it, vi } from "vitest";
const { fetchMock } = vi.hoisted(() => ({ fetchMock: vi.fn() }));
vi.mock("@tauri-apps/plugin-http", () => ({ fetch: fetchMock }));
import { passwordSessionToken } from "./adminLogin";
afterEach(() => { fetchMock.mockReset(); });

it("posts the name and password to /admin/auth/login and returns the session", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ token: "session", actor: "tino", expires_at: 1 })));
  await expect(passwordSessionToken("http://127.0.0.1:8090", " tino ", "pw")).resolves.toBe("session");
  const [url, options] = fetchMock.mock.calls[0];
  expect(url).toBe("http://127.0.0.1:8090/admin/auth/login");
  expect(options.method).toBe("POST");
  expect(JSON.parse(options.body)).toEqual({ username: "tino", password: "pw" });
});

it("explains a refusal in plain words", async () => {
  fetchMock.mockResolvedValue(new Response("wrong username or password", { status: 401 }));
  await expect(passwordSessionToken("http://127.0.0.1:8090", "tino", "nope")).rejects.toThrow("Wrong username or password.");
  fetchMock.mockResolvedValue(new Response("", { status: 429 }));
  await expect(passwordSessionToken("http://127.0.0.1:8090", "tino", "nope")).rejects.toThrow("Wait five minutes");
  fetchMock.mockResolvedValue(new Response("", { status: 503 }));
  await expect(passwordSessionToken("http://127.0.0.1:8090", "tino", "pw")).rejects.toThrow("ADMIN_USERS");
});

it("refuses a reply without a session", async () => {
  fetchMock.mockResolvedValue(new Response("{}"));
  await expect(passwordSessionToken("http://127.0.0.1:8090", "tino", "pw")).rejects.toThrow("did not return a session");
});
