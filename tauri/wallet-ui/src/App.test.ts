import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { getConnectedProvider, resolveExistingUsername } from './App';

// getConnectedProvider: which wallet extension to sign back through. Real
// regression this guards — see the function's own doc comment in App.tsx —
// is silently signing with the wrong wallet when more than one extension is
// installed, which surfaces to the player as a confusing "not enough SOL".
describe('getConnectedProvider', () => {
  const solflare = { name: 'solflare-provider' };
  const phantom = { name: 'phantom-provider' };

  beforeEach(() => {
    localStorage.clear();
    (window as any).solflare = undefined;
    (window as any).phantom = undefined;
  });

  it('returns solflare when that was the connected provider', () => {
    localStorage.setItem('xfchess_wallet_provider', 'solflare');
    (window as any).solflare = solflare;
    (window as any).phantom = { solana: phantom };
    expect(getConnectedProvider()).toBe(solflare);
  });

  it('returns phantom when that was the connected provider, even if solflare is also installed', () => {
    localStorage.setItem('xfchess_wallet_provider', 'phantom');
    (window as any).solflare = solflare;
    (window as any).phantom = { solana: phantom };
    expect(getConnectedProvider()).toBe(phantom);
  });

  it('falls back to phantom-then-solflare when no provider was ever recorded', () => {
    (window as any).solflare = solflare;
    (window as any).phantom = { solana: phantom };
    expect(getConnectedProvider()).toBe(phantom);

    (window as any).phantom = undefined;
    expect(getConnectedProvider()).toBe(solflare);
  });
});

// resolveExistingUsername: a wallet can have a real display name recorded
// two different ways (on-chain username_set, or the off-chain /auth/me
// username) — and register seeds a `pubkey.slice(0, 8)` placeholder into the
// off-chain field that must never be mistaken for a real name.
describe('resolveExistingUsername', () => {
  const pubkey = 'AbCdEfGh1234567890';

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('prefers the on-chain username when username_set is true', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);

    const result = await resolveExistingUsername('token', pubkey, {
      has_profile: true,
      username_set: true,
      is_verified: false,
      username: 'OnChainName',
    });

    expect(result).toBe('OnChainName');
    expect(fetchMock).not.toHaveBeenCalled(); // no need to hit /auth/me at all
  });

  it('falls back to the off-chain username when it differs from the registration placeholder', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ username: 'RealChosenName' }),
      })
    );

    const result = await resolveExistingUsername('token', pubkey, {
      has_profile: false,
      username_set: false,
      is_verified: false,
      username: null,
    });

    expect(result).toBe('RealChosenName');
  });

  it('treats the registration placeholder as "no real username yet"', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ username: pubkey.slice(0, 8) }),
      })
    );

    const result = await resolveExistingUsername('token', pubkey, {
      has_profile: false,
      username_set: false,
      is_verified: false,
      username: null,
    });

    expect(result).toBeNull();
  });

  it('returns null when /auth/me is unreachable rather than throwing', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500 }));

    const result = await resolveExistingUsername('token', pubkey, {
      has_profile: false,
      username_set: false,
      is_verified: false,
      username: null,
    });

    expect(result).toBeNull();
  });
});
