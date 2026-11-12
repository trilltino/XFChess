import { useState, useEffect, useRef, createContext, useContext, ReactNode } from "react";
import { apiClient } from "../services/api";
import type { AdminAuthState } from "../types/tournament";
import { envById, getOpsConfig, opsConfigProblem, type EnvId } from "../config/environments";
import { ensureTunnel, killTunnel } from "../services/tunnel";
import { loadAdminToken, saveAdminToken } from "../services/credentials";
import { rememberKeypairPath, savedKeypairPath, walletPopupSessionToken, walletSessionToken } from "../services/walletLogin";

interface AuthContextType {
  authState: AdminAuthState;
  /** Log in against a chosen environment; opens the SSH tunnel for PRODUCTION. */
  login: (token: string, env: EnvId) => Promise<boolean>;
  /**
   * Sign in with an admin wallet; the session lasts hours. With a keypair file path the file signs
   * the challenge silently; with null the connected wallet signs it through the wallet window.
   */
  loginWithWallet: (keypairPath: string | null, env: EnvId) => Promise<boolean>;
  logout: () => void;
  loading: boolean;
  /** The specific failure from the last login attempt, including tunnel and ACL errors. */
  lastError: string | null;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

// Non-secret session hints. The key itself is in the OS credential store.
const LAST_ENV_KEY = "admin_last_env";
// Explicit logout disables auto-resume until the next successful login.
const SIGNED_OUT_KEY = "admin_signed_out";

const LOCAL_URL = envById("local").backendUrl;

export function AuthProvider({ children }: { children: ReactNode }) {
  const [authState, setAuthState] = useState<AdminAuthState>({
    token: null,
    authenticated: false,
    backend_url: LOCAL_URL,
    env: "local",
  });
  const [loading, setLoading] = useState(true);
  const [lastError, setLastError] = useState<string | null>(null);
  const generation = useRef(0);

  // Resume the saved session on mount; any failure just leaves the login screen.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const lastEnv = localStorage.getItem(LAST_ENV_KEY);
      const env: EnvId | null = lastEnv === "local" || lastEnv === "production" ? lastEnv : null;
      if (env && localStorage.getItem(SIGNED_OUT_KEY) !== "1") {
        try {
          // A remembered wallet signs in again with no input; otherwise use the saved key.
          const walletPath = savedKeypairPath();
          const resumed = walletPath ? await loginWithWallet(walletPath, env) : false;
          if (!resumed) {
            const savedToken = await loadAdminToken(env);
            if (savedToken) await login(savedToken, env);
          }
        } catch {
          // Fall through to the login screen.
        }
      }
      if (!cancelled) setLoading(false);
    })();
    return () => {
      cancelled = true;
      generation.current++;
      apiClient.clearCredentials();
      void killTunnel();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const fail = async (reason: string, isProduction: boolean) => {
    setLastError(reason);
    apiClient.clearCredentials();
    if (isProduction) await killTunnel();
    return false;
  };

  /**
   * The shared sign-in flow: open or close the tunnel for the environment, obtain a credential,
   * and keep it only if the backend accepts a read-only probe with it.
   */
  const connect = async (
    env: EnvId,
    credential: (backendUrl: string) => Promise<{ token: string; remember: boolean }>,
  ): Promise<boolean> => {
    const operation = ++generation.current;
    const current = () => operation === generation.current;
    const cfg = envById(env);
    setLastError(null);
    try {
      if (cfg.isProduction) {
        const problem = opsConfigProblem(getOpsConfig());
        if (problem) return fail(problem, false);
        // ensureTunnel throws if it can't reach the backend through the forward.
        await ensureTunnel(cfg);
      } else {
        await killTunnel();
      }

      if (!current()) return false;
      const { token, remember } = await credential(cfg.backendUrl);
      if (!current()) return false;
      apiClient.setCredentials(token, cfg.backendUrl);
      // One cheap, read-only probe that exercises admin auth.
      const response = await apiClient.getAuditLog(1);
      if (!current()) return false;
      if (!response.ok) {
        const reason = `Backend rejected the request: HTTP ${response.error?.status ?? "?"} ${response.error?.message ?? ""}`.trim();
        console.error("[auth] login probe failed:", reason, response.error);
        return fail(reason, cfg.isProduction);
      }
      // Only an API key the backend accepted is remembered; a wallet session is short-lived and
      // is simply signed again next time.
      if (remember) await saveAdminToken(env, token);
      if (!current()) return false;
      localStorage.setItem(LAST_ENV_KEY, env);
      localStorage.removeItem(SIGNED_OUT_KEY);
      setAuthState({ token, authenticated: true, backend_url: cfg.backendUrl, env });
      return true;
    } catch (err) {
      if (!current()) return false;
      console.error("[auth] login failed before reaching the backend:", err);
      return fail(err instanceof Error ? err.message : String(err), cfg.isProduction);
    }
  };

  const login = (token: string, env: EnvId): Promise<boolean> =>
    connect(env, async () => ({ token, remember: true }));

  const loginWithWallet = async (keypairPath: string | null, env: EnvId): Promise<boolean> => {
    const ok = await connect(env, async (backendUrl) => ({
      token: keypairPath
        ? await walletSessionToken(backendUrl, keypairPath)
        : await walletPopupSessionToken(backendUrl),
      remember: false,
    }));
    if (ok && keypairPath) rememberKeypairPath(keypairPath.trim());
    return ok;
  };

  const logout = () => {
    generation.current++;
    localStorage.setItem(SIGNED_OUT_KEY, "1");
    apiClient.clearCredentials();
    void killTunnel();
    setAuthState({ token: null, authenticated: false, backend_url: LOCAL_URL, env: "local" });
  };

  const contextValue = { authState, login, loginWithWallet, logout, loading, lastError };

  return <AuthContext.Provider value={contextValue}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
