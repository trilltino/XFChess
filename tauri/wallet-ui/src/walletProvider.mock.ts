// Fake injected providers for connect/sign tests: resolve, reject, hang,
// or disconnect after connection.
export type MockMode = "resolve" | "reject" | "hang";

export interface MockWalletOptions {
  connect?: MockMode;
  signMessage?: MockMode;
  signTransaction?: MockMode;
  pubkey?: string;
}

export function createMockWalletProvider(opts: MockWalletOptions = {}) {
  const {
    connect = "resolve",
    signMessage = "resolve",
    signTransaction = "resolve",
    pubkey = "MockPubkey11111111111111111111111111111",
  } = opts;

  let connected = false;

  function behave<T>(mode: MockMode, value: () => T, rejection: unknown): Promise<T> {
    if (mode === "resolve") return Promise.resolve(value());
    if (mode === "reject") return Promise.reject(rejection);
    // "hang" — the broken-relay case: a promise that never settles.
    return new Promise<T>(() => {});
  }

  return {
    get publicKey() {
      return connected ? { toBase58: () => pubkey, toString: () => pubkey } : null;
    },
    connect: (_args?: { onlyIfTrusted?: boolean }) =>
      behave(
        connect,
        () => {
          connected = true;
          return { publicKey: { toBase58: () => pubkey, toString: () => pubkey } };
        },
        new Error("User rejected the request."),
      ),
    signMessage: (_bytes: Uint8Array) =>
      behave(
        signMessage,
        () => ({ signature: new Uint8Array(64) }),
        new Error("User rejected the request."),
      ),
    signTransaction: (tx: unknown) =>
      behave(signTransaction, () => tx, new Error("User rejected the request.")),
    // Simulates the relay dying after a successful connect — e.g. the MV3
    // service worker was torn down between connect() and the next call.
    simulateDisconnectMidFlow() {
      connected = false;
    },
  };
}
