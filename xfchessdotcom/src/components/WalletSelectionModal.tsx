/**
 * Wallet picker modal shown from the top-nav "Connect Wallet" button.
 *
 * Lists wallet adapters registered with `@solana/wallet-adapter-react`
 * (plus any Wallet Standard wallet a browser extension self-registers,
 * hence the explicit name filter below) and surfaces a short tagline per
 * wallet. Inside the Tauri desktop shell, extension-only wallets
 * (Phantom / Solflare) are disabled since there's no extension host there.
 */

import { useWallet } from '@solana/wallet-adapter-react';
import { SocialLoginButtons } from '../privy/SocialLoginButtons';
import { PRIVY_WALLET_NAME } from '../privy/config';

const isTauri =
  typeof window !== 'undefined' &&
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== undefined;

// Not explicitly registered as adapters — some wallet extensions
// self-register via the Wallet Standard protocol. Filtered out here rather
// than left to appear unannounced.
const HIDDEN_WALLETS = new Set(['Jupiter', 'WalletConnect']);

export function WalletSelectionModal({ onClose }: { onClose: () => void }) {
  const { wallets, select } = useWallet();

  const descriptions: Record<string, string> = {
    Phantom: isTauri
      ? 'Requires Chrome Extension (Browser only).'
      : 'The most popular Solana wallet with a sleek interface.',
    Solflare: isTauri
      ? 'Requires Chrome Extension (Browser only).'
      : 'A powerful, feature-rich wallet with advanced security.',
    'Mobile Wallet Adapter': 'Native mobile connection for Android and iOS devices.',
    [PRIVY_WALLET_NAME]: 'Signed in with Google or email — no extension needed.',
  };

  const visibleWallets = wallets.filter((w) => !HIDDEN_WALLETS.has(w.adapter.name));

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="custom-wallet-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>
            Select Network Provider{' '}
            {isTauri && (
              <span
                style={{
                  fontSize: '0.7rem',
                  opacity: 0.6,
                  background: 'var(--primary)',
                  color: '#fff',
                  padding: '2px 8px',
                  borderRadius: '10px',
                  marginLeft: '10px',
                  verticalAlign: 'middle',
                }}
              >
                DESKTOP APP
              </span>
            )}
          </h3>
          <button className="modal-close" onClick={onClose}>
            &times;
          </button>
        </div>
        <SocialLoginButtons />
        <div className="wallet-list">
          {visibleWallets.map((wallet) => {
            // Phantom/Solflare need a browser extension, which the Tauri shell
            // has no host for. Privy deliberately is NOT in this list: it is an
            // embedded wallet with no extension requirement, so it is the one
            // option that genuinely works inside the desktop shell.
            const isDisabled =
              isTauri && (wallet.adapter.name === 'Phantom' || wallet.adapter.name === 'Solflare');

            return (
              <div
                key={wallet.adapter.name}
                className={`wallet-item ${isDisabled ? 'disabled' : ''}`}
                onClick={() => {
                  if (isDisabled) return;
                  select(wallet.adapter.name);
                  onClose();
                }}
                style={{
                  opacity: isDisabled ? 0.5 : 1,
                  cursor: isDisabled ? 'not-allowed' : 'pointer',
                  border: '1px solid var(--border)',
                }}
              >
                <div className="wallet-icon-wrap">
                  <img
                    src={wallet.adapter.icon}
                    alt={wallet.adapter.name}
                    width={32}
                    height={32}
                  />
                </div>
                <div className="wallet-info">
                  <h4 style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    {wallet.adapter.name}
                  </h4>
                  <p>
                    {descriptions[wallet.adapter.name] ||
                      'Connect using your preferred Solana vault.'}
                  </p>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
