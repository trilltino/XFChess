import { useState, useEffect } from "react";
import { useAuth } from "../hooks/useAuth";
import { ENVIRONMENTS, type EnvId } from "../config/environments";

export default function TokenAuth() {
  const { login } = useAuth();
  const [env, setEnv] = useState<EnvId>("local");
  const [token, setToken] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const cfg = ENVIRONMENTS[env];

  // Prefill the per-environment token when the selected environment changes.
  useEffect(() => {
    const saved = localStorage.getItem(`admin_token_${env}`);
    setToken(saved || "");
    setError("");
  }, [env]);

  // Restore the last-used environment on mount.
  useEffect(() => {
    const last = localStorage.getItem("admin_last_env") as EnvId | null;
    if (last === "local" || last === "production") setEnv(last);
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError("");
    try {
      const ok = await login(token, env);
      if (!ok) {
        setError(
          cfg.isProduction
            ? "Could not authenticate. Tunnel down, or bad token — check the 'tunnel' SSH user and key."
            : "Invalid access credentials, or no local backend on 127.0.0.1:8090."
        );
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Connection failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={pageStyle}>
      <div className="onboarding-bg" />
      <div style={cardStyle}>
        <div style={{ marginBottom: "2rem" }}>
          <h1 style={{ color: "#fff", fontSize: "32px", fontWeight: 900, marginBottom: "0.5rem", letterSpacing: "-1px" }}>
            XF<span style={{ color: "var(--primary)" }}>CHESS</span>
          </h1>
          <div style={{ color: "var(--text-dim)", fontSize: "12px", letterSpacing: "2px", fontWeight: 700 }}>
            TOURNAMENT ORCHESTRATOR
          </div>
        </div>

        {/* Environment selector */}
        <div style={{ display: "flex", gap: "10px", marginBottom: "1.25rem" }}>
          {(Object.keys(ENVIRONMENTS) as EnvId[]).map((id) => {
            const e = ENVIRONMENTS[id];
            const active = env === id;
            const prod = e.isProduction;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setEnv(id)}
                style={{
                  flex: 1,
                  padding: "0.9rem",
                  borderRadius: "16px",
                  fontSize: "13px",
                  fontWeight: 800,
                  letterSpacing: "1px",
                  cursor: "pointer",
                  transition: "all 0.15s ease",
                  border: active
                    ? `1px solid ${prod ? "#ef4444" : "#4ade80"}`
                    : "1px solid var(--border)",
                  background: active
                    ? prod
                      ? "rgba(239,68,68,0.15)"
                      : "rgba(74,222,128,0.12)"
                    : "rgba(0,0,0,0.25)",
                  color: active ? (prod ? "#ef4444" : "#4ade80") : "var(--text-dim)",
                }}
              >
                {e.label}
              </button>
            );
          })}
        </div>

        <div style={{ fontSize: "11px", color: "var(--text-dim)", marginBottom: "1.5rem", lineHeight: 1.6 }}>
          {cfg.isProduction ? (
            <>Connects via SSH tunnel to <code>{`${cfg.tunnel!.sshUser}@${cfg.tunnel!.sshHost}`}</code> → backend loopback :{cfg.tunnel!.remotePort}. Requires the deploy key.</>
          ) : (
            <>Talks to a backend you run locally at <code>{cfg.backendUrl}</code>.</>
          )}
        </div>

        <form onSubmit={handleSubmit} style={{ textAlign: "left" }}>
          <div style={{ marginBottom: "1.75rem" }}>
            <label style={labelStyle}>ADMIN ACCESS TOKEN</label>
            <input
              type="password"
              value={token}
              onChange={(ev) => setToken(ev.target.value)}
              style={inputStyle}
              placeholder="••••••••••••••••"
              required
            />
          </div>

          {error && (
            <div style={errorBoxStyle}>{error}</div>
          )}

          <button
            type="submit"
            disabled={loading || !token.trim()}
            className="primary"
            style={{
              width: "100%",
              padding: "1.1rem",
              fontSize: "15px",
              borderRadius: "100px",
              boxShadow: "0 10px 30px rgba(173, 92, 47, 0.3)",
            }}
          >
            {loading
              ? cfg.isProduction
                ? "ESTABLISHING TUNNEL…"
                : "CONNECTING…"
              : `INITIATE ${cfg.label} TERMINAL`}
          </button>
        </form>
      </div>
    </div>
  );
}

const pageStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "center",
  alignItems: "center",
  minHeight: "100vh",
  backgroundColor: "var(--bg)",
  fontFamily: "'Outfit', sans-serif",
  position: "relative",
  overflow: "hidden",
};

const cardStyle: React.CSSProperties = {
  backgroundColor: "rgba(10, 33, 26, 0.4)",
  backdropFilter: "blur(40px)",
  padding: "3rem",
  borderRadius: "32px",
  border: "1px solid var(--border)",
  boxShadow: "0 40px 100px rgba(0,0,0,0.5)",
  width: "480px",
  zIndex: 10,
  textAlign: "center",
};

const labelStyle: React.CSSProperties = {
  display: "block",
  color: "var(--text-dim)",
  fontSize: "10px",
  fontWeight: 800,
  letterSpacing: "1.5px",
  marginBottom: "10px",
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "1.1rem 1.5rem",
  backgroundColor: "rgba(0, 0, 0, 0.3)",
  border: "1px solid var(--border)",
  borderRadius: "100px",
  color: "#ffffff",
  fontSize: "14px",
  outline: "none",
  transition: "all 0.2s ease",
  textAlign: "center",
};

const errorBoxStyle: React.CSSProperties = {
  color: "#ef4444",
  marginBottom: "1.5rem",
  padding: "1rem",
  backgroundColor: "rgba(239, 68, 68, 0.1)",
  border: "1px solid rgba(239, 68, 68, 0.3)",
  borderRadius: "16px",
  fontSize: "12px",
  textAlign: "center",
  fontWeight: 600,
};
