import { useState, useRef, useEffect } from "react";
import { Command } from "@tauri-apps/plugin-shell";
import { OPS_SSH } from "../config/environments";

interface TerminalLine {
  type: "cmd" | "out" | "err" | "info";
  text: string;
}

export default function HetznerSsh() {
  const [status, setStatus] = useState<"checking" | "online" | "offline">("checking");
  const [commandInput, setCommandInput] = useState("");
  const [lines, setLines] = useState<TerminalLine[]>([
    { type: "info", text: "XFChess Hetzner SSH Terminal — type a command below or click a quick action." }
  ]);
  const [running, setRunning] = useState(false);
  const termRef = useRef<HTMLDivElement>(null);
  const serverIp = OPS_SSH.host;
  const sshTarget = `${OPS_SSH.user}@${serverIp}`;

  useEffect(() => {
    // Ping to check reachability
    (async () => {
      try {
        const cmd = Command.create("ssh", ["-i", OPS_SSH.key, "-o", "ConnectTimeout=3", "-o", "BatchMode=yes", sshTarget, "echo ok"]);
        const out = await cmd.execute();
        setStatus(out.stdout.trim() === "ok" ? "online" : "offline");
      } catch { setStatus("offline"); }
    })();
  }, []);

  useEffect(() => {
    termRef.current?.scrollTo(0, termRef.current.scrollHeight);
  }, [lines]);

  const push = (type: TerminalLine["type"], text: string) =>
    setLines(prev => [...prev, { type, text }]);

  const runSshCommand = async (remoteCmd: string) => {
    if (running) return;
    setRunning(true);
    push("cmd", `$ ssh ${sshTarget} "${remoteCmd}"`);
    try {
      const child = Command.create("ssh", ["-i", OPS_SSH.key, sshTarget, remoteCmd]);
      child.stdout.on("data", data => push("out", data));
      child.stderr.on("data", data => push("err", data));
      const result = await child.execute();
      if (result.code !== 0) push("err", `Exit code: ${result.code}`);
    } catch (err: any) {
      push("err", `Error: ${err?.message ?? String(err)}`);
    }
    setRunning(false);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const cmd = commandInput.trim();
    if (!cmd) return;
    setCommandInput("");
    runSshCommand(cmd);
  };

  const openNativeTerminal = async () => {
    try {
      await Command.create("powershell", ["-NoExit", "-Command", `ssh -i ${OPS_SSH.key} ${sshTarget}`]).spawn();
    } catch (err: any) {
      push("err", `Failed to open native terminal: ${err?.message}`);
    }
  };

  const quickActions: { label: string; cmd: string }[] = [
    { label: "UPTIME", cmd: "uptime && free -h" },
    { label: "JOURNAL ERRORS", cmd: "journalctl -p err -n 30 --no-pager" },
    { label: "BACKEND STATUS", cmd: "systemctl status xfchess-backend --no-pager -l" },
    { label: "DISK USAGE", cmd: "df -h" },
    { label: "RESTART BACKEND", cmd: "sudo systemctl restart xfchess-backend && echo 'backend restarted'" },
    { label: "NGINX ERRORS", cmd: "tail -n 50 /var/log/nginx/error.log" },
  ];

  const lineColor = (type: TerminalLine["type"]) => {
    switch (type) {
      case "cmd": return "var(--primary)";
      case "err": return "#f87171";
      case "info": return "#a0a0c0";
      default: return "#e0e0e0";
    }
  };

  return (
    <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", padding: "2rem", border: "1px solid var(--border)", backdropFilter: "blur(20px)" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5rem" }}>
        <div>
          <h2 style={{ color: "#fff", margin: 0, fontWeight: "900", fontSize: "22px" }}>REMOTE TERMINAL</h2>
          <p style={{ color: "var(--text-dim)", fontSize: "11px", letterSpacing: "1px", margin: "4px 0 0" }}>
            {sshTarget}
            <span style={{ marginLeft: "12px", color: status === "online" ? "var(--primary)" : status === "offline" ? "#f87171" : "var(--accent)" }}>
              ● {status === "online" ? "OPERATIONAL" : status === "offline" ? "UNREACHABLE" : "SCANNING…"}
            </span>
          </p>
        </div>
        <button onClick={openNativeTerminal}
          style={{ padding: "8px 18px", borderRadius: "100px", backgroundColor: "rgba(255,255,255,0.06)", color: "#fff", border: "1px solid var(--border)", fontSize: "12px", fontWeight: "700", cursor: "pointer" }}>
          OPEN NATIVE TERMINAL ↗
        </button>
      </div>

      {/* Quick actions */}
      <div style={{ display: "flex", flexWrap: "wrap", gap: "8px", marginBottom: "1.25rem" }}>
        {quickActions.map(qa => (
          <button key={qa.label} onClick={() => runSshCommand(qa.cmd)} disabled={running}
            style={{ padding: "6px 14px", borderRadius: "100px", backgroundColor: "rgba(255,255,255,0.05)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "10px", fontWeight: "800", letterSpacing: "1px", cursor: running ? "default" : "pointer", opacity: running ? 0.5 : 1 }}>
            {qa.label}
          </button>
        ))}
      </div>

      {/* Terminal output */}
      <div ref={termRef} style={{ backgroundColor: "#0a0a12", borderRadius: "12px", padding: "14px 16px", height: "340px", overflowY: "auto", fontFamily: "'Fira Code', 'Cascadia Code', monospace", fontSize: "12px", border: "1px solid var(--border)", marginBottom: "12px" }}>
        {lines.map((l, i) => (
          <div key={i} style={{ color: lineColor(l.type), marginBottom: "2px", whiteSpace: "pre-wrap", wordBreak: "break-all" }}>
            {l.text}
          </div>
        ))}
        {running && <div style={{ color: "var(--accent)" }}>▌</div>}
      </div>

      {/* Command input */}
      <form onSubmit={handleSubmit} style={{ display: "flex", gap: "8px" }}>
        <span style={{ fontFamily: "monospace", fontSize: "13px", color: "var(--primary)", lineHeight: "36px" }}>$</span>
        <input
          value={commandInput} onChange={e => setCommandInput(e.target.value)}
          disabled={running}
          placeholder="Enter remote command…"
          style={{ flex: 1, backgroundColor: "#0a0a12", border: "1px solid var(--border)", color: "#fff", borderRadius: "8px", padding: "8px 12px", fontSize: "12px", fontFamily: "monospace" }}
          autoFocus
        />
        <button type="submit" disabled={running || !commandInput.trim()}
          style={{ padding: "8px 20px", borderRadius: "8px", backgroundColor: "var(--primary)", color: "#000", border: "none", fontWeight: "800", fontSize: "12px", cursor: "pointer", opacity: running ? 0.5 : 1 }}>
          RUN
        </button>
        <button type="button" onClick={() => setLines([{ type: "info", text: "Terminal cleared." }])}
          style={{ padding: "8px 14px", borderRadius: "8px", backgroundColor: "rgba(255,255,255,0.05)", color: "var(--text-dim)", border: "1px solid var(--border)", fontSize: "12px", cursor: "pointer" }}>
          CLR
        </button>
      </form>
    </div>
  );
}
