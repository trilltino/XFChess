import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Custom title bar for the borderless (decorations: false) tournament-admin
// window — Tauri draws no OS chrome at all once decorations are off, so this
// replaces it: a drag region plus minimize/maximize/close buttons wired to
// the Rust-side commands in tauri/src/services/ipc.rs.
export default function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    invoke<boolean>("is_tournament_admin_maximized")
      .then(setIsMaximized)
      .catch(() => {});
  }, []);

  const toggleMaximize = () => {
    invoke("toggle_maximize_tournament_admin").catch(() => {});
    setIsMaximized((v) => !v);
  };

  return (
    <div
      data-tauri-drag-region
      onDoubleClick={toggleMaximize}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        height: "40px",
        flexShrink: 0,
        backgroundColor: "var(--bg)",
        borderBottom: "1px solid var(--border)",
        userSelect: "none",
        WebkitUserSelect: "none",
      }}
    >
      <div
        data-tauri-drag-region
        style={{
          display: "flex",
          alignItems: "center",
          gap: "0.6rem",
          padding: "0 0.9rem",
          flex: 1,
          minWidth: 0,
          height: "100%",
        }}
      >
        <span style={{ fontSize: "15px", color: "var(--primary)" }}>♞</span>
        <span style={{ fontSize: "12px", fontWeight: 700, color: "var(--text)", whiteSpace: "nowrap" }}>
          XF<span style={{ color: "var(--primary)" }}>Chess</span> Tournament Admin
        </span>
      </div>

      <div style={{ display: "flex", height: "100%" }}>
        <TitleBarButton label="Minimize" onClick={() => invoke("minimize_tournament_admin").catch(() => {})}>
          <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0" y="4.5" width="10" height="1" fill="currentColor" /></svg>
        </TitleBarButton>
        <TitleBarButton label={isMaximized ? "Restore" : "Maximize"} onClick={toggleMaximize}>
          {isMaximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="2" y="0.5" width="7.5" height="7.5" fill="none" stroke="currentColor" strokeWidth="1" />
              <rect x="0.5" y="2" width="7.5" height="7.5" fill="var(--bg)" stroke="currentColor" strokeWidth="1" />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" />
            </svg>
          )}
        </TitleBarButton>
        <TitleBarButton label="Close" danger onClick={() => invoke("close_tournament_admin").catch(() => {})}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0.5" y1="0.5" x2="9.5" y2="9.5" stroke="currentColor" strokeWidth="1.2" />
            <line x1="9.5" y1="0.5" x2="0.5" y2="9.5" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </TitleBarButton>
      </div>
    </div>
  );
}

function TitleBarButton({
  children,
  onClick,
  label,
  danger,
}: {
  children: React.ReactNode;
  onClick: () => void;
  label: string;
  danger?: boolean;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      aria-label={label}
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: "44px",
        height: "100%",
        border: "none",
        borderRadius: 0,
        padding: 0,
        backdropFilter: "none",
        background: hover ? (danger ? "#ef4444" : "rgba(255,255,255,0.08)") : "transparent",
        color: hover && danger ? "#ffffff" : "var(--text-dim)",
        cursor: "pointer",
        transition: "background-color 0.12s ease, color 0.12s ease",
      }}
    >
      {children}
    </button>
  );
}
