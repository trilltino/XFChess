import { useState } from "react";
import { apiClient } from "../services/api";

export default function KycStatus() {
  const [wallet, setWallet] = useState("");
  const [status, setStatus] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [verifying, setVerifying] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");

  const checkStatus = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!wallet.trim()) return;

    setLoading(true);
    setError("");
    setSuccess("");
    setStatus(null);

    try {
      const response = await apiClient.getKycStatus(wallet);
      if (response.ok) {
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

  const verifyPlayer = async () => {
    if (!wallet) return;
    setVerifying(true);
    setError("");
    setSuccess("");

    try {
      // Assuming we'll add verifyProfile to apiClient
      const response = await (apiClient as any).verifyProfile(wallet);
      if (response.ok) {
        setSuccess("Player successfully verified on-chain");
        checkStatus();
      } else {
        setError(response.error?.message || "Verification failed");
      }
    } catch (err) {
      setError("Network error during verification");
    } finally {
      setVerifying(false);
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

        {success && (
          <div style={{ padding: "1rem", backgroundColor: "rgba(34, 197, 94, 0.1)", border: "1px solid #22c55e", borderRadius: "12px", color: "#22c55e", marginBottom: "1.5rem", fontSize: "13px" }}>
            {success}
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
              <div style={{ color: "white", fontSize: "1.1rem", fontWeight: "bold", marginBottom: "0.25rem" }}>{status.username || "Anonymous"}</div>
              <div style={{ color: "var(--text-dim)", fontSize: "0.8rem", fontFamily: "monospace" }}>{wallet}</div>
            </div>

            {!status.verified && (
              <button 
                onClick={verifyPlayer}
                disabled={verifying}
                className="primary" 
                style={{ padding: "1rem", borderRadius: "100px", fontWeight: "bold", boxShadow: "0 4px 15px rgba(173, 92, 47, 0.3)" }}
              >
                {verifying ? "APPROVING ON-CHAIN..." : "✅ APPROVE KYC STATUS"}
              </button>
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
