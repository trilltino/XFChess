import { useEffect, useState } from "react";
import { apiClient, type CreateTournamentRequest } from "../../services/api";
import { lamportsToUsd, lamportsToUsdInput, usdInputToLamports } from "../../services/sol";
import { useSolUsdRate } from "../../hooks/useSolUsdRate";

interface CreateTournamentProps {
  onTournamentCreated: () => void;
  onCancel: () => void;
}

export default function CreateTournament({ onTournamentCreated, onCancel }: CreateTournamentProps) {
  const [currentStep, setCurrentStep] = useState(1);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const solUsdRate = useSolUsdRate();

  // Form state
  const [formData, setFormData] = useState<CreateTournamentRequest>({
    tournament_id: 0,
    name: "",
    entry_fee_lamports: 0,
    platform_fee_lamports: 4000000,
    max_players: 16,
    format: "SingleElimination",
    swiss_rounds: 5,
    elo_min: undefined,
    elo_max: undefined,
    min_players: undefined,
    prize_shares: [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
    winner_takes_all: false,
    scheduled_at: undefined,
    kyc_required: false,
  });

  // Text mirrors of the two lamport fields, kept as separate string state so
  // the input can hold in-progress text ("0.", "0.00") that parseFloat would
  // otherwise mangle if it round-tripped through the lamport value on every
  // keystroke. USD is the only unit shown; the inputs stay disabled until
  // the live rate loads since there's no lamport value to compute without it.
  const [entryFeeUsdInput, setEntryFeeUsdInput] = useState("");
  const [platformFeeUsdInput, setPlatformFeeUsdInput] = useState("");

  // Populate the USD mirrors once the live rate first loads — the initial
  // lamport defaults above were set before any rate was known. Guarded by
  // `prev ||` so a later rate refresh doesn't clobber whatever the admin
  // has since typed.
  useEffect(() => {
    if (solUsdRate == null) return;
    setEntryFeeUsdInput(prev => prev || lamportsToUsdInput(formData.entry_fee_lamports, solUsdRate));
    setPlatformFeeUsdInput(prev => prev || lamportsToUsdInput(formData.platform_fee_lamports || 0, solUsdRate));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [solUsdRate]);

  const updateFormData = (field: keyof CreateTournamentRequest, value: any) => {
    setFormData(prev => ({ ...prev, [field]: value }));
  };

  const validateStep = () => {
    switch (currentStep) {
      case 1:
        return formData.tournament_id > 0 && formData.name.trim() !== "";
      case 2:
        return formData.entry_fee_lamports >= 0 && prizeShareTotalBps(formData.prize_shares) <= 10000;
      case 3:
        return true;
      case 4:
        return true;
      default:
        return false;
    }
  };

  const nextStep = () => {
    if (validateStep() && currentStep < 4) {
      setCurrentStep(currentStep + 1);
    }
  };

  const prevStep = () => {
    if (currentStep > 1) {
      setCurrentStep(currentStep - 1);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validateStep()) return;

    setLoading(true);
    setError("");

    try {
      const response = await apiClient.createTournament(formData);
      if (response.ok) {
        onTournamentCreated();
      } else {
        setError(response.error?.message || "Failed to create tournament");
      }
    } catch (err) {
      setError("Network error creating tournament");
    } finally {
      setLoading(false);
    }
  };

  // Mirrors the backend's default splits (signing/routes/tournament.rs) so a
  // 2-player tournament never advertises a 3rd-place share it can't pay out.
  const defaultSharesFor = (maxPlayers: number): number[] => {
    if (maxPlayers <= 2) return [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0];
    if (maxPlayers <= 64) return [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0];
    if (maxPlayers === 128) return [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0];
    return [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300];
  };

  const updateMaxPlayers = (capacity: number) => {
    setFormData(prev => {
      const untouched =
        JSON.stringify(prev.prize_shares) === JSON.stringify(defaultSharesFor(prev.max_players));
      return {
        ...prev,
        max_players: capacity as CreateTournamentRequest["max_players"],
        prize_shares: (untouched ? defaultSharesFor(capacity) : prev.prize_shares) as any,
      };
    });
  };

  const renderStep1 = () => (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <SectionTitle>Basic Infrastructure</SectionTitle>
      
      <div>
        <label style={labelStyle}>TOURNAMENT ID <span style={{ color: "var(--primary)" }}>*</span></label>
        <input
          type="number"
          value={formData.tournament_id || ""}
          onChange={(e) => updateFormData("tournament_id", parseInt(e.target.value) || 0)}
          style={inputStyle}
          placeholder="Unique sequence number"
        />
      </div>

      <div>
        <label style={labelStyle}>NAME <span style={{ color: "var(--primary)" }}>*</span></label>
        <input
          type="text"
          value={formData.name}
          onChange={(e) => updateFormData("name", e.target.value)}
          style={inputStyle}
          placeholder="Match designator"
        />
      </div>
    </div>
  );

  const renderStep2 = () => (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <SectionTitle>Economics & Rewards</SectionTitle>
      
      <div>
        <label style={labelStyle}>ENTRY FEE (USD)</label>
        <input
          type="number"
          step="0.01"
          min="0"
          value={entryFeeUsdInput}
          disabled={solUsdRate == null}
          onChange={(e) => {
            setEntryFeeUsdInput(e.target.value);
            updateFormData("entry_fee_lamports", usdInputToLamports(e.target.value, solUsdRate));
          }}
          style={{ ...inputStyle, ...(solUsdRate == null ? disabledInputStyle : {}) }}
          placeholder={solUsdRate == null ? "Loading rate…" : "0 for FREE tournament"}
        />
        <FeeEquivalent lamports={formData.entry_fee_lamports} solUsdRate={solUsdRate} color="var(--accent)" />
      </div>

      <div>
        <label style={labelStyle}>PLATFORM FEE (USD)</label>
        <input
          type="number"
          step="0.01"
          min="0"
          value={platformFeeUsdInput}
          disabled={solUsdRate == null}
          onChange={(e) => {
            setPlatformFeeUsdInput(e.target.value);
            updateFormData("platform_fee_lamports", usdInputToLamports(e.target.value, solUsdRate));
          }}
          style={{ ...inputStyle, ...(solUsdRate == null ? disabledInputStyle : {}) }}
          placeholder={solUsdRate == null ? "Loading rate…" : "Service fee (e.g. $0.50)"}
        />
        <FeeEquivalent lamports={formData.platform_fee_lamports || 0} solUsdRate={solUsdRate} color="var(--primary)" />
      </div>

      <div style={{ 
        padding: "1rem", 
        backgroundColor: "rgba(255,255,255,0.02)", 
        borderRadius: "16px",
        border: "1px solid var(--border)"
      }}>
        <label style={{ ...labelStyle, display: "flex", alignItems: "center", cursor: "pointer", margin: 0 }}>
          <input
            type="checkbox"
            checked={formData.winner_takes_all}
            onChange={(e) => updateFormData("winner_takes_all", e.target.checked)}
            style={{ marginRight: "0.75rem", accentColor: "var(--primary)", width: "16px", height: "16px" }}
          />
          WINNER TAKES ALL [BATTLE ROYALE]
        </label>
      </div>

      {!formData.winner_takes_all && (
        <div style={{ 
          padding: "1.5rem", 
          backgroundColor: "rgba(0,0,0,0.2)", 
          borderRadius: "16px",
          border: "1px solid var(--border)"
        }}>
          <label style={labelStyle}>PRIZE DISTRIBUTION [%]</label>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: "1rem" }}>
            {[1, 2, 3, 4, 5].map(i => (
              <div key={i}>
                <label style={{ color: "var(--text-dim)", fontSize: "10px", marginBottom: "4px", display: "block" }}>
                  RANK {i}
                </label>
                <div style={{ position: "relative" }}>
                  <input
                    type="number"
                    step="0.01"
                    min="0"
                    max="100"
                    value={bpsToPercentInput(formData.prize_shares?.[i - 1] ?? 0)}
                    onChange={(e) => {
                      const newShares = [...(formData.prize_shares || [])];
                      newShares[i - 1] = percentInputToBps(e.target.value);
                      updateFormData("prize_shares", newShares as any);
                    }}
                    style={{ ...inputStyle, padding: "0.5rem 1.5rem 0.5rem 0.5rem" }}
                  />
                  <span style={{ position: "absolute", right: "10px", top: "50%", transform: "translateY(-50%)", color: "var(--text-dim)", fontSize: "12px", pointerEvents: "none" }}>%</span>
                </div>
              </div>
            ))}
          </div>
          <div style={{ marginTop: "10px", fontSize: "11px", fontWeight: "700", color: prizeShareTotalBps(formData.prize_shares) > 10000 ? "#ef4444" : "var(--text-dim)" }}>
            TOTAL: {(prizeShareTotalBps(formData.prize_shares) / 100).toFixed(2)}%
            {prizeShareTotalBps(formData.prize_shares) > 10000 ? " — exceeds 100%" : ""}
          </div>
        </div>
      )}
    </div>
  );

  const renderStep3 = () => (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <SectionTitle>Match Parameters</SectionTitle>
      
      <div>
        <label style={labelStyle}>TOURNAMENT FORMAT</label>
        <select
          value={formData.format}
          onChange={(e) => updateFormData("format", e.target.value)}
          style={inputStyle}
        >
          <option value="SingleElimination">SINGLE ELIMINATION</option>
          <option value="Swiss">SWISS SYSTEM</option>
        </select>
      </div>

      {formData.format === "Swiss" && (
        <div>
          <label style={labelStyle}>ROUNDS</label>
          <input
            type="number"
            min="2"
            max="20"
            value={formData.swiss_rounds}
            onChange={(e) => updateFormData("swiss_rounds", parseInt(e.target.value) || 5)}
            style={inputStyle}
          />
        </div>
      )}

      <div>
        <label style={labelStyle}>PLAYER CAPACITY</label>
        <select
          value={formData.max_players}
          onChange={(e) => updateMaxPlayers(parseInt(e.target.value))}
          style={inputStyle}
        >
          <option value={2}>2 PLAYER DOCK (HEAD-TO-HEAD)</option>
          <option value={4}>4 PLAYER DOCK</option>
          <option value={8}>8 PLAYER DOCK</option>
          <option value={16}>16 PLAYER DOCK</option>
          <option value={32}>32 PLAYER DOCK</option>
          <option value={64}>64 PLAYER DOCK</option>
          <option value={128}>128 PLAYER DOCK</option>
        </select>
      </div>
    </div>
  );

  const renderStep4 = () => (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <SectionTitle>Policy & Scheduling</SectionTitle>
      
      <div>
        <label style={labelStyle}>ELO FILTER</label>
        <div style={{ display: "flex", gap: "1rem" }}>
          <input
            type="number"
            value={formData.elo_min || ""}
            onChange={(e) => updateFormData("elo_min", e.target.value ? parseInt(e.target.value) : undefined)}
            style={inputStyle}
            placeholder="MIN ELO"
          />
          <input
            type="number"
            value={formData.elo_max || ""}
            onChange={(e) => updateFormData("elo_max", e.target.value ? parseInt(e.target.value) : undefined)}
            style={inputStyle}
            placeholder="MAX ELO"
          />
        </div>
      </div>

      <div style={{ 
        padding: "1rem", 
        backgroundColor: "rgba(255,255,255,0.02)", 
        borderRadius: "16px",
        border: "1px solid var(--border)"
      }}>
        <label style={{ ...labelStyle, display: "flex", alignItems: "center", cursor: "pointer", margin: 0 }}>
          <input
            type="checkbox"
            checked={formData.kyc_required}
            onChange={(e) => updateFormData("kyc_required", e.target.checked)}
            style={{ marginRight: "0.75rem", accentColor: "var(--primary)", width: "16px", height: "16px" }}
          />
          REQUIRE KYC / CACF CLEARANCE
        </label>
      </div>

      <div>
        <label style={labelStyle}>SCHEDULED ACTIVATION</label>
        <div style={{ display: "flex", gap: "8px" }}>
          <input
            type="datetime-local"
            value={formData.scheduled_at ? new Date(formData.scheduled_at * 1000).toISOString().slice(0, 16) : ""}
            onChange={(e) => {
              if (!e.target.value) { updateFormData("scheduled_at", undefined); return; }
              const date = new Date(e.target.value);
              updateFormData("scheduled_at", date.getTime() / 1000);
            }}
            style={{ ...inputStyle, flex: 1 }}
          />
          <button
            type="button"
            onClick={() => updateFormData("scheduled_at", undefined)}
            title="Clear the date — registration opens the moment INITIALIZE TOURNAMENT succeeds"
            style={{
              padding: "0 1.25rem",
              borderRadius: "12px",
              border: formData.scheduled_at ? "1px solid var(--border)" : "1px solid var(--primary)",
              background: formData.scheduled_at ? "transparent" : "rgba(173,92,47,0.15)",
              color: formData.scheduled_at ? "var(--text-dim)" : "var(--primary)",
              fontWeight: 700,
              fontSize: "12px",
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            INSTANT
          </button>
        </div>
        <div style={{ fontSize: "11px", color: "var(--text-dim)", marginTop: "6px", fontStyle: "italic" }}>
          {formData.scheduled_at
            ? "Registration opens at the scheduled time above."
            : "No date set — registration opens immediately once created."}
        </div>
      </div>
    </div>
  );

  return (
    <div style={{
      backgroundColor: "var(--surface)",
      borderRadius: "24px",
      padding: "2.5rem",
      border: "1px solid var(--border)",
      backdropFilter: "blur(20px)",
      width: "100%",
      maxWidth: "800px",
      margin: "0 auto",
      boxShadow: "0 20px 60px rgba(0,0,0,0.4)"
    }}>
      {/* Progress Pills */}
      <div style={{ display: "flex", gap: "8px", marginBottom: "3rem" }}>
        {[1, 2, 3, 4].map(step => (
          <div
            key={step}
            style={{
              flex: 1,
              height: "4px",
              borderRadius: "100px",
              backgroundColor: step <= currentStep ? "var(--primary)" : "rgba(255,255,255,0.05)",
              transition: "all 0.4s ease"
            }}
          />
        ))}
      </div>

      <form onSubmit={handleSubmit}>
        {currentStep === 1 && renderStep1()}
        {currentStep === 2 && renderStep2()}
        {currentStep === 3 && renderStep3()}
        {currentStep === 4 && renderStep4()}

        {error && (
          <div style={{
            marginTop: "1.5rem",
            padding: "1rem",
            backgroundColor: "rgba(239, 68, 68, 0.1)",
            border: "1px solid #ef4444",
            borderRadius: "12px",
            color: "#ef4444",
            fontSize: "13px"
          }}>
            SYSTEM ERROR: {error}
          </div>
        )}

        {/* Navigation */}
        <div style={{
          display: "flex",
          justifyContent: "space-between",
          marginTop: "3rem",
          paddingTop: "2rem",
          borderTop: "1px solid var(--border)"
        }}>
          <button
            type="button"
            onClick={prevStep}
            disabled={currentStep === 1 || loading}
            style={{
              padding: "0.85rem 2rem",
              borderRadius: "100px",
              backgroundColor: "transparent",
              color: currentStep === 1 ? "transparent" : "var(--text-dim)",
              border: "1px solid var(--border)",
              pointerEvents: currentStep === 1 ? "none" : "auto"
            }}
          >
            PREVIOUS
          </button>

          <div style={{ display: "flex", gap: "1rem" }}>
            <button
              type="button"
              onClick={onCancel}
              style={{
                padding: "0.85rem 2rem",
                borderRadius: "100px",
                backgroundColor: "transparent",
                color: "var(--text-dim)",
                border: "1px solid transparent"
              }}
            >
              CANCEL
            </button>

            {currentStep < 4 ? (
              <button
                type="button"
                onClick={nextStep}
                className="primary"
                disabled={!validateStep() || loading}
                style={{
                  padding: "0.85rem 2.5rem",
                  borderRadius: "100px",
                  opacity: validateStep() ? 1 : 0.4
                }}
              >
                CONTINUE
              </button>
            ) : (
              <button
                type="submit"
                className="primary"
                disabled={loading}
                style={{
                  padding: "0.85rem 2.5rem",
                  borderRadius: "100px",
                }}
              >
                {loading ? "INITIALIZING..." : "INITIALIZE TOURNAMENT"}
              </button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}

// Utility Components
const SectionTitle = ({ children }: { children: React.ReactNode }) => (
  <h2 style={{ color: "#fff", fontSize: "24px", fontWeight: "800", marginBottom: "0.5rem" }}>{children}</h2>
);

/** The on-chain program and backend store prize shares in basis points
 * (10000 = 100%); these three helpers keep that the wire format while the
 * UI reads/writes plain percentages. */
const bpsToPercentInput = (bps: number): string => {
  const pct = bps / 100;
  return pct === 0 ? "0" : String(pct);
};

const percentInputToBps = (raw: string): number => {
  const pct = parseFloat(raw);
  if (!Number.isFinite(pct) || pct < 0) return 0;
  return Math.round(pct * 100);
};

const prizeShareTotalBps = (shares?: number[]): number =>
  (shares || []).reduce((sum, v) => sum + (v || 0), 0);

/** Shows lamports + a best-effort USD equivalent under a SOL input; omits the
 * USD half until a rate has loaded rather than showing a stale/wrong figure. */
const FeeEquivalent = ({ lamports, solUsdRate, color }: { lamports: number; solUsdRate: number | null; color: string }) => {
  const usd = lamportsToUsd(lamports, solUsdRate);
  return (
    <div style={{ color, fontSize: "11px", marginTop: "8px", fontWeight: "700" }}>
      {lamports.toLocaleString()} LAMPORTS{usd != null ? ` · ≈ $${usd.toFixed(2)} USD` : ""}
    </div>
  );
};

const labelStyle: React.CSSProperties = {
  display: "block",
  color: "var(--text-dim)",
  fontSize: "11px",
  fontWeight: "800",
  letterSpacing: "1.5px",
  marginBottom: "10px"
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "1rem 1.25rem",
  backgroundColor: "rgba(0, 0, 0, 0.2)",
  border: "1px solid var(--border)",
  borderRadius: "16px",
  color: "#ffffff",
  fontSize: "14px",
  outline: "none",
  transition: "border-color 0.2s ease"
};

/** Applied on top of inputStyle while a USD field has no rate to convert
 * against yet — visibly inert rather than a plain disabled input that reads
 * as broken. */
const disabledInputStyle: React.CSSProperties = {
  opacity: 0.5,
  cursor: "not-allowed"
};
