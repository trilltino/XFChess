import { useState, useEffect, createContext, useContext, ReactNode } from "react";
import { apiClient } from "../services/api";
import type { AdminAuthState } from "../types/tournament";
import { ENVIRONMENTS, envById, type EnvId } from "../config/environments";
import { ensureTunnel, killTunnel } from "../services/tunnel";

interface AuthContextType {
  authState: AdminAuthState;
  /** Log in against a chosen environment; opens the SSH tunnel for PRODUCTION. */
  login: (token: string, env: EnvId) => Promise<boolean>;
  logout: () => void;
  loading: boolean;
  /** The specific failure from the last login attempt, including tunnel and ACL errors. */
  lastError: string | null;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

// Per-environment token storage so a LOCAL token is never replayed at PRODUCTION.
const tokenKey = (env: EnvId) => `admin_token_${env}`;

// Explicit logout disables auto-resume until successful login.
const SIGNED_OUT_KEY = "admin_signed_out";

export function AuthProvider({ children }: { children: ReactNode }) {
  const [authState, setAuthState] = useState<AdminAuthState>({
    token: null,
    authenticated: false,
    backend_url: ENVIRONMENTS.local.backendUrl,
    env: "local",
  });
  const [loading, setLoading] = useState(true);
  const [lastError, setLastError] = useState<string | null>(null);

  // Resume the saved session on mount using the idempotent Rust tunnel API.
  // Failure silently leaves the login screen available.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const signedOut = localStorage.getItem(SIGNED_OUT_KEY) === "1";
      const lastEnv = localStorage.getItem("admin_last_env") as EnvId | null;
      const env: EnvId | null =
        lastEnv === "local" || lastEnv === "production" ? lastEnv : null;
      const savedToken = env ? localStorage.getItem(tokenKey(env)) : null;
      if (!signedOut && env && savedToken) {
        try {
          await login(savedToken, env);
        } catch {
          // Fall through to the login screen.
        }
      }
      if (!cancelled) setLoading(false);
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const login = async (token: string, env: EnvId): Promise<boolean> => {
    const cfg = envById(env);
    setLastError(null);
    try {
      if (cfg.isProduction) {
        // Bring up the SSH tunnel first; ensureTunnel throws if it can't reach
        // the backend through the forward.
        await ensureTunnel(cfg);
      } else {
        await killTunnel();
      }

      apiClient.setCredentials(token, cfg.backendUrl);
      // One cheap, read-only probe that exercises admin auth.
      const response = await apiClient.getAuditLog(1);
      if (response.ok) {
        localStorage.setItem(tokenKey(env), token);
        localStorage.setItem("admin_last_env", env);
        localStorage.removeItem(SIGNED_OUT_KEY);
        setAuthState({ token, authenticated: true, backend_url: cfg.backendUrl, env });
        return true;
      }
      const reason = `Backend rejected the request: HTTP ${response.error?.status ?? "?"} ${response.error?.message ?? ""}`.trim();
      console.error("[auth] login probe failed:", reason, response.error);
      setLastError(reason);
      apiClient.clearCredentials();
      if (cfg.isProduction) await killTunnel();
      return false;
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      console.error("[auth] login threw before reaching the backend:", err);
      setLastError(reason);
      apiClient.clearCredentials();
      if (cfg.isProduction) await killTunnel();
      return false;
    }
  };

  const logout = () => {
    localStorage.setItem(SIGNED_OUT_KEY, "1");
    apiClient.clearCredentials();
    void killTunnel();
    setAuthState({
      token: null,
      authenticated: false,
      backend_url: ENVIRONMENTS.local.backendUrl,
      env: "local",
    });
  };

  const contextValue = { authState, login, logout, loading, lastError };

  return <AuthContext.Provider value={contextValue}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
