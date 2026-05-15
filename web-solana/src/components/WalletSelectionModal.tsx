/**
 * Wallet picker modal shown from the top-nav "Connect Wallet" button.
 *
 * Lists all wallet adapters registered with `@solana/wallet-adapter-react`,
 * surfaces a short tagline per wallet, and — when running inside the Tauri
 * desktop shell — disables extension-only wallets (Phantom / Solflare) and
 * promotes WalletConnect as the recommended option since only mobile/QR
 * flows work there.
 */

import { useWallet } from '@solana/wallet-adapter-react';

const isTauri =
  typeof window !== 'undefined' &&
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== undefined;

export function WalletSelectionModal({ onClose }: { onClose: () => void }) {
  const { wallets, select } = useWallet();

  const descriptions: Record<string, string> = {
    Phantom: isTauri
      ? 'Requires Chrome Extension (Browser only).'
      : 'The most popular Solana wallet with a sleek interface.',
    Solflare: isTauri
      ? 'Requires Chrome Extension (Browser only).'
      : 'A powerful, feature-rich wallet with advanced security.',
    WalletConnect: isTauri
      ? 'Recommended for Desktop App (Connect via Mobile).'
      : 'Connect to your mobile wallet via a secure bridge.',
    'Mobile Wallet Adapter': 'Native mobile connection for Android and iOS devices.',
  };

  // Sort wallets to prioritize WalletConnect in Tauri
  const sortedWallets = [...wallets].sort((a, b) => {
    if (isTauri) {
      if (a.adapter.name === 'WalletConnect') return -1;
      if (b.adapter.name === 'WalletConnect') return 1;
    }
    return 0;
  });

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
        <div className="wallet-list">
          {sortedWallets.map((wallet) => {
            const isDisabled =
              isTauri && (wallet.adapter.name === 'Phantom' || wallet.adapter.name === 'Solflare');
            const isRecommended = isTauri && wallet.adapter.name === 'WalletConnect';

            return (
              <div
                key={wallet.adapter.name}
                className={`wallet-item ${isDisabled ? 'disabled' : ''} ${
                  isRecommended ? 'recommended' : ''
                }`}
                onClick={() => {
                  if (isDisabled) return;
                  select(wallet.adapter.name);
                  onClose();
                }}
                style={{
                  opacity: isDisabled ? 0.5 : 1,
                  cursor: isDisabled ? 'not-allowed' : 'pointer',
                  border: isRecommended ? '1px solid var(--primary)' : '1px solid var(--border)',
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
                    {isRecommended && (
                      <span
                        style={{ fontSize: '0.6rem', color: 'var(--primary)', fontWeight: 800 }}
                      >
                        RECOMMENDED
                      </span>
                    )}
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
