import { useEffect, useState } from "react";
import { apiClient, type CreateTournamentRequest } from "../../services/api";
import { lamportsToUsd, lamportsToUsdInput, usdInputToLamports } from "../../services/sol";
import { useSolUsdRate } from "../../hooks/useSolUsdRate";

interface CreateTournamentProps {
  initialConfig?: CreateTournamentRequest;
  /** Receives the backend-assigned id of the new tournament. */
  onTournamentCreated: (tournamentId: number) => void;
  onCancel: () => void;
}

const STEPS = ["Basics", "Money", "Rules", "Review"] as const;
const CAPACITIES = [2, 4, 8, 16, 32, 64, 128, 256] as const;

export const suggestedName = (format: CreateTournamentRequest["format"], maxPlayers: number, swissRounds?: number) =>
  `${maxPlayers}-player ${format === "Swiss" ? `Swiss ${swissRounds || 5}-round` : "Single-elimination"}`;

// Mirrors the backend's default splits (signing/routes/tournament.rs) so a
// 2-player tournament never advertises a 3rd-place share it can't pay out.
const defaultSharesFor = (maxPlayers: number): CreateTournamentRequest["prize_shares"] => {
  if (maxPlayers <= 2) return [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0];
  if (maxPlayers <= 64) return [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0];
  if (maxPlayers === 128) return [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0];
  return [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300];
};

