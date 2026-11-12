import { afterEach, expect, it, vi } from "vitest";
import { startPolling } from "./poll";
afterEach(() => vi.useRealTimers());

it("does not overlap slow work and suppresses updates after cleanup", async () => {
  vi.useFakeTimers();
  let finish!: () => void;
  let active!: () => boolean;
  const work = vi.fn(async check => { active = check; await new Promise<void>(resolve => { finish = resolve; }); });
  const stop = startPolling(work, 100);
  await vi.advanceTimersByTimeAsync(1000);
  expect(work).toHaveBeenCalledTimes(1);
  stop(); finish();
  await vi.advanceTimersByTimeAsync(1000);
  expect(active()).toBe(false);
  expect(work).toHaveBeenCalledTimes(1);
});

it("continues polling after a failed round", async () => {
  vi.useFakeTimers();
  const work = vi.fn().mockRejectedValue(new Error("offline"));
  const stop = startPolling(work, 100);
  await vi.advanceTimersByTimeAsync(300);
  expect(work).toHaveBeenCalledTimes(4);
  stop();
});
