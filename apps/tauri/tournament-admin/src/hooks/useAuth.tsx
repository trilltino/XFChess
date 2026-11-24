import { useState, useEffect, useRef, createContext, useContext, ReactNode } from "react";
import { apiClient } from "../services/api";
import type { AdminAuthState } from "../types/tournament";
import { envById, getOpsConfig, opsConfigProblem, type EnvId } from "../config/environments";
import { ensureTunnel, killTunnel } from "../services/tunnel";
import { clearAdminSession, loadAdminSession, saveAdminSession } from "../services/credentials";
import { passwordSessionToken, rememberUsername } from "../services/adminLogin";

interface AuthContextType {
  authState: AdminAuthState;
  /** Sign in to an environment with a username and password; opens the SSH tunnel for PRODUCTION. */
  login: (username: string, password: string, env: EnvId) => Promise<boolean>;
  logout: () => void;
  loading: boolean;
  /** The specific failure from the last login attempt, including tunnel and ACL errors. */
  lastError: string | null;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

// Non-secret session hint. The session itself is in the OS credential store.
const LAST_ENV_KEY = "admin_last_env";
// Left by earlier panel versions (API keys, wallet sign-in); removed on start.
const OBSOLETE_KEYS = ["admin_token", "admin_token_local", "admin_token_production", "admin_wallet_keypair_path", "admin_signed_out"];

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

  // Resume a saved session that has not expired; otherwise show the login screen.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        OBSOLETE_KEYS.forEach(key => localStorage.removeItem(key));
      } catch {
        // Storage may be unavailable; nothing to clean up then.
      }
      const lastEnv = localStorage.getItem(LAST_ENV_KEY);
      const env: EnvId | null = lastEnv === "local" || lastEnv === "production" ? lastEnv : null;
      if (env) {
        try {
          const saved = await loadAdminSession(env);
          if (saved && !(await connect(env, async () => ({ token: saved, remember: false })))) {
            // Expired, revoked by a password change, or the backend is unreachable: sign in again.
            await clearAdminSession(env).catch(() => {});
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
   * The shared sign-in flow: open or close the tunnel for the environment, obtain a session,
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
      // A new session is saved so a restart within its eight hours resumes it.
      if (remember) await saveAdminSession(env, token);
      if (!current()) return false;
      localStorage.setItem(LAST_ENV_KEY, env);
      setAuthState({ token, authenticated: true, backend_url: cfg.backendUrl, env });
      return true;
    } catch (err) {
      if (!current()) return false;
      console.error("[auth] login failed before reaching the backend:", err);
      return fail(err instanceof Error ? err.message : String(err), cfg.isProduction);
    }
  };

  const login = async (username: string, password: string, env: EnvId): Promise<boolean> => {
    const ok = await connect(env, async (backendUrl) => ({
      token: await passwordSessionToken(backendUrl, username, password),
      remember: true,
    }));
    if (ok) rememberUsername(username.trim());
    return ok;
  };

  const logout = () => {
    generation.current++;
    void clearAdminSession(authState.env).catch(() => {});
    apiClient.clearCredentials();
    void killTunnel();
    setAuthState({ token: null, authenticated: false, backend_url: LOCAL_URL, env: "local" });
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
