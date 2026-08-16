import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import * as web3 from '@solana/web3.js';
import {
  getConnectedProvider,
  isNetworkMismatchError,
  isStaleBlockhashError,
  refreshBlockhash,
  resolveExistingUsername,
} from './App';

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

// isNetworkMismatchError: distinguishes the live-repro Solflare failure
// ("current network devnet, but this transaction is for mainnet") from an
// ordinary rejection/timeout, so it can be turned into an actionable message
// instead of the wallet's own confusing raw text — see App.tsx's doc comment
// above the function for the fuller context (ensureDevnet is unverifiable
// best-effort, this is the reactive fallback).
describe('isNetworkMismatchError', () => {
  it('recognizes the exact live-repro Solflare message', () => {
    expect(
      isNetworkMismatchError(new Error('current network devnet, but this transaction is for mainnet')),
    ).toBe(true);
  });

  it('is case-insensitive', () => {
    expect(
      isNetworkMismatchError(new Error('Current Network DEVNET, but this transaction is for MAINNET')),
    ).toBe(true);
  });

  it('recognizes a generic cluster-mismatch phrasing', () => {
    expect(isNetworkMismatchError(new Error('wrong network: expected devnet cluster mismatch'))).toBe(
      true,
    );
  });

  it('does not misclassify an ordinary user rejection', () => {
    expect(isNetworkMismatchError(new Error('User rejected the request'))).toBe(false);
  });

  it('does not misclassify a plain timeout', () => {
    expect(isNetworkMismatchError(new Error('Wallet signature timed out'))).toBe(false);
  });

  it('does not misclassify an unrelated network-flavored message with no cluster name', () => {
    expect(isNetworkMismatchError(new Error('network request failed'))).toBe(false);
  });

  it('handles a raw string instead of an Error object', () => {
    expect(isNetworkMismatchError('devnet mainnet network mismatch')).toBe(true);
  });

  it('handles null/undefined without throwing', () => {
    expect(isNetworkMismatchError(null)).toBe(false);
    expect(isNetworkMismatchError(undefined)).toBe(false);
  });
});

// refreshBlockhash: closes the live-repro bug where a blockhash baked in at
// tx-build time went stale by the time a real human finished clicking
// through the wallet extension's approval popup, and broadcast-tx 502'd
// with "Blockhash not found" even though signing had already succeeded.
describe('refreshBlockhash', () => {
  const FRESH = '9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin';

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('overwrites recentBlockhash on a legacy Transaction', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ blockhash: FRESH, lastValidBlockHeight: 12345 }),
      }),
    );

    const tx = new web3.Transaction();
    tx.recentBlockhash = 'stale-blockhash-from-tx-build-time';

    await refreshBlockhash(tx);

    expect(tx.recentBlockhash).toBe(FRESH);
  });

  it('overwrites message.recentBlockhash on a VersionedTransaction', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ blockhash: FRESH, lastValidBlockHeight: 12345 }),
      }),
    );

    // Minimal VersionedTransaction-shaped object — enough for the
    // `instanceof` branch and the one field this function touches, without
    // needing a fully compiled transaction message.
    const tx = Object.create(web3.VersionedTransaction.prototype) as web3.VersionedTransaction;
    (tx as any).message = { recentBlockhash: 'stale-blockhash-from-tx-build-time' };

    await refreshBlockhash(tx);

    expect(tx.message.recentBlockhash).toBe(FRESH);
  });

  it('leaves the transaction untouched if the fetch fails (best-effort)', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('bridge unreachable')));

    const tx = new web3.Transaction();
    tx.recentBlockhash = 'original-blockhash';

    await refreshBlockhash(tx);

    expect(tx.recentBlockhash).toBe('original-blockhash');
  });

  it('leaves the transaction untouched on a non-ok HTTP response', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 502 }));

    const tx = new web3.Transaction();
    tx.recentBlockhash = 'original-blockhash';

    await refreshBlockhash(tx);

    expect(tx.recentBlockhash).toBe('original-blockhash');
  });
});

// isStaleBlockhashError: the specific rejection shape a stale/expired
// blockhash produces on broadcast (RPC -32002, "Blockhash not found") —
// distinguishes "safe to retry with a fresh signature" from a real
// rejection (insufficient funds, program error) that must not be retried.
describe('isStaleBlockhashError', () => {
  it('recognizes the exact live-repro broadcast error', () => {
    expect(
      isStaleBlockhashError(
        new Error(
          'RPC broadcast: RPC response error -32002: Transaction simulation failed: Blockhash not found;',
        ),
      ),
    ).toBe(true);
  });

  it('is case-insensitive', () => {
    expect(isStaleBlockhashError(new Error('BLOCKHASH NOT FOUND'))).toBe(true);
  });

  it('recognizes the bare RPC error code', () => {
    expect(isStaleBlockhashError(new Error('some wrapper text -32002 more text'))).toBe(true);
  });

  it('does not misclassify an unrelated broadcast failure', () => {
    expect(isStaleBlockhashError(new Error('insufficient funds for rent'))).toBe(false);
  });

  it('does not misclassify a program error', () => {
    expect(isStaleBlockhashError(new Error('custom program error: 0x1771'))).toBe(false);
  });

  it('handles null/undefined without throwing', () => {
    expect(isStaleBlockhashError(null)).toBe(false);
    expect(isStaleBlockhashError(undefined)).toBe(false);
  });
});
