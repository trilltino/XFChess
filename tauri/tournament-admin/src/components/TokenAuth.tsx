import { useState, useEffect } from "react";
import type { AdminAuthState } from "../types/tournament";

interface TokenAuthProps {
  onAuth: (authState: AdminAuthState) => void;
}

export default function TokenAuth({ onAuth }: TokenAuthProps) {
  const [token, setToken] = useState("");
  const [backendUrl, setBackendUrl] = useState("http://127.0.0.1:8090");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    const savedToken = localStorage.getItem("admin_token");
    const savedUrl = localStorage.getItem("backend_url");
    if (savedToken) setToken(savedToken);
    if (savedUrl) setBackendUrl(savedUrl);
  }, []);

  useEffect(() => {
    if (token) localStorage.setItem("admin_token", token);
    if (backendUrl) localStorage.setItem("backend_url", backendUrl);
  }, [token, backendUrl]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError("");

    try {
      const response = await fetch(`${backendUrl}/admin/players`, {
        headers: {
          "X-API-Key": token,
          "Content-Type": "application/json",
        },
      });

      if (response.ok) {
        onAuth({
          token,
          authenticated: true,
          backend_url: backendUrl,
        });
      } else {
        setError("Invalid access credentials");
      }
    } catch (err) {
      setError(`Handshake failed: ${err instanceof Error ? err.message : "Network error"}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{
      display: "flex",
      justifyContent: "center",
      alignItems: "center",
      minHeight: "100vh",
      backgroundColor: "var(--bg)",
      fontFamily: "'Outfit', sans-serif",
      position: "relative",
      overflow: "hidden"
    }}>
      <div className="onboarding-bg" />

      <div style={{
        backgroundColor: "rgba(10, 33, 26, 0.4)",
        backdropFilter: "blur(40px)",
        padding: "3rem",
        borderRadius: "32px",
        border: "1px solid var(--border)",
        boxShadow: "0 40px 100px rgba(0,0,0,0.5)",
        width: "480px",
        zIndex: 10,
        textAlign: "center"
      }}>
        <div style={{ marginBottom: "2.5rem" }}>
          <h1 style={{ color: "#fff", fontSize: "32px", fontWeight: "900", marginBottom: "0.5rem", letterSpacing: "-1px" }}>
            XF<span style={{ color: "var(--primary)" }}>CHESS</span>
          </h1>
          <div style={{ color: "var(--text-dim)", fontSize: "12px", letterSpacing: "2px", fontWeight: "700" }}>TOURNAMENT ORCHESTRATOR</div>
        </div>

        <form onSubmit={handleSubmit} style={{ textAlign: "left", marginTop: "1rem" }}>
          <div style={{ marginBottom: "1.5rem" }}>
            <label style={labelStyle}>UPLINK ENDPOINT</label>
            <input
              type="text"
              value={backendUrl}
              onChange={(e) => setBackendUrl(e.target.value)}
              style={inputStyle}
              placeholder="http://127.0.0.1:8090"
            />
          </div>

          <div style={{ marginBottom: "2.5rem" }}>
            <label style={labelStyle}>ADMIN ACCESS TOKEN</label>
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              style={inputStyle}
              placeholder="••••••••••••••••"
              required
            />
          </div>

          {error && (
            <div style={{
              color: "#ef4444",
              marginBottom: "1.5rem",
              padding: "1rem",
              backgroundColor: "rgba(239, 68, 68, 0.1)",
              border: "1px solid rgba(239, 68, 68, 0.3)",
              borderRadius: "16px",
              fontSize: "12px",
              textAlign: "center",
              fontWeight: "600"
            }}>
              AUTHENTICATION ERROR: {error}
            </div>
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
              boxShadow: "0 10px 30px rgba(173, 92, 47, 0.3)"
            }}
          >
            {loading ? "ESTABLISHING SECURE SESSION..." : "INITIATE TERMINAL"}
          </button>
        </form>

        <div style={{ marginTop: "2rem", fontSize: "10px", color: "rgba(255,255,255,0.15)", letterSpacing: "1px" }}>
          SECURE CHANNEL 256-BIT ENCRYPTION ACTIVE
        </div>
      </div>
    </div>
  );
}

const labelStyle: React.CSSProperties = {
  display: "block",
  color: "var(--text-dim)",
  fontSize: "10px",
  fontWeight: "800",
  letterSpacing: "1.5px",
  marginBottom: "10px"
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
  textAlign: "center"
};
