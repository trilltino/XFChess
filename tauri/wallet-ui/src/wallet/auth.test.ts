import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { authenticateWallet } from "./auth";
import type { WalletSource } from "./types";

const pubkey = "WalletForAuthTest";
const source = () => ({ kind: "phantom", pubkey, provider: null, signRaw: vi.fn().mockResolvedValue("test-signature") }) as unknown as WalletSource;
const response = (value: unknown, status = 200) => ({ ok: status < 400, status,
  headers: { get: () => "application/json" }, json: async () => value, text: async () => JSON.stringify(value) });
beforeEach(() => localStorage.clear());
afterEach(() => vi.unstubAllGlobals());

it("does not request a signature when the backend is unavailable", async () => {
  const wallet = source();
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(response({}, 503)));
  await expect(authenticateWallet(wallet)).rejects.toThrow(/Could not check/);
  expect(wallet.signRaw).not.toHaveBeenCalled();
});

it("hands the JWT to the bridge before publishing the connected wallet", async () => {
  const calls: string[] = [];
  vi.stubGlobal("fetch", vi.fn(async (url: string) => {
    calls.push(url);
    return response(url.includes("/auth/login") ? { token: "test-jwt", username: "Tester", wallet: pubkey } : {});
  }));
  const wallet = source();
  expect((await authenticateWallet(wallet)).token).toBe("test-jwt");
  expect(calls.map(url => new URL(url).pathname)).toEqual([`/api/auth/check-wallet/${pubkey}`, "/api/auth/login", "/token", "/wallet"]);
  expect(wallet.signRaw).toHaveBeenCalledWith(expect.stringMatching(/^xfchess:login:\d+$/));
  expect(localStorage.getItem("xfchess_token")).toBeNull();
});

it("registers only when the backend reports a missing wallet", async () => {
  vi.stubGlobal("fetch", vi.fn(async (url: string) => response(url.includes("/auth/register")
    ? { token: "test-jwt", username: "Tester", wallet: pubkey } : {}, url.includes("check-wallet") ? 404 : 200)));
  const wallet = source();
  await authenticateWallet(wallet);
  expect(wallet.signRaw).toHaveBeenCalledWith(expect.stringMatching(/^xfchess:register:\d+$/));
});

it("rejects a backend response for a different wallet", async () => {
  const fetch = vi.fn(async (url: string) => response(url.includes("/auth/login") ? { token: "test-jwt", wallet: "OtherWallet" } : {}));
  vi.stubGlobal("fetch", fetch);
  await expect(authenticateWallet(source())).rejects.toThrow(/different wallet/);
  expect(fetch).toHaveBeenCalledTimes(2);
});

it("does not publish a connected wallet when token handoff fails", async () => {
  const fetch = vi.fn(async (url: string) => response(url.includes("/auth/login") ? { token: "test-jwt", wallet: pubkey } : {}, url.endsWith("/token") ? 500 : 200));
  vi.stubGlobal("fetch", fetch);
  await expect(authenticateWallet(source())).rejects.toThrow();
  expect(fetch).toHaveBeenCalledTimes(3);
});
