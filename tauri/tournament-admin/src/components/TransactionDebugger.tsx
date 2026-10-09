import { useMemo, useState } from "react";
import { apiClient } from "../services/api";

type DebugResult = {
  signature: string;
  debug_info: any;
  formatted?: string;
};

const sigPattern = /^[1-9A-HJ-NP-Za-km-z]{64,88}$/;

export default function TransactionDebugger({ initialSignature = "" }: { initialSignature?: string }) {
  const [signature, setSignature] = useState(initialSignature);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<DebugResult | null>(null);
  const [showRaw, setShowRaw] = useState(false);

  const trimmed = signature.trim();
  const valid = trimmed.length === 0 || sigPattern.test(trimmed);
  const info = result?.debug_info;
  const failedIx = info?.failing_instruction;
  const statusColor = info?.success ? "#4ade80" : info ? "#f87171" : "var(--accent)";

  const rawJson = useMemo(() => result ? JSON.stringify(result, null, 2) : "", [result]);

  const analyze = async (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!sigPattern.test(trimmed) || loading) return;
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const response = await apiClient.debugTransaction(trimmed);
      if (!response.ok) {
        setError(response.error?.message ?? "Transaction lookup failed.");
      } else {
        setResult(response.data);
      }
    } catch (err: any) {
      setError(err?.message ?? String(err));
    } finally {
      setLoading(false);
    }
  };

  const copyRaw = async () => {
    if (!rawJson) return;
    await navigator.clipboard.writeText(rawJson);
  };

  return (
    <div style={{ display: "grid", gap: "1rem" }}>
      <section style={panelStyle}>
        <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "flex-start" }}>
          <div>
            <h2 style={titleStyle}>TRANSACTION DEBUGGER</h2>
            <p style={mutedStyle}>Paste a Solana signature. The backend queries Triton RPC and returns the evidence chain.</p>
          </div>
          <div style={{ ...pillStyle, color: statusColor }}>
            {loading ? "ANALYZING" : info ? (info.success ? "LANDED" : "FAILED") : "SIGNATURE FIRST"}
          </div>
        </div>

        <form onSubmit={analyze} style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", gap: "0.75rem", marginTop: "1rem" }}>
          <input
            value={signature}
            onChange={(e) => setSignature(e.target.value)}
            placeholder="Paste transaction signature"
            spellCheck={false}
            style={{
              minWidth: 0,
              backgroundColor: "#07150f",
              border: `1px solid ${valid ? "var(--border)" : "#f87171"}`,
              borderRadius: "8px",
              color: "var(--text)",
              fontFamily: "'Fira Code', 'Cascadia Code', monospace",
              fontSize: "13px",
              padding: "0.8rem 0.9rem",
            }}
          />
          <button className="primary" disabled={!sigPattern.test(trimmed) || loading} style={{ borderRadius: "8px", opacity: !sigPattern.test(trimmed) || loading ? 0.55 : 1 }}>
            {loading ? "ANALYZING" : "ANALYZE"}
          </button>
        </form>
        {!valid && <div style={{ ...mutedStyle, color: "#f87171", marginTop: "0.5rem" }}>That does not look like a Solana transaction signature.</div>}
        {error && <div style={errorStyle}>{error}</div>}
      </section>

      {!info && (
        <section style={panelStyle}>
          <h3 style={sectionTitleStyle}>DEBUG ORDER</h3>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: "0.75rem", marginTop: "0.75rem" }}>
            {["Get signature", "Check landed/status", "Identify failing instruction", "Follow CPI tree", "Inspect accounts", "Classify root cause"].map((label, idx) => (
              <div key={label} style={miniCardStyle}>
                <div style={{ color: "var(--accent)", fontSize: "11px", fontWeight: 900 }}>STEP {idx + 1}</div>
                <div style={{ marginTop: "0.25rem", color: "#fff", fontWeight: 800 }}>{label}</div>
              </div>
            ))}
          </div>
        </section>
      )}

      {info && (
        <>
          <section style={panelStyle}>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: "0.75rem" }}>
              <Metric label="Status" value={info.success ? "Success" : "Failed"} color={statusColor} />
              <Metric label="Slot" value={String(info.slot ?? "-")} />
              <Metric label="Compute" value={info.compute_units_consumed ? String(info.compute_units_consumed) : "-"} />
              <Metric label="Fee" value={`${info.fee_paid ?? 0} lamports`} />
              <Metric label="Slot age" value={info.freshness?.slot_age == null ? "-" : `${info.freshness.slot_age} slots`} />
              <Metric label="ALT" value={info.metadata?.uses_address_lookup_tables ? "Used" : "Not used"} />
            </div>
          </section>

          <section style={panelStyle}>
            <h3 style={sectionTitleStyle}>ROOT CAUSE</h3>
            <div style={{ marginTop: "0.75rem", display: "grid", gap: "0.75rem" }}>
              <div style={{ ...pillStyle, justifySelf: "start", color: statusColor }}>{info.root_cause?.category ?? "unknown"}</div>
              <div style={{ color: "#fff", fontSize: "15px", lineHeight: 1.45 }}>{info.root_cause?.summary}</div>
              <List items={info.root_cause?.evidence ?? []} empty="No evidence returned." />
              <h4 style={subTitleStyle}>Recommended Actions</h4>
              <List items={info.recommended_actions ?? []} empty="No action returned." />
            </div>
          </section>

          <section style={panelStyle}>
            <h3 style={sectionTitleStyle}>FAILING INSTRUCTION</h3>
            {failedIx ? (
              <div style={{ display: "grid", gap: "0.75rem", marginTop: "0.75rem" }}>
                <div style={miniCardStyle}>
                  <div style={{ color: "#fff", fontWeight: 900 }}>#{failedIx.index} {failedIx.program_label}</div>
                  <Mono>{failedIx.program_id}</Mono>
                  <div style={mutedStyle}>Discriminator: {failedIx.discriminator ?? "n/a"}</div>
                  {failedIx.error && <div style={{ color: "#f87171", marginTop: "0.5rem", fontSize: "12px" }}>{failedIx.error}</div>}
                </div>
                <Table
                  headers={["#", "Account", "Owner", "Signer", "Writable"]}
                  rows={(failedIx.accounts ?? []).map((a: any) => [
                    a.index,
                    short(a.pubkey),
                    a.owner_label ?? short(a.owner),
                    a.signer ? "yes" : "no",
                    a.writable ? "yes" : "no",
                  ])}
                />
              </div>
            ) : (
              <div style={mutedStyle}>No failing instruction was reported.</div>
            )}
          </section>

          <section style={panelStyle}>
            <h3 style={sectionTitleStyle}>CPI TREE</h3>
            <div style={{ display: "grid", gap: "0.4rem", marginTop: "0.75rem" }}>
              {(info.cpi_tree ?? []).length === 0 ? <div style={mutedStyle}>No CPI frames parsed from logs.</div> : info.cpi_tree.map((f: any, idx: number) => (
                <div key={`${f.message}-${idx}`} style={{ ...logLineStyle, marginLeft: `${Math.min(f.depth || 0, 5) * 16}px`, borderColor: f.status === "failed" ? "rgba(248,113,113,0.35)" : "var(--border)" }}>
                  <span style={{ color: f.status === "failed" ? "#f87171" : "var(--accent)", marginRight: "0.5rem" }}>{f.status}</span>
                  {f.program_label} <span style={{ color: "var(--text-dim)" }}>{short(f.program_id)}</span>
                </div>
              ))}
            </div>
          </section>

          <section style={panelStyle}>
            <h3 style={sectionTitleStyle}>OUTER INSTRUCTIONS</h3>
            <Table
              headers={["#", "Program", "Program ID", "Accounts", "Discriminator"]}
              rows={(info.outer_instructions ?? []).map((ix: any) => [
                ix.index,
                ix.program_label,
                short(ix.program_id),
                ix.account_indexes?.length ?? 0,
                ix.discriminator ?? "n/a",
              ])}
            />
          </section>

          <section style={panelStyle}>
            <h3 style={sectionTitleStyle}>ACCOUNTS</h3>
            <Table
              headers={["#", "Account", "Owner", "Lamports delta", "Data", "Flags"]}
              rows={(info.accounts ?? []).slice(0, 32).map((a: any) => [
                a.index,
                short(a.pubkey),
                a.owner_label ?? short(a.owner),
                a.lamports_change ?? "-",
                a.data_len == null ? "-" : `${a.data_len} B`,
                `${a.signer ? "S" : "-"} ${a.writable ? "W" : "-"}`,
              ])}
            />
          </section>

          <section style={panelStyle}>
            <h3 style={sectionTitleStyle}>LOGS</h3>
            <div style={{ display: "grid", gap: "0.35rem", marginTop: "0.75rem", maxHeight: "360px", overflow: "auto" }}>
              {(info.logs ?? []).length === 0 ? <div style={mutedStyle}>No logs returned.</div> : info.logs.map((line: string, idx: number) => (
                <div key={`${idx}-${line}`} style={logLineStyle}>{line}</div>
              ))}
            </div>
          </section>

          <section style={panelStyle}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "0.75rem" }}>
              <h3 style={sectionTitleStyle}>RAW JSON</h3>
              <div style={{ display: "flex", gap: "0.5rem" }}>
                <button onClick={() => setShowRaw(!showRaw)} style={smallButtonStyle}>{showRaw ? "HIDE" : "SHOW"}</button>
                <button onClick={copyRaw} style={smallButtonStyle}>COPY</button>
              </div>
            </div>
            {showRaw && <pre style={rawStyle}>{rawJson}</pre>}
          </section>
        </>
      )}
    </div>
  );
}

