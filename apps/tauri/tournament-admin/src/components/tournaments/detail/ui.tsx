// Shared presentational pieces and formatters for the tournament detail screen.

export const fmtSol = (lamports: number) => (lamports / 1e9).toFixed(4) + " SOL";
export const fmtTs = (t: number) => new Date(t * 1000).toLocaleString();
export const shorten = (k: string) => (k.length > 16 ? `${k.slice(0, 8)}…${k.slice(-4)}` : k);
export const statusColor = (s: string) =>
  ({ active: "var(--primary)", completed: "#3b82f6", scheduled: "var(--accent)", registration: "#4ade80" }[s.toLowerCase()] ?? "var(--text-dim)");
export const isError = (msg: string | null | undefined) => Boolean(msg && (msg.startsWith("Error") || msg.startsWith("Failed")));

export const InfoCard = ({ title, children }: { title: string; children: React.ReactNode }) => (
  <div style={{ backgroundColor: "var(--surface)", padding: "2rem", borderRadius: "24px", border: "1px solid var(--border)", backdropFilter: "blur(20px)", boxShadow: "0 10px 40px rgba(0,0,0,0.3)" }}>
    <h4 style={{ color: "var(--primary)", fontSize: "11px", fontWeight: "800", letterSpacing: "2px", margin: "0 0 1.5rem 0" }}>{title}</h4>
    {children}
  </div>
);

export const Row = ({ label, value, color }: { label: string; value: React.ReactNode; color?: string }) => (
  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "12px", alignItems: "baseline" }}>
    <span style={{ color: "var(--text-dim)", fontSize: "11px", fontWeight: "700" }}>{label}</span>
    <span style={{ color: color || "#fff", fontSize: "14px", fontWeight: "800" }}>{value}</span>
  </div>
);

export const Message = ({ text }: { text: string | null | undefined }) =>
  text ? (
    <div role={isError(text) ? "alert" : "status"} style={{ marginTop: "6px", fontSize: "11px", color: isError(text) ? "#f87171" : "#4ade80", wordBreak: "break-word" }}>
      {text}
    </div>
  ) : null;

export const StatusPill = ({ status }: { status: string }) => {
  const c = ({ Completed: { bg: "rgba(59,130,246,0.12)", text: "#60a5fa" }, Active: { bg: "rgba(74,222,128,0.12)", text: "#4ade80" } } as Record<string, { bg: string; text: string }>)[status]
    ?? { bg: "rgba(255,255,255,0.06)", text: "var(--text-dim)" };
  return <span style={{ fontSize: "10px", fontWeight: "800", padding: "2px 8px", borderRadius: "4px", backgroundColor: c.bg, color: c.text }}>{status.toUpperCase()}</span>;
};

export const PlayerTag = ({ color, name, isWinner }: { color: "white" | "black"; name: string; isWinner: boolean }) => (
  <div style={{ display: "flex", alignItems: "center", gap: "6px", flex: 1, minWidth: 0 }}>
    <div style={{ width: "10px", height: "10px", borderRadius: "50%", flexShrink: 0, backgroundColor: color === "white" ? "#e2e8f0" : "#1e293b", border: "1.5px solid rgba(255,255,255,0.2)" }} />
    <span style={{ fontSize: "12px", fontFamily: "monospace", color: isWinner ? "#4ade80" : "#cbd5e1", fontWeight: isWinner ? "800" : "400", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
      {name.length > 20 ? `${name.slice(0, 8)}…${name.slice(-4)}` : name}
    </span>
    {isWinner && <span style={{ fontSize: "10px", color: "#4ade80" }}>✓</span>}
  </div>
);

export const inputStyle: React.CSSProperties = { background: "rgba(255,255,255,0.06)", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "6px 10px", fontSize: "12px" };
export const primaryButtonStyle: React.CSSProperties = { padding: "6px 14px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "700", fontSize: "12px", cursor: "pointer" };
export const ghostButtonStyle: React.CSSProperties = { padding: "6px 14px", borderRadius: "8px", background: "rgba(255,255,255,0.08)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "11px", cursor: "pointer" };
export const dangerButtonStyle: React.CSSProperties = { padding: "2px 8px", borderRadius: "4px", background: "rgba(239,68,68,0.12)", color: "#f87171", border: "1px solid rgba(239,68,68,0.3)", fontSize: "10px", cursor: "pointer" };
