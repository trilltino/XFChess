import { afterEach, expect, it, vi } from "vitest";
import { trackWalletPopups } from "./popups";

afterEach(() => vi.restoreAllMocks());

it("closes only child windows opened by this wallet page", () => {
  const child = { close: vi.fn() } as unknown as Window;
  const unrelated = { close: vi.fn() };
  const original = vi.spyOn(window, "open").mockReturnValue(child);
  const dispose = trackWalletPopups();
  expect(window.open).not.toBe(original);
  expect(window.open("https://auth.privy.io/")).toBe(child);
  window.dispatchEvent(new Event("pagehide"));
  expect(child.close).toHaveBeenCalledOnce();
  expect(unrelated.close).not.toHaveBeenCalled();
  dispose();
  expect(window.open).toBe(original);
  expect(child.close).toHaveBeenCalledOnce();
});

it("preserves blocked and noopener popup behavior", () => {
  vi.spyOn(window, "open").mockReturnValue(null);
  const dispose = trackWalletPopups();
  expect(window.open("https://auth.privy.io/", "_blank", "noopener")).toBeNull();
  expect(dispose).not.toThrow();
});
