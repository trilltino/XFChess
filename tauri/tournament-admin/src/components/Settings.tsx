import { useState } from "react";
import { apiClient } from "../services/api";
import { useAuth } from "../hooks/useAuth";
import { saveAdminToken } from "../services/credentials";
import { getOpsConfig, opsConfigProblem, saveOpsConfig, type OpsConfig } from "../config/environments";

const FEEPAYER_THRESHOLD_KEY = "feepayer_threshold_sol";

const fieldStyle = { background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px" };
const primaryButton = { padding: "8px 20px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" };

export default function Settings() {
  const { authState } = useAuth();
  const [tokenMsg, setTokenMsg] = useState<string | null>(null);
  const [candidate, setCandidate] = useState("");
  const [tokenBusy, setTokenBusy] = useState(false);
  const [ops, setOps] = useState<OpsConfig>(getOpsConfig);
  const [opsMsg, setOpsMsg] = useState<string | null>(null);
  const [threshold, setThreshold] = useState(
    parseFloat(localStorage.getItem(FEEPAYER_THRESHOLD_KEY) || "0.5").toString()
  );
  const [thresholdMsg, setThresholdMsg] = useState<string | null>(null);

  const handleRotateToken = async () => {
    if (tokenBusy) return;
    setTokenBusy(true);
    const r = await apiClient.rotateToken();
    setTokenBusy(false);
    if (!r.ok) {
      setTokenMsg(`Error: ${r.error?.message}`);
      return;
    }
    const newToken = r.data?.new_token;
    if (typeof newToken !== "string" || !newToken) {
      setTokenMsg("Error: the backend did not return a replacement key.");
      return;
    }
    setCandidate(newToken);
    setTokenMsg("Replacement generated. Install it as ADMIN_API_KEY on the backend and restart, then verify it here.");
  };

  const handleVerifyToken = async () => {
    if (tokenBusy) return;
    setTokenBusy(true);
    try {
      await apiClient.activateAdminToken(candidate, () => saveAdminToken(authState.env, candidate));
      setCandidate("");
      setTokenMsg("Replacement verified and saved to your OS credential store.");
    } catch (e) {
      setTokenMsg(`Error: ${String(e)} Your current panel credentials are retained.`);
    } finally {
      setTokenBusy(false);
    }
  };

  const handleSaveOps = () => {
    const problem = opsConfigProblem(ops);
    if (problem) { setOpsMsg(`Error: ${problem}`); return; }
    saveOpsConfig(ops);
    setOpsMsg("Saved. Used the next time you log on to PRODUCTION or open the ops console.");
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
        <p style={{ color: "var(--text-dim)", margin: "0.25rem 0 0" }}>Connection, key rotation and alert thresholds</p>
      </div>

      <SettingsCard title="OPS CONNECTION">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 1rem", lineHeight: "1.6" }}>
          The production VPS and SSH key used for the PRODUCTION tunnel and the Hetzner console. Stored on this machine only.
        </p>
        <div style={{ display: "grid", gap: "8px" }}>
          <input aria-label="Production VPS host" value={ops.vpsHost} onChange={e => setOps(o => ({ ...o, vpsHost: e.target.value }))} placeholder="VPS host (IPv4)" style={fieldStyle} />
          <input aria-label="SSH key path" value={ops.sshKeyPath} onChange={e => setOps(o => ({ ...o, sshKeyPath: e.target.value }))} placeholder="~/.ssh/xfchess_vps" style={fieldStyle} />
          <button onClick={handleSaveOps} style={{ ...primaryButton, justifySelf: "start" }}>SAVE</button>
        </div>
        {opsMsg && <div style={{ marginTop: "8px", fontSize: "12px", color: opsMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{opsMsg}</div>}
      </SettingsCard>

      {/* VPS Authority rotation — runbook, not a button */}
      <SettingsCard title="VPS AUTHORITY KEY ROTATION" danger>
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: 0, lineHeight: "1.6" }}>
          Authority-key rotation is a deliberate operational procedure, not a one-click action.
          Follow <code style={{ color: "var(--primary)" }}>ops/SECRETS_ROTATION.md</code>: generate the new keypair
          offline, update <code style={{ color: "var(--primary)" }}>VPS_AUTHORITY_KEY</code> in <code style={{ color: "var(--primary)" }}>/opt/xfchess/.env</code>,
          and restart <code style={{ color: "var(--primary)" }}>xfchess-backend</code>.
        </p>
      </SettingsCard>

      <SettingsCard title="ADMIN KEY ROTATION">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 1rem", lineHeight: "1.6" }}>
          Generate a replacement key for {authState.env.toUpperCase()}, install it as <code style={{ color: "var(--primary)" }}>ADMIN_API_KEY</code> on the backend, then restart the backend. Verification checks a protected admin endpoint before switching this panel and its saved credentials.
        </p>
        <button onClick={handleRotateToken} disabled={tokenBusy} style={primaryButton}>GENERATE REPLACEMENT</button>
        {candidate && <div style={{ display: "grid", gap: 8, marginTop: 12 }}>
          <input aria-label="Replacement admin API key" readOnly value={candidate} style={fieldStyle} />
          <button onClick={handleVerifyToken} disabled={tokenBusy} style={primaryButton}>VERIFY AND USE REPLACEMENT</button>
        </div>}
        {tokenMsg && <div style={{ marginTop: "8px", fontSize: "12px", color: tokenMsg.startsWith("Error") ? "#f87171" : "#4ade80" }}>{tokenMsg}</div>}
      </SettingsCard>

      <SettingsCard title="FEEPAYER ALERT THRESHOLD">
        <p style={{ color: "var(--text-dim)", fontSize: "12px", margin: "0 0 1rem", lineHeight: "1.6" }}>
          Dashboard shows a red banner when feepayer balance drops below this threshold.
        </p>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          <input value={threshold} onChange={e => setThreshold(e.target.value)} type="number" step="0.1" min="0" style={{ ...fieldStyle, width: "120px" }} />
          <span style={{ color: "var(--text-dim)", fontSize: "12px" }}>SOL</span>
          <button onClick={handleSaveThreshold} style={primaryButton}>SAVE</button>
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
