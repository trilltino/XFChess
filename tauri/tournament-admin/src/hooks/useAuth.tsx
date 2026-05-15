import { useState, useEffect, createContext, useContext, ReactNode } from "react";
import { apiClient } from "../services/api";
import type { AdminAuthState } from "../types/tournament";

interface AuthContextType {
  authState: AdminAuthState;
  login: (token: string, backendUrl: string) => Promise<boolean>;
  logout: () => void;
  loading: boolean;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [authState, setAuthState] = useState<AdminAuthState>({
    token: null,
    authenticated: false,
    backend_url: "http://127.0.0.1:8090",
  });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const token = localStorage.getItem("admin_token");
    const backendUrl = localStorage.getItem("backend_url");
    
    if (token && backendUrl) {
      setAuthState({
        token,
        authenticated: true,
        backend_url: backendUrl,
      });
      apiClient.setCredentials(token, backendUrl);
    }
    setLoading(false);
  }, []);

  const login = async (token: string, backendUrl: string): Promise<boolean> => {
    try {
      apiClient.setCredentials(token, backendUrl);
      const response = await apiClient.getTournaments();
      
      if (response.ok) {
        setAuthState({
          token,
          authenticated: true,
          backend_url: backendUrl,
        });
        return true;
      } else {
        apiClient.clearCredentials();
        return false;
      }
    } catch {
      apiClient.clearCredentials();
      return false;
    }
  };

  const logout = () => {
    apiClient.clearCredentials();
    setAuthState({
      token: null,
      authenticated: false,
      backend_url: "http://127.0.0.1:8090",
    });
  };

  const contextValue = { authState, login, logout, loading };

  return (
    <AuthContext.Provider value={contextValue}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
