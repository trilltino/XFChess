/**
 * "Continue with Google / Email" block shown at the top of WalletSelectionModal.
 *
 * These do not connect a wallet directly. They authenticate the user with Privy,
 * which creates a Solana embedded wallet (`createOnLogin: 'users-without-wallets'`)
 * and exposes it as a Wallet Standard wallet. PrivyStandardBridge then registers
 * it, at which point it appears in the wallet list below like any other wallet and
 * the normal `select()` path takes over.
 *
 * The two-step nature is deliberate and visible to the user: after login the
 * "Privy" row lights up in the list, and selecting it runs the same
 * check-wallet → login/register signature flow every other wallet runs. There is
 * no privileged shortcut for social users — see the plan's D2.
 */
import { useEffect, useState } from 'react';
import { PRIVY_ENABLED } from './config';
import { loadPrivy, usePrivyRuntime, type PrivyRuntime } from './privyRuntime';

function GoogleMark() {
  // Inline so it works offline and needs no asset pipeline entry.
  return (
    <svg width="20" height="20" viewBox="0 0 48 48" aria-hidden="true">
      <path
        fill="#FFC107"
        d="M43.6 20.5h-1.9V20H24v8h11.3c-1.6 4.7-6.1 8-11.3 8-6.6 0-12-5.4-12-12s5.4-12 12-12c3.1 0 5.9 1.2 8 3.1l5.7-5.7C34.0 6.1 29.3 4 24 4 13 4 4 13 4 24s9 20 20 20 20-9 20-20c0-1.3-.1-2.4-.4-3.5z"
      />
      <path
        fill="#FF3D00"
        d="M6.3 14.7l6.6 4.8C14.7 15.1 19 12 24 12c3.1 0 5.9 1.2 8 3.1l5.7-5.7C34.0 6.1 29.3 4 24 4 16.3 4 9.7 8.3 6.3 14.7z"
      />
      <path
        fill="#4CAF50"
        d="M24 44c5.2 0 9.9-2 13.4-5.2l-6.2-5.2C29.2 35.1 26.7 36 24 36c-5.2 0-9.6-3.3-11.3-7.9l-6.5 5C9.5 39.6 16.2 44 24 44z"
      />
      <path
        fill="#1976D2"
        d="M43.6 20.5H24v8h11.3c-.8 2.2-2.2 4.1-4.1 5.5l6.2 5.2C41.2 36.3 44 30.7 44 24c0-1.3-.1-2.4-.4-3.5z"
      />
    </svg>
  );
}

function SocialLoginButtonsInner({ runtime }: { runtime: PrivyRuntime }) {
  const [error, setError] = useState<string | null>(null);
  const { ready, authenticated } = runtime.core.usePrivy();
  const { login } = runtime.core.useLogin({
    onError: (code) => setError(`Sign-in failed (${code}). Please try again.`),
  });

  const start = (method: 'google' | 'email') => {
    setError(null);
    try {
      login({ loginMethods: [method] });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const btn: React.CSSProperties = {
    width: '100%',
    display: 'flex',
    alignItems: 'center',
    gap: 12,
    padding: '12px 16px',
    borderRadius: 10,
    border: '1px solid var(--border)',
    background: 'rgba(255,255,255,0.03)',
    color: 'inherit',
    font: 'inherit',
    fontWeight: 600,
    cursor: ready ? 'pointer' : 'wait',
    opacity: ready ? 1 : 0.6,
    marginBottom: 8,
  };

  return (
    <div style={{ padding: '0 4px 4px' }}>
      {authenticated ? (
        <p style={{ fontSize: '0.85rem', opacity: 0.75, margin: '0 0 12px' }}>
          Signed in. Select <strong>Privy</strong> below to connect your wallet.
        </p>
      ) : (
        <>
          <button type="button" style={btn} disabled={!ready} onClick={() => start('google')}>
            <GoogleMark />
            <span>Continue with Google</span>
          </button>
        </>
      )}

      {error && (
        <p style={{ fontSize: '0.8rem', color: 'var(--danger, #ff6b6b)', margin: '4px 0 0' }}>
          {error}
        </p>
      )}

      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          margin: '14px 0 6px',
          opacity: 0.5,
          fontSize: '0.75rem',
          textTransform: 'uppercase',
          letterSpacing: '0.08em',
        }}
      >
        <span style={{ flex: 1, height: 1, background: 'var(--border)' }} />
        <span>or use a wallet</span>
        <span style={{ flex: 1, height: 1, background: 'var(--border)' }} />
      </div>
    </div>
  );
}

/**
 * Gate around the hook-using inner component. The SDK is normally already
 * loaded by the time anyone opens this modal (privyRuntime kicks the load off
 * on app mount), but if the user is fast — or that background load failed and
 * reset itself for retry — opening the modal requests it again rather than
 * leaving a dead control.
 */
export function SocialLoginButtons() {
  const runtime = usePrivyRuntime();

  useEffect(() => {
    if (PRIVY_ENABLED && !runtime) void loadPrivy();
  }, [runtime]);

  if (!PRIVY_ENABLED) return null;

  if (!runtime) {
    return (
      <div style={{ padding: '0 4px 12px', fontSize: '0.85rem', opacity: 0.6 }}>
        Loading sign-in options…
      </div>
    );
  }

  return <SocialLoginButtonsInner runtime={runtime} />;
}

export default SocialLoginButtons;
