import { useState } from "react";
import { apiClient } from "../services/api";

export default function KycStatus() {
  const [wallet, setWallet] = useState("");
  const [status, setStatus] = useState<{ verified: boolean; verified_at: number | null; country: string | null; requires_kyc: boolean } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  // Read-only: KYC verification is fully automatic when a player completes
  // POST /identity/register (in-game or on the website) — that flow submits
  // the on-chain verify_profile_ix itself. There is no separate backend
  // capability for an admin to manually approve KYC, so this page only
  // reports status; it used to have a fake "APPROVE" button that called a
  // method (`apiClient.verifyProfile`) which never existed.
  const checkStatus = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!wallet.trim()) return;

    setLoading(true);
    setError("");
    setStatus(null);

    try {
      const response = await apiClient.getKycStatus(wallet);
      if (response.ok && response.data) {
        setStatus(response.data);
      } else {
        setError(response.error?.message || "Player not found or error fetching status");
      }
    } catch (err) {
      setError("Network error fetching status");
    } finally {
      setLoading(false);
    }
  };

  const inputStyle: React.CSSProperties = {
    backgroundColor: "rgba(255,255,255,0.03)",
    border: "1px solid rgba(255,255,255,0.1)",
    borderRadius: "12px",
    padding: "0.8rem 1.2rem",
    color: "white",
    fontSize: "14px",
    width: "100%",
    outline: "none",
    transition: "all 0.3s ease"
  };

  return (
    <div style={{ padding: "2rem", maxWidth: "800px", margin: "0 auto" }}>
      <div style={{ 
        backgroundColor: "var(--surface)",
        padding: "2.5rem",
        borderRadius: "24px",
        border: "1px solid var(--border)",
        backdropFilter: "blur(20px)",
        boxShadow: "0 20px 60px rgba(0,0,0,0.4)"
      }}>
        <h1 style={{ margin: "0 0 0.5rem 0", fontSize: "2rem", color: "white" }}>KYC <span style={{ color: "var(--primary)" }}>CLEARANCE</span></h1>
        <p style={{ color: "var(--text-dim)", marginBottom: "2rem" }}>Verify player identities and manage on-chain KYC status</p>

        <form onSubmit={checkStatus} style={{ display: "flex", gap: "1rem", marginBottom: "2rem" }}>
          <input
            type="text"
            placeholder="Enter player wallet address..."
            value={wallet}
            onChange={(e) => setWallet(e.target.value)}
            style={inputStyle}
          />
          <button type="submit" className="primary" style={{ padding: "0.8rem 2rem", borderRadius: "12px", border: "none", cursor: "pointer", fontWeight: "bold" }} disabled={loading}>
            {loading ? "SEARCHING..." : "SEARCH"}
          </button>
        </form>

        {error && (
          <div style={{ padding: "1rem", backgroundColor: "rgba(239, 68, 68, 0.1)", border: "1px solid #ef4444", borderRadius: "12px", color: "#ef4444", marginBottom: "1.5rem", fontSize: "13px" }}>
            {error}
          </div>
        )}

        {status && (
          <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem", animation: "fadeIn 0.4s ease" }}>
            <div style={{ padding: "1.5rem", backgroundColor: "rgba(255,255,255,0.02)", borderRadius: "16px", border: "1px solid var(--border)" }}>
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "1rem", alignItems: "center" }}>
                <span style={{ fontSize: "11px", color: "var(--text-dim)", fontWeight: "bold", letterSpacing: "1px" }}>PLAYER IDENTITY</span>
                <span style={{ 
                  fontSize: "10px", 
                  padding: "4px 10px", 
                  borderRadius: "100px", 
                  backgroundColor: status.verified ? "rgba(34, 197, 94, 0.1)" : "rgba(234, 179, 8, 0.1)",
                  color: status.verified ? "#22c55e" : "#eab308",
                  border: `1px solid ${status.verified ? "#22c55e" : "#eab308"}44`
                }}>
                  {status.verified ? "VERIFIED" : "UNVERIFIED"}
                </span>
              </div>
              <div style={{ color: "var(--text-dim)", fontSize: "0.8rem", fontFamily: "monospace" }}>{wallet}</div>
              {status.verified_at && (
                <div style={{ color: "var(--text-dim)", fontSize: "0.8rem", marginTop: "0.5rem" }}>
                  Verified at: {new Date(status.verified_at * 1000).toLocaleString()}
                </div>
              )}
            </div>

            {!status.verified && (
              <div style={{ textAlign: "center", color: "var(--text-dim)", fontSize: "0.9rem" }}>
                Not yet verified. KYC is completed by the player themselves via identity registration
                (in-game or on the website) — there is no manual admin-approval step to trigger it here.
              </div>
            )}

            {status.verified && (
              <div style={{ textAlign: "center", color: "var(--text-dim)", fontSize: "0.9rem" }}>
                This player has already cleared KYC and can participate in high-stakes tournaments.
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
