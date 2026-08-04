import { useState } from "react";
import { apiClient } from "../services/api";

const FEEPAYER_THRESHOLD_KEY = "feepayer_threshold_sol";

export default function Settings() {
  // Token rotation
  const [tokenMsg, setTokenMsg] = useState<string | null>(null);

  // Feepayer threshold
  const [threshold, setThreshold] = useState(
    parseFloat(localStorage.getItem(FEEPAYER_THRESHOLD_KEY) || "0.5").toString()
  );
  const [thresholdMsg, setThresholdMsg] = useState<string | null>(null);

  const handleRotateToken = async () => {
    const r = await apiClient.rotateToken();
    if (r.ok) {
      const newToken = r.data?.new_token as string;
      apiClient.setCredentials(newToken, apiClient.getBaseUrl());
      setTokenMsg(`Token rotated. New token saved to localStorage. Old token invalidated on next backend restart.`);
    } else {
      setTokenMsg(`Error: ${r.error?.message}`);
    }
  };

  const handleSaveThreshold = () => {
    const v = parseFloat(threshold);
    if (isNaN(v) || v < 0) return;
    localStorage.setItem(FEEPAYER_THRESHOLD_KEY, v.toString());
    setThresholdMsg(`Threshold saved: ${v} SOL`);
    setTimeout(() => setThresholdMsg(null), 2000);
  };

  return (
    <div style={{ padding: "1.5rem", display: "flex", flexDirection: "column", gap: "2rem", maxWidth: "700px" }}>
      <div>
        <h1 style={{ margin: 0, color: "#fff", fontSize: "1.5rem" }}>ADMIN <span style={{ color: "var(--primary)" }}>SETTINGS</span></h1>
        <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0" }}>Key rotation, token management, and alert thresholds</p>
      </div>

      {/* VPS Authority rotation — runbook, not a button */}
      <SettingsCard title="VPS AUTHORITY KEY ROTATION" danger>
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: 0, lineHeight: "1.6" }}>
          Authority-key rotation is a deliberate operational procedure, not a one-click action — the
          old "rotate" button only logged and told you to hand-edit <code style={{ color: "var(--primary)" }}>.env</code> anyway.
          Follow <code style={{ color: "var(--primary)" }}>ops/SECRETS_ROTATION.md</code>: generate the new keypair
          offline, update <code style={{ color: "var(--primary)" }}>VPS_AUTHORITY_KEY</code> in <code style={{ color: "var(--primary)" }}>/opt/xfchess/.env</code>,
          and restart <code style={{ color: "var(--primary)" }}>xfchess-backend</code>. Treasury/dispute authorities move to a Squads multisig in Phase 5.
        </p>
      </SettingsCard>

      {/* Admin token rotation */}
      <SettingsCard title="ADMIN TOKEN ROTATION">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 1rem", lineHeight: "1.6" }}>
          Generate a new admin Bearer token. The new token is saved to localStorage and this session automatically. Old token is invalidated on next backend restart (update <code style={{ color: "var(--primary)" }}>ADMIN_TOKEN</code> in .env).
        </p>
        <button onClick={handleRotateToken}
          style={{ padding: "8px 20px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" }}>
          ROTATE TOKEN
        </button>
        {tokenMsg && <div style={{ marginTop: "8px", fontSize: "12px", color: tokenMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{tokenMsg}</div>}
      </SettingsCard>

      {/* Feepayer alert threshold */}
      <SettingsCard title="FEEPAYER ALERT THRESHOLD">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 1rem", lineHeight: "1.6" }}>
          Dashboard shows a red banner when feepayer balance drops below this threshold.
        </p>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          <input value={threshold} onChange={e => setThreshold(e.target.value)} type="number" step="0.1" min="0"
            style={{ width: "120px", background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" }} />
          <span style={{ color: "var(--text-dim)", fontSize: "12px" }}>SOL</span>
          <button onClick={handleSaveThreshold}
            style={{ padding: "8px 20px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" }}>
            SAVE
          </button>
        </div>
        {thresholdMsg && <div style={{ marginTop: "8px", fontSize: "12px", color: "#4ade80" }}>{thresholdMsg}</div>}
      </SettingsCard>
    </div>
  );
}

function SettingsCard({ title, children, danger }: { title: string; children: React.ReactNode; danger?: boolean }) {
  return (
    <div style={{
      backgroundColor: "var(--surface)", padding: "2rem", borderRadius: "24px",
      border: `1px solid ${danger ? "rgba(245,158,11,0.3)" : "var(--border)"}`,
      backdropFilter: "blur(20px)"
    }}>
      <h4 style={{ color: danger ? "#f59e0b" : "var(--primary)", fontSize: "11px", fontWeight: "800", letterSpacing: "2px", margin: "0 0 1.25rem" }}>{title}</h4>
      {children}
    </div>
  );
}
