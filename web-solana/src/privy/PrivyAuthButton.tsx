import { usePrivy } from '@privy-io/react-auth';
import { PRIVY_ENABLED } from './config';

// Inner component always calls the hook (valid) — only ever mounted inside a
// PrivyProvider, because the wrapper below refuses to render it when disabled.
function PrivyButtonInner() {
  const { ready, authenticated, user, login, logout } = usePrivy();
  if (!ready) return null;

  if (authenticated) {
    const wallet = user?.wallet?.address;
    const label = user?.email?.address || (wallet ? `${wallet.slice(0, 4)}…${wallet.slice(-4)}` : 'Account');
    return (
      <button className="nav-link" onClick={() => logout()} title={label}>
        Sign out
      </button>
    );
  }
  return (
    <button className="nav-link" onClick={() => login()}>
      Privy Login
    </button>
  );
}

/** Renders the Privy login/logout control, or nothing when Privy is not configured. */
export function PrivyAuthButton() {
  if (!PRIVY_ENABLED) return null;
  return <PrivyButtonInner />;
}

export default PrivyAuthButton;