function Metric({ label, value, color = "#fff" }: { label: string; value: string; color?: string }) {
  return (
    <div style={miniCardStyle}>
      <div style={{ color: "var(--text-dim)", fontSize: "10px", fontWeight: 900, letterSpacing: "1px" }}>{label.toUpperCase()}</div>
      <div style={{ color, fontSize: "18px", fontWeight: 900, marginTop: "0.25rem", wordBreak: "break-word" }}>{value}</div>
    </div>
  );
}

function List({ items, empty }: { items: string[]; empty: string }) {
  if (!items.length) return <div style={mutedStyle}>{empty}</div>;
  return (
    <div style={{ display: "grid", gap: "0.4rem" }}>
      {items.map((item, idx) => <div key={`${idx}-${item}`} style={logLineStyle}>{item}</div>)}
    </div>
  );
}

function Table({ headers, rows }: { headers: string[]; rows: any[][] }) {
  if (!rows.length) return <div style={{ ...mutedStyle, marginTop: "0.75rem" }}>No rows.</div>;
  return (
    <div style={{ overflowX: "auto", marginTop: "0.75rem" }}>
      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "12px" }}>
        <thead>
          <tr>
            {headers.map((h) => <th key={h} style={thStyle}>{h}</th>)}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, idx) => (
            <tr key={idx}>
              {row.map((cell, cellIdx) => <td key={cellIdx} style={tdStyle}>{String(cell ?? "-")}</td>)}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Mono({ children }: { children: React.ReactNode }) {
  return <div style={{ fontFamily: "'Fira Code', 'Cascadia Code', monospace", color: "var(--text-dim)", fontSize: "12px", wordBreak: "break-all", marginTop: "0.25rem" }}>{children}</div>;
}

function short(value?: string | null) {
  if (!value) return "-";
  return value.length > 18 ? `${value.slice(0, 8)}...${value.slice(-6)}` : value;
}

const panelStyle: React.CSSProperties = {
  backgroundColor: "var(--surface)",
  border: "1px solid var(--border)",
  borderRadius: "8px",
  padding: "1rem",
};

const miniCardStyle: React.CSSProperties = {
  backgroundColor: "rgba(255,255,255,0.035)",
  border: "1px solid var(--border)",
  borderRadius: "8px",
  padding: "0.85rem",
};

const titleStyle: React.CSSProperties = {
  color: "#fff",
  margin: 0,
  fontWeight: 900,
  fontSize: "22px",
};

const sectionTitleStyle: React.CSSProperties = {
  color: "#fff",
  margin: 0,
  fontWeight: 900,
  fontSize: "14px",
  letterSpacing: "1px",
};

const subTitleStyle: React.CSSProperties = {
  color: "#fff",
  margin: "0.25rem 0 0",
  fontWeight: 900,
  fontSize: "12px",
};

const mutedStyle: React.CSSProperties = {
  color: "var(--text-dim)",
  fontSize: "12px",
};

const pillStyle: React.CSSProperties = {
  border: "1px solid var(--border)",
  borderRadius: "999px",
  padding: "0.4rem 0.7rem",
  fontSize: "11px",
  fontWeight: 900,
  letterSpacing: "1px",
  backgroundColor: "rgba(255,255,255,0.04)",
  whiteSpace: "nowrap",
};

const errorStyle: React.CSSProperties = {
  color: "#f87171",
  backgroundColor: "rgba(248,113,113,0.08)",
  border: "1px solid rgba(248,113,113,0.25)",
  borderRadius: "8px",
  padding: "0.75rem",
  marginTop: "0.75rem",
  fontSize: "12px",
};

const logLineStyle: React.CSSProperties = {
  backgroundColor: "#07150f",
  border: "1px solid var(--border)",
  borderRadius: "6px",
  padding: "0.55rem 0.65rem",
  color: "#d8dedb",
  fontFamily: "'Fira Code', 'Cascadia Code', monospace",
  fontSize: "11px",
  wordBreak: "break-word",
};

const thStyle: React.CSSProperties = {
  textAlign: "left",
  color: "var(--text-dim)",
  fontSize: "10px",
  letterSpacing: "1px",
  textTransform: "uppercase",
  padding: "0.6rem",
  borderBottom: "1px solid var(--border)",
};

const tdStyle: React.CSSProperties = {
  color: "#e5e7eb",
  padding: "0.6rem",
  borderBottom: "1px solid rgba(255,255,255,0.05)",
  fontFamily: "'Fira Code', 'Cascadia Code', monospace",
  whiteSpace: "nowrap",
};

const smallButtonStyle: React.CSSProperties = {
  borderRadius: "8px",
  padding: "0.45rem 0.75rem",
  fontSize: "11px",
};

const rawStyle: React.CSSProperties = {
  marginTop: "0.75rem",
  maxHeight: "420px",
  overflow: "auto",
  backgroundColor: "#07150f",
  border: "1px solid var(--border)",
  borderRadius: "8px",
  padding: "0.85rem",
  color: "#d8dedb",
  fontSize: "11px",
};
