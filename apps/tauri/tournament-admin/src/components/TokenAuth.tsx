import { useState } from "react";
import { useAuth } from "../hooks/useAuth";
import { ENV_IDS, envById, type EnvId } from "../config/environments";
import { savedUsername } from "../services/adminLogin";

export default function TokenAuth() {
  const { login, lastError } = useAuth();
  const [env, setEnv] = useState<EnvId>(() => localStorage.getItem("admin_last_env") === "production" ? "production" : "local");
  const [username, setUsername] = useState(savedUsername);
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  // The sign-in failed: show the reason useAuth recorded (wrong password, tunnel, setup).
  const [failed, setFailed] = useState(false);

  const cfg = envById(env);
  const fallback = cfg.isProduction
    ? "Could not sign in. Is the tunnel up? Check the 'tunnel' SSH user and key."
    : "Could not sign in. Is the backend running on 127.0.0.1:8090?";
  const message = failed ? lastError || fallback : error;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setFailed(false);
    setError("");
    setLoading(true);
    try {
      if (await login(username, password, env)) setPassword("");
      else setFailed(true);
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
                onClick={() => { setEnv(id); setError(""); setFailed(false); }}
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
                {envById(id).label}
              </button>
            );
          })}
        </div>

        <form onSubmit={handleSubmit} style={{ textAlign: "left" }}>
          <div style={{ display: "grid", gap: "10px", marginBottom: "1.75rem" }}>
            <input
              value={username}
              onChange={(ev) => setUsername(ev.target.value)}
              style={inputStyle}
              placeholder="Username"
              aria-label="Username"
              autoComplete="username"
              spellCheck={false}
              autoFocus={!username}
              required
            />
            <input
              type="password"
              value={password}
              onChange={(ev) => setPassword(ev.target.value)}
              style={inputStyle}
              placeholder="Password"
              aria-label="Password"
              autoComplete="current-password"
              autoFocus={Boolean(username)}
              required
            />
          </div>

          {message && (
            <div style={errorBoxStyle}>{message}</div>
          )}

          <button
            type="submit"
            disabled={loading || !username.trim() || !password}
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
                : "SIGNING IN…"
              : "SIGN IN"}
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
