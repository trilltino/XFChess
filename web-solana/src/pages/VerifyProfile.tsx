import { useState } from 'react';
import { useWallet } from '@solana/wallet-adapter-react';
import { ShieldAlert, ShieldCheck, Loader2 } from 'lucide-react';
import bs58 from 'bs58';

export function VerifyProfile() {
    const { publicKey, signMessage } = useWallet();
    const [loading, setLoading] = useState(false);
    const [success, setSuccess] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const [form, setForm] = useState({
        full_name: '',
        dob: '',
        address: '',
        country: '',
        tax_id: ''
    });
    const [consentKyc, setConsentKyc] = useState(false);
    const [consentRetentionYears, setConsentRetentionYears] = useState(7);

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setForm(prev => ({ ...prev, [e.target.name]: e.target.value }));
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!publicKey || !signMessage) {
            setError("Wallet not connected or does not support signing messages.");
            return;
        }

        setLoading(true);
        setError(null);
        try {
            const timestamp = Math.floor(Date.now() / 1000);
            
            // 1. Construct exactly the message the backend expects
            const message = `register_identity:${publicKey.toBase58()}:${timestamp}`;
            const messageBytes = new TextEncoder().encode(message);
            
            // 2. Request user signature
            const signatureBytes = await signMessage(messageBytes);
            const signatureBase58 = bs58.encode(signatureBytes);

            if (!consentKyc) {
                setError("You must consent to data processing to continue.");
                return;
            }

            // 3. Assemble Payload
            const payload = {
                pubkey: publicKey.toBase58(),
                full_name: form.full_name,
                dob: form.dob,
                address: form.address,
                country: form.country,
                tax_id: form.tax_id,
                signature: signatureBase58,
                timestamp: timestamp,
                consent_kyc: consentKyc,
                consent_retention_years: consentRetentionYears,
            };

            // 4. Send to Identity Vault Backend
            const backendUrl = import.meta.env.VITE_BACKEND_URL || 'http://localhost:8090';
            const res = await fetch(`${backendUrl}/identity/register`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload)
            });

            if (!res.ok) {
                const text = await res.text();
                throw new Error(text || "Failed to verify identity");
            }

            setSuccess(true);
        } catch (err: unknown) {
            console.error(err);
            setError(err instanceof Error ? err.message : "An error occurred during verification.");
        } finally {
            setLoading(false);
        }
    };

    if (success) {
        return (
            <main className="section" style={{ minHeight: '100vh', paddingTop: '140px', display: 'flex', justifyContent: 'center' }}>
                <div className="profile-card" style={{ textAlign: 'center' }}>
                    <ShieldCheck size={64} style={{ color: '#ffffff', margin: '0 auto 20px auto' }} />
                    <h2 style={{ fontSize: '2rem', marginBottom: '16px' }}>Identity Vaulted Successfully</h2>
                    <p style={{ color: 'var(--text-dim)', marginBottom: '32px' }}>
                        Your details have been securely encrypted and stored entirely off-chain.
                        A verification transaction has been dispatched to the blockchain. You may now play wagered competitive tournaments!
                    </p>
                    <button onClick={() => window.location.href = '/profile'} className="btn btn-secondary" style={{ width: 'auto', margin: '0 auto', padding: '0 32px' }}>
                        Return to Profile
                    </button>
                </div>
            </main>
        );
    }

    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '140px', display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
            <div className="section-label">CARF 2026 COMPLIANCE</div>
            <h2 style={{ fontSize: '2.5rem', textAlign: 'center' }}>Secure Identity Vault<span className="accent">.</span></h2>
            <p style={{ textAlign: 'center', color: 'var(--text-dim)', maxWidth: '600px', marginBottom: '40px' }}>
                To participate in wagered tournaments, verified tax reporting information is required. 
                Your information is encrypted securely in our off-chain vault mapping your public key index. 
                It will <strong>never</strong> be published on the blockchain.
            </p>

            <form onSubmit={handleSubmit} className="profile-card" style={{ display: 'flex', flexDirection: 'column', gap: '20px', width: '100%' }}>
                
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <label style={{ fontSize: '0.85rem', color: 'var(--text-dim)', fontWeight: 600 }}>Full Legal Name</label>
                    <input type="text" name="full_name" value={form.full_name} onChange={handleChange} required
                           style={{ padding: '14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--glass)', color: '#fff' }} />
                </div>
                
                <div style={{ display: 'flex', gap: '20px' }}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', flex: 1 }}>
                        <label style={{ fontSize: '0.85rem', color: 'var(--text-dim)', fontWeight: 600 }}>Date of Birth</label>
                        <input type="date" name="dob" value={form.dob} onChange={handleChange} required
                               style={{ padding: '14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--glass)', color: '#fff', colorScheme: 'dark' }} />
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', flex: 1 }}>
                        <label style={{ fontSize: '0.85rem', color: 'var(--text-dim)', fontWeight: 600 }}>Country of Residence</label>
                        <input type="text" name="country" value={form.country} onChange={handleChange} required placeholder="e.g. United Kingdom"
                               style={{ padding: '14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--glass)', color: '#fff' }} />
                    </div>
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <label style={{ fontSize: '0.85rem', color: 'var(--text-dim)', fontWeight: 600 }}>Full Physical Address</label>
                    <input type="text" name="address" value={form.address} onChange={handleChange} required
                           style={{ padding: '14px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--glass)', color: '#fff' }} />
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '10px' }}>
                    <label style={{ fontSize: '0.85rem', color: 'var(--text-dim)', fontWeight: 600 }}>Tax Identification Number</label>
                    <input type="text" name="tax_id" value={form.tax_id} onChange={handleChange} required placeholder="SSN, NIN, TIN, etc."
                           style={{ padding: '14px', borderRadius: '8px', border: '1px solid var(--primary)', background: 'var(--glass)', color: '#fff' }} />
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', padding: '16px', background: 'rgba(255,255,255,0.04)', borderRadius: '8px', border: '1px solid var(--border)' }}>
                    <label style={{ display: 'flex', alignItems: 'flex-start', gap: '12px', cursor: 'pointer', fontSize: '0.85rem', color: 'var(--text-dim)' }}>
                        <input type="checkbox" checked={consentKyc} onChange={e => setConsentKyc(e.target.checked)} required
                               style={{ marginTop: '3px', accentColor: 'var(--accent)', width: '16px', height: '16px', flexShrink: 0 }} />
                        <span>
                            I consent to the collection, encryption, and secure storage of my personal data for CARF 2026 / FATCA compliance.
                            My data will <strong>never</strong> be published on-chain. I understand I may request deletion at any time.
                        </span>
                    </label>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '12px', fontSize: '0.85rem', color: 'var(--text-dim)' }}>
                        <span style={{ whiteSpace: 'nowrap' }}>Data retention period:</span>
                        <select value={consentRetentionYears} onChange={e => setConsentRetentionYears(Number(e.target.value))}
                                style={{ padding: '8px 12px', borderRadius: '6px', border: '1px solid var(--border)', background: '#fff', color: '#111', flex: 1 }}>
                            <option value={5} style={{ color: '#111', background: '#fff' }}>5 years (minimum)</option>
                            <option value={7} style={{ color: '#111', background: '#fff' }}>7 years (recommended)</option>
                            <option value={10} style={{ color: '#111', background: '#fff' }}>10 years</option>
                        </select>
                    </label>
                </div>

                {error && (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', color: 'var(--primary)', padding: '12px', background: 'rgba(230, 57, 70, 0.1)', borderRadius: '8px', fontSize: '0.9rem' }}>
                        <ShieldAlert size={20} />
                        {error}
                    </div>
                )}

                <button type="submit" className="btn btn-primary" disabled={loading || !publicKey || !consentKyc}>
                    {loading ? <Loader2 className="spinner" /> : "Sign & Securely Submit"}
                </button>
            </form>
        </main>
    );
}

