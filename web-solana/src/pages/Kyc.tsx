import { useMemo, useState } from 'react';
import { motion } from 'framer-motion';
import { ArrowLeft, Loader2 } from 'lucide-react';
import { Link } from 'react-router-dom';
import { submitKyc, type KycSubmission } from '../lib/api';

const COUNTRIES = [
  { code: 'GB', label: 'United Kingdom', taxLabel: 'National Insurance Number', pattern: /^[A-Za-z]{2}\d{6}[A-Za-z]$/, example: 'AB123456C' },
  { code: 'BR', label: 'Brazil', taxLabel: 'CPF', pattern: /^\d{3}\.?\d{3}\.?\d{3}-?\d{2}$/, example: '123.456.789-01' },
  { code: 'DE', label: 'Germany', taxLabel: 'Steueridentifikationsnummer', pattern: /^\d{11}$/, example: '12345678901' },
  { code: 'CA', label: 'Canada', taxLabel: 'Social Insurance Number', pattern: /^\d{3}-?\d{3}-?\d{3}$/, example: '123-456-789' },
];

const KycPage = () => {
  const walletPubkey = localStorage.getItem('xfchess_wallet') || localStorage.getItem('xfchess_wallet_pubkey') || '';
  const [form, setForm] = useState<KycSubmission>({
    wallet_pubkey: walletPubkey,
    country: 'GB',
    full_name: '',
    dob: '',
    residence: '',
    tax_id: '',
  });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [taxIdError, setTaxIdError] = useState<string | null>(null);

  const currentCountry = useMemo(
    () => COUNTRIES.find((country) => country.code === form.country),
    [form.country],
  );

  const updateField = <K extends keyof KycSubmission>(key: K, value: KycSubmission[K]) => {
    setForm((current) => ({ ...current, [key]: value }));

    // Validate tax_id when it changes
    if (key === 'tax_id' && currentCountry?.pattern) {
      if (!currentCountry.pattern.test(value.toString())) {
        setTaxIdError(`Invalid format. Example: ${currentCountry.example}`);
      } else {
        setTaxIdError(null);
      }
    }
  };

  // Reset tax_id error when country changes
  const handleCountryChange = (value: string) => {
    updateField('country', value);
    setTaxIdError(null);
    setForm((current) => ({ ...current, tax_id: '' }));
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    setSuccess(null);

    if (!form.wallet_pubkey.trim()) {
      setError('Connect a wallet before submitting KYC.');
      return;
    }

    if (taxIdError) {
      setError('Please fix the tax ID format before submitting.');
      return;
    }

    setSubmitting(true);
    try {
      await submitKyc(form);
      setSuccess('KYC details submitted successfully. You can now return to wagering setup.');
    } catch (submissionError) {
      setError(submissionError instanceof Error ? submissionError.message : 'KYC submission failed.');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div style={{ maxWidth: '760px', margin: '0 auto 32px', background: 'rgba(255, 255, 255, 0.03)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '16px', padding: '32px' }}>
          <div className="section-label">Verification</div>
          <h1 style={{ fontSize: '2rem', fontWeight: 900, marginBottom: '10px' }}>Complete KYC</h1>
          <p style={{ color: 'var(--text-dim)', lineHeight: 1.7, marginBottom: '24px' }}>
            Enter the details required for wagering eligibility. This submits directly to your local/backend `/api/kyc/submit` endpoint.
          </p>

          <form onSubmit={handleSubmit} style={{ display: 'grid', gap: '16px' }}>
            <label style={{ display: 'grid', gap: '8px' }}>
              <span>Wallet public key</span>
              <input
                type="text"
                value={form.wallet_pubkey}
                onChange={(event) => updateField('wallet_pubkey', event.target.value)}
                placeholder="Connect wallet first or paste pubkey"
                style={{ padding: '12px 14px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff' }}
              />
            </label>

            <label style={{ display: 'grid', gap: '8px' }}>
              <span>Country of residence</span>
              <select
                value={form.country}
                onChange={(event) => handleCountryChange(event.target.value)}
                style={{ padding: '12px 14px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff' }}
              >
                {COUNTRIES.map((country) => (
                  <option key={country.code} value={country.code}>
                    {country.label}
                  </option>
                ))}
              </select>
            </label>

            <label style={{ display: 'grid', gap: '8px' }}>
              <span>Legal full name</span>
              <input
                type="text"
                required
                value={form.full_name}
                onChange={(event) => updateField('full_name', event.target.value)}
                style={{ padding: '12px 14px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff' }}
              />
            </label>

            <label style={{ display: 'grid', gap: '8px' }}>
              <span>Date of birth</span>
              <input
                type="date"
                required
                value={form.dob}
                onChange={(event) => updateField('dob', event.target.value)}
                style={{ padding: '12px 14px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff' }}
              />
            </label>

            <label style={{ display: 'grid', gap: '8px' }}>
              <span>Residential address</span>
              <input
                type="text"
                required
                value={form.residence}
                onChange={(event) => updateField('residence', event.target.value)}
                style={{ padding: '12px 14px', borderRadius: '10px', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff' }}
              />
            </label>

            <label style={{ display: 'grid', gap: '8px' }}>
              <span>{currentCountry?.taxLabel ?? 'Tax ID'}</span>
              <input
                type="text"
                required
                value={form.tax_id}
                onChange={(event) => updateField('tax_id', event.target.value)}
                placeholder={`Example: ${currentCountry?.example ?? ''}`}
                style={{ padding: '12px 14px', borderRadius: '10px', border: taxIdError ? '1px solid rgba(255, 80, 80, 0.5)' : '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.04)', color: '#fff' }}
              />
              {taxIdError && <span style={{ color: '#ffd0d0', fontSize: '0.85rem' }}>{taxIdError}</span>}
              {currentCountry?.example && !taxIdError && <span style={{ color: 'var(--text-dim)', fontSize: '0.8rem' }}>Format: {currentCountry.example}</span>}
            </label>

            {error && <div style={{ color: '#ffd0d0', background: 'rgba(255, 80, 80, 0.12)', border: '1px solid rgba(255, 80, 80, 0.3)', borderRadius: '10px', padding: '12px 16px' }}>{error}</div>}
            {success && <div style={{ color: '#d6ffe0', background: 'rgba(80, 200, 120, 0.12)', border: '1px solid rgba(80, 200, 120, 0.3)', borderRadius: '10px', padding: '12px 16px' }}>{success}</div>}

            <button
              type="submit"
              disabled={submitting}
              style={{ padding: '14px 20px', borderRadius: '10px', border: 'none', background: 'rgba(255,255,255,0.12)', color: '#fff', fontWeight: 800, cursor: 'pointer', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: '10px' }}
            >
              {submitting ? <Loader2 size={18} className="spinner" /> : null}
              {submitting ? 'Submitting...' : 'Submit KYC'}
            </button>
          </form>
        </div>

        <div className="legal-compliance-container">
          <div className="legal-header">
            <div className="legal-title">
              <h3>XFCHESS — IDENTITY VERIFICATION & KYC SUMMARY</h3>
              <div className="legal-meta">
                <span className="review-date">Last reviewed: April 2026</span>
                <span className="operator-info">XForceSolutions Ltd, registered in England and Wales</span>
              </div>
            </div>
          </div>

          <div className="legal-sections">
            <div className="legal-section">
              <h4>WHY THIS IS REQUIRED</h4>
              <p>XFChess requires identity verification for all players before any deposit or wagered match. This reflects both our legal assessment in progress (MLR 2017 / FCA cryptoasset regime SI 2026/102) and our own platform integrity standards. Regardless of final regulatory classification, we will not permit anonymous real-money play.</p>
            </div>

            <div className="legal-section">
              <h4>2. VERIFICATION REQUIREMENTS</h4>
              <p>Triggered on first deposit or first wagered match entry.</p>
              <p>Players must provide government-issued photo identification (passport, national ID, or driving licence) and complete identity verification before accessing any wagered features. Verification confirms the player is aged 18 or over and is not on any applicable sanctions list.</p>
            </div>

            <div className="legal-section">
              <h4>3. WHAT XFCHESS RECEIVES AND STORES</h4>
              <p><strong>Received:</strong> Verified status, 18+ confirmation, sanctions clear/match, full name, date of birth, country of residence</p>
              <p><strong>Not stored:</strong> Document images, raw biometric data, NFC chip data</p>
              <p>Document and biometric data stays with the verification provider. XFChess retains only the minimum required by UK AML law — 5 years from end of business relationship.</p>
            </div>

            <div className="legal-section">
              <h4>4. SANCTIONS & PEP SCREENING</h4>
              <p>All players screened at verification against:</p>
              <p>HM Treasury UK list, UN list, EU list, OFAC, and PEP databases.</p>
              <div className="legal-highlight">
                <p>A sanctions match = declined. Not subject to appeal through XFChess.</p>
              </div>
            </div>

            <div className="legal-section">
              <h4>5. TAX ID VERIFICATION</h4>
              <p>For compliance with CACF (Crypto-Asset Reporting Framework) requirements, players from certain jurisdictions must provide country-specific tax identification numbers:</p>
              <ul className="tax-list">
                <li><strong>United Kingdom:</strong> National Insurance Number (NI) — 2 letters + 6 digits + 1 letter (e.g., AB123456C)</li>
                <li><strong>Brazil:</strong> CPF (Cadastro de Pessoas Físicas) — 11 digits</li>
                <li><strong>Germany:</strong> Tax ID (Steueridentifikationsnummer) — 11 digits</li>
                <li><strong>Canada:</strong> Social Insurance Number (SIN) — 9 digits</li>
              </ul>
              <p>Tax IDs are stored using blind index hashing for privacy, allowing compliance verification without exposing raw identification data.</p>
            </div>

            <div className="legal-section">
              <h4>6. ENHANCED DUE DILIGENCE (EDD)</h4>
              <p>Players exceeding defined wager volume or deposit thresholds may be asked for proof of address and source of funds. Failure to provide within the specified timeframe results in account restrictions.</p>
            </div>

            <div className="legal-section">
              <h4>7. TRAVEL RULE</h4>
              <p>For cryptoasset transfers at or above £1,000, originator and beneficiary information is collected and transmitted where applicable under UK Travel Rule requirements (MLR 2017 as amended).</p>
            </div>

            <div className="legal-section">
              <h4>8. DATA PROTECTION</h4>
              <p>Processed under UK GDPR and Data Protection Act 2018.</p>
              <p>Legal basis: Article 6(1)(c) — compliance with legal obligation.</p>
              <p>Data is not sold. Shared only with verification provider and authorities where legally required.</p>
              <p>Data rights requests: <a href="mailto:privacy@xfchess.com">privacy@xfchess.com</a></p>
            </div>

            <div className="legal-section">
              <h4>9. DECLINED VERIFICATIONS</h4>
              <p>Common reasons: expired document, liveness fail, name mismatch, unrecognised document, sanctions match.</p>
              <p>Contact support at <a href="mailto:kyc@xfchess.com">kyc@xfchess.com</a> for case-by-case review. Sanctions matches cannot be overridden.</p>
            </div>

            <div className="legal-section">
              <h4>10. DISCLAIMER</h4>
              <p>Pre-launch platform. Implementation details subject to change.</p>
              <p>Nothing on this page is legal advice.</p>
              <div className="legal-contact">
                <p><strong>Legal queries:</strong> <a href="mailto:legal@xfchess.com">legal@xfchess.com</a></p>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default KycPage;