export default function CreateTournament({ onTournamentCreated, onCancel, initialConfig }: CreateTournamentProps) {
  const [step, setStep] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");
  const solUsdRate = useSolUsdRate();
  const [preflight, setPreflight] = useState<{ cost: any; wallet: any } | null>(null);
  const [preflightLoading, setPreflightLoading] = useState(false);

  const [form, setForm] = useState<CreateTournamentRequest>(() => initialConfig ?? {
    name: "",
    entry_fee_lamports: 0,
    platform_fee_lamports: 4_000_000,
    max_players: 16,
    format: "SingleElimination",
    swiss_rounds: 5,
    prize_shares: defaultSharesFor(16),
    winner_takes_all: false,
    kyc_required: false,
  });
  const [prizeLamports, setPrizeLamports] = useState(0);

  // USD text is kept separately so partial input such as "0." survives typing.
  const [entryUsd, setEntryUsd] = useState("");
  const [platformUsd, setPlatformUsd] = useState("");
  const [prizeUsd, setPrizeUsd] = useState("");
  useEffect(() => {
    if (solUsdRate == null) return;
    setEntryUsd(prev => prev || lamportsToUsdInput(form.entry_fee_lamports, solUsdRate));
    setPlatformUsd(prev => prev || lamportsToUsdInput(form.platform_fee_lamports || 0, solUsdRate));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [solUsdRate]);

  useEffect(() => {
    if (step !== STEPS.length - 1) return;
    let cancelled = false;
    setPreflightLoading(true);
    Promise.all([
      apiClient.predictTournamentCost({ max_players: form.max_players, format: form.format, average_moves: 40 }),
      apiClient.getOperatorAffordability(),
    ]).then(([cost, wallet]) => {
      if (!cancelled) setPreflight({ cost: cost.ok ? cost.data : null, wallet: wallet.ok ? wallet.data : null });
    }).finally(() => { if (!cancelled) setPreflightLoading(false); });
    return () => { cancelled = true; };
  }, [step, form.max_players, form.format]);

  const set = <K extends keyof CreateTournamentRequest>(field: K, value: CreateTournamentRequest[K]) =>
    setForm(prev => ({ ...prev, [field]: value }));

  const setCapacity = (capacity: number) => setForm(prev => {
    const untouched = JSON.stringify(prev.prize_shares) === JSON.stringify(defaultSharesFor(prev.max_players));
    return {
      ...prev,
      max_players: capacity as CreateTournamentRequest["max_players"],
      prize_shares: untouched ? defaultSharesFor(capacity) : prev.prize_shares,
      min_players: prev.min_players && prev.min_players > capacity ? capacity : prev.min_players,
    };
  });

  const name = form.name.trim() || suggestedName(form.format, form.max_players, form.swiss_rounds);
  const workingCapital = preflight?.cost?.working_capital_lamports ?? 0;
  const needed = workingCapital + prizeLamports;
  const affordable = Boolean(preflight?.cost && preflight?.wallet && preflight.wallet.balance_lamports >= needed);

  // Why the current step cannot continue, or null.
  const blocker = ((): string | null => {
    switch (step) {
      case 0:
        if (form.format === "Swiss" && (!form.swiss_rounds || form.swiss_rounds < 1)) return "Swiss needs at least one round.";
        if (form.min_players != null && (form.min_players < 2 || form.min_players > form.max_players)) return "Minimum players must be between 2 and the capacity.";
        return null;
      case 1:
        if (prizeShareTotalBps(form.prize_shares) > 10000) return "Prize split exceeds 100%.";
        if (form.entry_fee_lamports > 0 && prizeLamports <= 0) return "Paid tournaments need a guaranteed prize — players cannot register until one is locked.";
        return null;
      case 3:
        if (preflightLoading) return "Checking costs…";
        if (!preflight?.cost || !preflight?.wallet) return "Cost and wallet checks are unavailable; creation is blocked until they succeed.";
        if (!affordable) return "The VPS authority cannot cover setup plus the prize.";
        return null;
      default:
        return null;
    }
  })();

  const submit = async () => {
    if (blocker) return;
    setSubmitting(true);
    setError("");
    try {
      const { tournament_id: _ignored, ...config } = form;
      const created = await apiClient.createTournament({
        ...config,
        name,
        min_players: form.format === "Swiss" ? form.min_players : undefined,
        // The fee field is hidden for free events; never send a fee nobody saw.
        platform_fee_lamports: form.entry_fee_lamports > 0 ? form.platform_fee_lamports : 0,
      });
      if (!created.ok || !created.data) {
        setError(created.error?.message || "Failed to create tournament");
        return;
      }
      const id = Number(created.data.tournament_id);
      if (prizeLamports > 0) {
        // Locking the prize is a second transaction; if it fails, the detail
        // page's next-step panel offers it again.
        await apiClient.fundTournamentPrize(id, prizeLamports);
      }
      onTournamentCreated(id);
    } catch {
      setError("Network error creating tournament");
    } finally {
      setSubmitting(false);
    }
  };

  const usdField = (label: string, value: string, setValue: (v: string) => void, lamports: number, onLamports: (l: number) => void, placeholder: string) => (
    <div>
      <label style={labelStyle}>{label}</label>
      <input type="number" step="0.01" min="0" value={value} disabled={solUsdRate == null}
        onChange={e => { setValue(e.target.value); onLamports(usdInputToLamports(e.target.value, solUsdRate)); }}
        style={{ ...inputStyle, ...(solUsdRate == null ? disabledInputStyle : {}) }}
        placeholder={solUsdRate == null ? "Loading SOL/USD rate…" : placeholder} aria-label={label} />
      <FeeEquivalent lamports={lamports} solUsdRate={solUsdRate} />
    </div>
  );

  const steps = [
    // Basics
    <>
      <div>
        <label style={labelStyle}>NAME</label>
        <input value={form.name} onChange={e => set("name", e.target.value)} placeholder={suggestedName(form.format, form.max_players, form.swiss_rounds)} style={inputStyle} aria-label="Tournament name" />
        <Hint>Leave blank to use the suggested name. The tournament ID is assigned automatically.</Hint>
      </div>
      <div>
        <label style={labelStyle}>FORMAT</label>
        <div style={{ display: "flex", gap: "8px" }}>
          {(["SingleElimination", "Swiss"] as const).map(f => (
            <button key={f} type="button" onClick={() => set("format", f)} style={choiceStyle(form.format === f)}>
              {f === "Swiss" ? "SWISS" : "SINGLE ELIMINATION"}
            </button>
          ))}
        </div>
        <Hint>{form.format === "Swiss"
          ? "Everyone plays every round; the best score wins. Starts once the minimum has joined."
          : "Knock-out bracket. Starts automatically the moment it is full."}</Hint>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: form.format === "Swiss" ? "1fr 1fr 1fr" : "1fr", gap: "1rem" }}>
        <div>
          <label style={labelStyle}>{form.format === "Swiss" ? "CAPACITY" : "BRACKET SIZE"}</label>
          <select value={form.max_players} onChange={e => setCapacity(parseInt(e.target.value, 10))} style={inputStyle} aria-label="Capacity">
            {CAPACITIES.map(c => <option key={c} value={c}>{c} players</option>)}
          </select>
        </div>
        {form.format === "Swiss" && <>
          <div>
            <label style={labelStyle}>ROUNDS</label>
            <input type="number" min={1} max={20} value={form.swiss_rounds ?? 5} onChange={e => set("swiss_rounds", parseInt(e.target.value, 10) || undefined)} style={inputStyle} aria-label="Rounds" />
          </div>
          <div>
            <label style={labelStyle}>START AT (MIN PLAYERS)</label>
            <input type="number" min={2} max={form.max_players} value={form.min_players ?? ""} placeholder={String(Math.min(form.max_players, 8))}
              onChange={e => set("min_players", e.target.value ? parseInt(e.target.value, 10) : undefined)} style={inputStyle} aria-label="Minimum players" />
          </div>
        </>}
      </div>
    </>,

    // Money
    <>
      {usdField("ENTRY FEE (USD)", entryUsd, setEntryUsd, form.entry_fee_lamports, l => set("entry_fee_lamports", l), "0 for a free tournament")}
      {form.entry_fee_lamports > 0 && usdField("PLATFORM FEE (USD)", platformUsd, setPlatformUsd, form.platform_fee_lamports || 0, l => set("platform_fee_lamports", l), "Service fee, e.g. 0.50")}
      {usdField(form.entry_fee_lamports > 0 ? "GUARANTEED PRIZE (USD) — REQUIRED" : "GUARANTEED PRIZE (USD) — OPTIONAL", prizeUsd, setPrizeUsd, prizeLamports, setPrizeLamports, "Locked from the VPS authority when you create")}
      <label style={{ ...labelStyle, display: "flex", alignItems: "center", cursor: "pointer" }}>
        <input type="checkbox" checked={form.winner_takes_all} onChange={e => set("winner_takes_all", e.target.checked)} style={{ marginRight: "0.75rem", accentColor: "var(--primary)" }} />
        WINNER TAKES ALL
      </label>
      {!form.winner_takes_all && (
        <div>
          <label style={labelStyle}>PRIZE SPLIT (%)</label>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: "0.75rem" }}>
            {[0, 1, 2, 3, 4].map(i => (
              <input key={i} type="number" step="0.01" min="0" max="100" aria-label={`Rank ${i + 1} share`}
                value={bpsToPercentInput(form.prize_shares?.[i] ?? 0)}
                onChange={e => {
                  const shares = [...(form.prize_shares ?? defaultSharesFor(form.max_players)!)] as NonNullable<CreateTournamentRequest["prize_shares"]>;
                  shares[i] = percentInputToBps(e.target.value);
                  set("prize_shares", shares);
                }}
                style={{ ...inputStyle, padding: "0.5rem" }} placeholder={`#${i + 1}`} />
            ))}
          </div>
          <Hint>Ranks 1–5. Total {(prizeShareTotalBps(form.prize_shares) / 100).toFixed(2)}%; the remainder goes to the treasury.</Hint>
        </div>
      )}
    </>,

    // Rules
    <>
      <div>
        <label style={labelStyle}>ELO RANGE (OPTIONAL)</label>
        <div style={{ display: "flex", gap: "1rem" }}>
          <input type="number" value={form.elo_min ?? ""} onChange={e => set("elo_min", e.target.value ? parseInt(e.target.value, 10) : undefined)} style={inputStyle} placeholder="Min ELO" aria-label="Minimum ELO" />
          <input type="number" value={form.elo_max ?? ""} onChange={e => set("elo_max", e.target.value ? parseInt(e.target.value, 10) : undefined)} style={inputStyle} placeholder="Max ELO" aria-label="Maximum ELO" />
        </div>
      </div>
      <label style={{ ...labelStyle, display: "flex", alignItems: "center", cursor: "pointer" }}>
        <input type="checkbox" checked={form.kyc_required} onChange={e => set("kyc_required", e.target.checked)} style={{ marginRight: "0.75rem", accentColor: "var(--primary)" }} />
        REQUIRE KYC / CACF CLEARANCE
      </label>
      <div>
        <label style={labelStyle}>JOIN PASSWORD (OPTIONAL — MAKES IT PRIVATE)</label>
        <input type="text" value={form.password ?? ""} onChange={e => set("password", e.target.value || undefined)} placeholder="Leave blank for a public tournament" style={inputStyle} aria-label="Join password" />
        <Hint>Stored only as an argon2 hash.</Hint>
      </div>
      <div>
        <label style={labelStyle}>SCHEDULED START (OPTIONAL)</label>
        <input type="datetime-local" aria-label="Scheduled start"
          value={form.scheduled_at ? toLocalInput(form.scheduled_at) : ""}
          onChange={e => set("scheduled_at", e.target.value ? Math.floor(new Date(e.target.value).getTime() / 1000) : undefined)}
          style={inputStyle} />
        <Hint>{form.scheduled_at
          ? "Starts at this time if enough players have joined; otherwise it is cancelled and refunded after a grace period."
          : "No schedule: registration opens immediately and it starts as described in Basics."}</Hint>
      </div>
    </>,

    // Review
    <>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
        <Metric label="NAME" value={name} />
        <Metric label="FORMAT" value={form.format === "Swiss" ? `Swiss · ${form.swiss_rounds} rounds` : "Single elimination"} />
        <Metric label="PLAYERS" value={form.format === "Swiss" ? `${form.min_players ?? Math.min(form.max_players, 8)}–${form.max_players}` : `${form.max_players} (starts when full)`} />
        <Metric label="ENTRY FEE" value={form.entry_fee_lamports > 0 ? sol(form.entry_fee_lamports) : "Free"} />
        <Metric label="GUARANTEED PRIZE" value={prizeLamports > 0 ? sol(prizeLamports) : "None"} />
        <Metric label="START" value={form.scheduled_at ? new Date(form.scheduled_at * 1000).toLocaleString() : "When ready"} />
      </div>
      {preflightLoading && <div style={{ color: "var(--text-dim)" }}>Checking costs…</div>}
      {!preflightLoading && preflight?.cost && preflight?.wallet && (
        <div style={{ padding: "1rem", border: `1px solid ${affordable ? "#4ade80" : "#f87171"}`, borderRadius: "12px", background: affordable ? "rgba(74,222,128,0.08)" : "rgba(248,113,113,0.08)" }}>
          <strong style={{ color: affordable ? "#4ade80" : "#f87171" }}>{affordable ? "VPS AUTHORITY CAN FUND THIS" : "INSUFFICIENT VPS BALANCE"}</strong>
          <div style={{ marginTop: "6px", fontSize: "12px", color: "var(--text-dim)" }}>
            Needs {sol(needed)} ({sol(workingCapital)} working capital{prizeLamports > 0 ? ` + ${sol(prizeLamports)} prize` : ""}); balance {sol(preflight.wallet.balance_lamports)}.
          </div>
          <Hint>Cost estimate assumes 40 moves per game (confidence: {String(preflight.cost.er_cost_confidence)}).</Hint>
        </div>
      )}
    </>,
  ];

  const last = step === STEPS.length - 1;
  return (
    <div style={{ backgroundColor: "var(--surface)", borderRadius: "24px", padding: "2.5rem", border: "1px solid var(--border)", width: "100%", maxWidth: "800px", margin: "0 auto" }}>
      <div style={{ display: "flex", gap: "8px", marginBottom: "2rem" }}>
        {STEPS.map((label, i) => (
          <button key={label} type="button" disabled={i > step} onClick={() => setStep(i)}
            style={{ flex: 1, background: "none", border: "none", borderTop: `4px solid ${i <= step ? "var(--primary)" : "rgba(255,255,255,0.08)"}`, color: i === step ? "#fff" : "var(--text-dim)", fontSize: "11px", fontWeight: 800, letterSpacing: "1px", paddingTop: "8px", cursor: i < step ? "pointer" : "default" }}>
            {i + 1}. {label.toUpperCase()}
          </button>
        ))}
      </div>

      <h2 style={{ color: "#fff", fontSize: "24px", fontWeight: 800, marginTop: 0 }}>{initialConfig ? "Duplicate tournament" : "New tournament"} · {STEPS[step]}</h2>
      <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>{steps[step]}</div>

      {(error || (blocker && step === STEPS.length - 1 && !preflightLoading)) && (
        <div role="alert" style={{ marginTop: "1.5rem", padding: "1rem", backgroundColor: "rgba(239, 68, 68, 0.1)", border: "1px solid #ef4444", borderRadius: "12px", color: "#ef4444", fontSize: "13px" }}>
          {error || blocker}
        </div>
      )}
      {blocker && step < STEPS.length - 1 && <Hint>{blocker}</Hint>}

      <div style={{ display: "flex", justifyContent: "space-between", marginTop: "2.5rem", paddingTop: "1.5rem", borderTop: "1px solid var(--border)" }}>
        <button type="button" onClick={() => (step === 0 ? onCancel() : setStep(step - 1))} disabled={submitting}
          style={{ padding: "0.85rem 2rem", borderRadius: "100px", backgroundColor: "transparent", color: "var(--text-dim)", border: "1px solid var(--border)" }}>
          {step === 0 ? "CANCEL" : "BACK"}
        </button>
        <button type="button" className="primary" disabled={Boolean(blocker) || submitting}
          onClick={() => (last ? void submit() : setStep(step + 1))}
          style={{ padding: "0.85rem 2.5rem", borderRadius: "100px", opacity: blocker || submitting ? 0.5 : 1 }}>
          {last ? (submitting ? "CREATING ON-CHAIN…" : prizeLamports > 0 ? "CREATE & LOCK PRIZE" : "CREATE TOURNAMENT") : "CONTINUE"}
        </button>
      </div>
    </div>
  );
}

