/**
 * Wallet Standard registration for Privy's embedded wallet.
 *
 * This is the hinge the whole website integration turns on. `@solana/wallet-
 * adapter-react` (via `@solana/wallet-standard-wallet-adapter-react`, already in
 * the dep tree) listens for the `wallet-standard:register-wallet` event and
 * wraps whatever it receives in a `StandardWalletAdapter`. So once Privy's
 * embedded wallet is dispatched through here, it appears in `useWallet().wallets`
 * alongside Phantom and Solflare — and every existing `useWallet()` consumer
 * keeps working with no change at all.
 *
 * The reason this works across the `@solana/kit` (what Privy speaks) vs
 * `@solana/web3.js` (what this app speaks) split is that the Wallet Standard
 * `solana:signTransaction` feature is defined over raw `Uint8Array` in and out.
 * No typed transaction object ever crosses the boundary, so the two libraries
 * never have to agree on a representation.
 *
 * Implementation is the documented Wallet Standard pattern: dispatch the event
 * for apps that are already listening, AND register a listener for
 * `wallet-standard:app-ready` for apps that mount afterwards. Both are needed —
 * which of the two fires depends purely on mount ordering between this bridge
 * and `WalletProvider`, and that ordering is not guaranteed.
 */
import type {
  Wallet,
  WalletEventsWindow,
  WindowRegisterWalletEvent,
  WindowRegisterWalletEventCallback,
} from '@wallet-standard/base';

class RegisterWalletEvent
  extends CustomEvent<WindowRegisterWalletEventCallback>
  implements WindowRegisterWalletEvent
{
  readonly #detail: WindowRegisterWalletEventCallback;

  get detail() {
    return this.#detail;
  }

  get type() {
    return 'wallet-standard:register-wallet' as const;
  }

  constructor(callback: WindowRegisterWalletEventCallback) {
    super('wallet-standard:register-wallet', {
      bubbles: false,
      cancelable: false,
      detail: callback,
    });
    this.#detail = callback;
  }

  preventDefault(): never {
    throw new Error('preventDefault is not supported');
  }

  stopPropagation(): never {
    throw new Error('stopPropagation is not supported');
  }

  stopImmediatePropagation(): never {
    throw new Error('stopImmediatePropagation is not supported');
  }
}

export function registerWallet(wallet: Wallet): void {
  const callback: WindowRegisterWalletEventCallback = ({ register }) => register(wallet);

  // Path 1: an app (wallet-adapter) is already listening — it receives the
  // wallet immediately.
  try {
    (window as WalletEventsWindow).dispatchEvent(new RegisterWalletEvent(callback));
  } catch (error) {
    console.error('[privy] wallet-standard:register-wallet could not be dispatched\n', error);
  }

  // Path 2: the app mounts later and announces itself — hand it the wallet then.
  try {
    (window as WalletEventsWindow).addEventListener(
      'wallet-standard:app-ready',
      ({ detail: api }) => callback(api)
    );
  } catch (error) {
    console.error('[privy] wallet-standard:app-ready listener could not be added\n', error);
  }
}
