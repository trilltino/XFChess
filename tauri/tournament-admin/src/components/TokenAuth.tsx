import { useState, useEffect } from "react";
import { useAuth } from "../hooks/useAuth";
import { ENV_IDS, envById, getOpsConfig, opsConfigProblem, saveOpsConfig, type EnvId, type OpsConfig } from "../config/environments";
import { clearAdminToken, loadAdminToken } from "../services/credentials";

export default function TokenAuth() {
  const { login, lastError } = useAuth();
  const [env, setEnv] = useState<EnvId>(() => localStorage.getItem("admin_last_env") === "production" ? "production" : "local");
  const [token, setToken] = useState("");
  const [hasSavedToken, setHasSavedToken] = useState(false);
  const [ops, setOps] = useState<OpsConfig>(getOpsConfig);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const cfg = envById(env, ops);

  // Prefill the key saved for the selected environment (OS credential store).
  useEffect(() => {
    let cancelled = false;
    setError("");
    loadAdminToken(env)
      .then(saved => { if (!cancelled) { setToken(saved ?? ""); setHasSavedToken(Boolean(saved)); } })
      .catch(() => { if (!cancelled) { setToken(""); setHasSavedToken(false); } });
    return () => { cancelled = true; };
  }, [env]);

  const forgetSavedKey = async () => {
    await clearAdminToken(env).catch(() => {});
    setToken("");
    setHasSavedToken(false);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (cfg.isProduction) {
      const problem = opsConfigProblem(ops);
      if (problem) { setError(problem); return; }
      saveOpsConfig(ops);
    }
    setLoading(true);
    setError("");
    try {
      const ok = await login(token, env);
      if (!ok) {
        const fallback = cfg.isProduction
          ? "Could not authenticate. Tunnel down, or bad key — check the 'tunnel' SSH user and key."
          : "Invalid admin key, or no local backend on 127.0.0.1:8090.";
        // Show the actual auth error so key, tunnel, and permission failures remain distinct.
        setError(lastError ? `${fallback}\n\nDetail: ${lastError}` : fallback);
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
        </div>

        {/* Environment selector */}
        <div style={{ display: "flex", gap: "10px", marginBottom: "1.25rem" }}>
          {ENV_IDS.map((id) => {
            const active = env === id;
            const prod = id === "production";
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
                  border: active ? `1px solid ${prod ? "#ef4444" : "#4ade80"}` : "1px solid var(--border)",
                  background: active ? (prod ? "rgba(239,68,68,0.15)" : "rgba(74,222,128,0.12)") : "rgba(0,0,0,0.25)",
                  color: active ? (prod ? "#ef4444" : "#4ade80") : "var(--text-dim)",
                }}
              >
                {envById(id, ops).label}
              </button>
            );
          })}
        </div>

        <form onSubmit={handleSubmit} style={{ textAlign: "left" }}>
          {cfg.isProduction && (
            <div style={{ display: "grid", gap: "10px", marginBottom: "1rem" }}>
              <input
                value={ops.vpsHost}
                onChange={(ev) => setOps(o => ({ ...o, vpsHost: ev.target.value }))}
                style={inputStyle}
                placeholder="Production VPS host (IPv4)"
                aria-label="Production VPS host"
              />
              <input
                value={ops.sshKeyPath}
                onChange={(ev) => setOps(o => ({ ...o, sshKeyPath: ev.target.value }))}
                style={inputStyle}
                placeholder="SSH key path, e.g. ~/.ssh/xfchess_vps"
                aria-label="SSH key path"
              />
            </div>
          )}
          <div style={{ marginBottom: "1.75rem" }}>
            <input
              type="password"
              value={token}
              onChange={(ev) => setToken(ev.target.value)}
              style={inputStyle}
              placeholder="Admin API key"
              aria-label="Admin API key"
              required
            />
            {hasSavedToken && (
              <button type="button" onClick={forgetSavedKey}
                style={{ marginTop: "8px", background: "none", border: "none", color: "var(--text-dim)", fontSize: "11px", cursor: "pointer", textDecoration: "underline" }}>
                Using the key saved in your OS credential store — forget it
              </button>
            )}
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
              : "LOG ON"}
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
  height: "100%",
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
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
};