const sol = (lamports: number) => `${(lamports / 1e9).toFixed(4)} SOL`;
const toLocalInput = (unix: number) => {
  const d = new Date(unix * 1000);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60_000).toISOString().slice(0, 16);
};

const Hint = ({ children }: { children: React.ReactNode }) => (
  <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "6px" }}>{children}</div>
);

const Metric = ({ label, value }: { label: string; value: string }) => (
  <div style={{ padding: "12px", background: "rgba(255,255,255,0.04)", border: "1px solid var(--border)", borderRadius: "8px" }}>
    <div style={{ color: "var(--text-dim)", fontSize: "10px", fontWeight: 800 }}>{label}</div>
    <div style={{ color: "#fff", fontSize: "14px", fontWeight: 800, marginTop: "4px" }}>{value}</div>
  </div>
);

/** Shows lamports plus a USD equivalent once a rate has loaded. */
const FeeEquivalent = ({ lamports, solUsdRate }: { lamports: number; solUsdRate: number | null }) => {
  const usd = lamportsToUsd(lamports, solUsdRate);
  return (
    <div style={{ color: "var(--text-dim)", fontSize: "11px", marginTop: "6px", fontWeight: 700 }}>
      {sol(lamports)}{usd != null ? ` · ≈ $${usd.toFixed(2)}` : ""}
    </div>
  );
};

