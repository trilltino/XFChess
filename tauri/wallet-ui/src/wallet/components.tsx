import type { CSSProperties } from "react";
import { TEXT, TEXT_DIM } from "./theme";

export function PrimaryBtn({
  children, onClick, disabled, loading, style,
}: {
  children: React.ReactNode; onClick?: () => void; disabled?: boolean; loading?: boolean; style?: CSSProperties;
}) {
  return (
    <button onClick={onClick} disabled={disabled || loading} style={{
      width: "100%", padding: "14px 0", borderRadius: 10, border: "none",
      background: disabled || loading ? "rgba(255,255,255,0.12)" : "#ffffff",
      color: disabled || loading ? TEXT_DIM : "#000000", fontSize: 15, fontWeight: 700, letterSpacing: "0.02em",
      transition: "all 0.2s", boxShadow: disabled || loading ? "none" : `0 4px 20px rgba(255,255,255,0.15)`,
      display: "flex", alignItems: "center", justifyContent: "center", gap: 8, ...style,
    }}>
      {loading && <div style={{ width: 16, height: 16, border: "2px solid rgba(255,255,255,0.3)", borderTop: "2px solid #fff", borderRadius: "50%", animation: "spin 0.7s linear infinite" }} />}
      {children}
    </button>
  );
}

export function ErrorMsg({ msg }: { msg: string }) {
  return (
    <div style={{
      padding: "10px 14px", borderRadius: 10, background: "rgba(255,255,255,0.04)",
      border: `1px solid rgba(255,255,255,0.20)`, color: TEXT, fontSize: 13, marginBottom: 16,
    }}>
      {msg}
    </div>
  );
}

