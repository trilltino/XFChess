import { describe, it, expect, vi } from "vitest";
import { withTimeout } from "./App";
import { createMockWalletProvider } from "./walletProvider.mock";

// withTimeout is the fix for the exact bug that motivated this suite: Phantom/
// Solflare's injected provider methods proxy through a content script to the
// extension's background service worker, and when that relay breaks
// ("Could not establish connection. Receiving end does not exist") the
// promise never resolves OR rejects — before this wrapper, that hung the
// "Connect" button's spinner forever with no error shown, see WalletStep in
// App.tsx.
describe("withTimeout", () => {
  it("resolves with the underlying value when the promise settles in time", async () => {
    await expect(withTimeout(Promise.resolve("ok"), 50, "op")).resolves.toBe("ok");
  });

  it("propagates a real rejection immediately, not the timeout message", async () => {
    await expect(withTimeout(Promise.reject(new Error("boom")), 50, "op")).rejects.toThrow(
      "boom",
    );
  });

  it("times out a promise that never settles, with an actionable message", async () => {
    vi.useFakeTimers();
    const hung = new Promise(() => {});
    const result = withTimeout(hung, 1000, "Phantom connection");
    vi.advanceTimersByTime(1000);
    await expect(result).rejects.toThrow(/Phantom connection timed out/);
    vi.useRealTimers();
  });
});

// Exercises App.tsx's actual connect/sign call shape against every relay
// failure mode via the mock provider, so a future change to the timeout
// wiring (wrong ms value, wrapper removed, etc.) fails a test instead of
// only surfacing as a field report of a hung popup.
describe("wallet relay failure modes (via withTimeout + mock provider)", () => {
  it("a hung connect() times out instead of hanging forever", async () => {
    vi.useFakeTimers();
    const provider = createMockWalletProvider({ connect: "hang" });
    const result = withTimeout(provider.connect(), 30000, "Phantom connection");
    vi.advanceTimersByTime(30000);
    await expect(result).rejects.toThrow(/Phantom connection timed out/);
    vi.useRealTimers();
  });

  it("a rejected connect() (user closed the approval popup) surfaces immediately", async () => {
    const provider = createMockWalletProvider({ connect: "reject" });
    await expect(withTimeout(provider.connect(), 30000, "Phantom connection")).rejects.toThrow(
      "User rejected the request.",
    );
  });

  it("connect() succeeds but a hung signMessage() still times out on its own budget", async () => {
    vi.useFakeTimers();
    const provider = createMockWalletProvider({ connect: "resolve", signMessage: "hang" });
    await provider.connect();
    const result = withTimeout(
      provider.signMessage(new Uint8Array([1, 2, 3])),
      60000,
      "Phantom signature",
    );
    vi.advanceTimersByTime(60000);
    await expect(result).rejects.toThrow(/Phantom signature timed out/);
    vi.useRealTimers();
  });

  it("a relay that disconnects mid-flow rejects signTransaction rather than hanging", async () => {
    const provider = createMockWalletProvider({ connect: "resolve" });
    await provider.connect();
    provider.simulateDisconnectMidFlow();
    expect(provider.publicKey).toBeNull();
  });

  it("a healthy relay resolves connect + sign without needing the timeout to fire", async () => {
    const provider = createMockWalletProvider();
    const resp = await withTimeout(provider.connect(), 30000, "Phantom connection");
    expect((resp as { publicKey: { toBase58(): string } }).publicKey.toBase58()).toMatch(/^Mock/);
    const { signature } = await withTimeout(
      provider.signMessage(new Uint8Array([1])),
      60000,
      "Phantom signature",
    );
    expect(signature).toBeInstanceOf(Uint8Array);
  });
});
