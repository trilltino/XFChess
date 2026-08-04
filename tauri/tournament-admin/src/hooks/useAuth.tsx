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
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

// Per-environment token storage so a LOCAL token is never replayed at PRODUCTION.
const tokenKey = (env: EnvId) => `admin_token_${env}`;

export function AuthProvider({ children }: { children: ReactNode }) {
  const [authState, setAuthState] = useState<AdminAuthState>({
    token: null,
    authenticated: false,
    backend_url: ENVIRONMENTS.local.backendUrl,
    env: "local",
  });
  const [loading, setLoading] = useState(true);

  // No auto-login: production requires actively opening a tunnel, so always
  // start at the selector. We only prefill the last-used env's token in the UI.
  useEffect(() => {
    setLoading(false);
  }, []);

  const login = async (token: string, env: EnvId): Promise<boolean> => {
    const cfg = envById(env);
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
        setAuthState({ token, authenticated: true, backend_url: cfg.backendUrl, env });
        return true;
      }
      apiClient.clearCredentials();
      if (cfg.isProduction) await killTunnel();
      return false;
    } catch {
      apiClient.clearCredentials();
      if (cfg.isProduction) await killTunnel();
      return false;
    }
  };

  const logout = () => {
    apiClient.clearCredentials();
    void killTunnel();
    setAuthState({
      token: null,
      authenticated: false,
      backend_url: ENVIRONMENTS.local.backendUrl,
      env: "local",
    });
  };

  const contextValue = { authState, login, logout, loading };

  return <AuthContext.Provider value={contextValue}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