/** Wire prize shares use basis points (10000 = 100%); the UI edits percentages. */
const bpsToPercentInput = (bps: number): string => (bps === 0 ? "0" : String(bps / 100));
const percentInputToBps = (raw: string): number => {
  const pct = parseFloat(raw);
  return Number.isFinite(pct) && pct >= 0 ? Math.round(pct * 100) : 0;
};
const prizeShareTotalBps = (shares?: number[]): number => (shares || []).reduce((sum, v) => sum + (v || 0), 0);

const choiceStyle = (active: boolean): React.CSSProperties => ({
  flex: 1, padding: "0.9rem", borderRadius: "12px", fontSize: "12px", fontWeight: 800, cursor: "pointer",
  border: `1px solid ${active ? "var(--primary)" : "var(--border)"}`,
  background: active ? "rgba(173,92,47,0.15)" : "rgba(0,0,0,0.2)",
  color: active ? "var(--primary)" : "var(--text-dim)",
});

const labelStyle: React.CSSProperties = { display: "block", color: "var(--text-dim)", fontSize: "11px", fontWeight: 800, letterSpacing: "1.5px", marginBottom: "10px" };
const inputStyle: React.CSSProperties = { width: "100%", padding: "1rem 1.25rem", backgroundColor: "rgba(0, 0, 0, 0.2)", border: "1px solid var(--border)", borderRadius: "16px", color: "#ffffff", fontSize: "14px", outline: "none" };
const disabledInputStyle: React.CSSProperties = { opacity: 0.5, cursor: "not-allowed" };
