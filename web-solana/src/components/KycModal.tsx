import { useState } from 'react';
import { X, Loader2 } from 'lucide-react';
import { submitKyc, type KycSubmission } from '../lib/api';

interface Props {
  walletPubkey: string;
  onClose: () => void;
  onSuccess: () => void;
}

const COUNTRIES = [
  { code: 'GB', label: 'United Kingdom', taxLabel: 'National Insurance Number' },
  { code: 'BR', label: 'Brazil', taxLabel: 'CPF' },
  { code: 'DE', label: 'Germany', taxLabel: 'Steueridentifikationsnummer' },
  { code: 'CA', label: 'Canada', taxLabel: 'Social Insurance Number' },
];

export function KycModal({ walletPubkey, onClose, onSuccess }: Props) {
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

  const currentCountry = COUNTRIES.find((c) => c.code === form.country);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await submitKyc(form);
      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Submission failed.');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="kyc-modal-backdrop" onClick={onClose}>
      <div className="kyc-modal" onClick={(e) => e.stopPropagation()}>
        <button type="button" className="kyc-close" onClick={onClose} aria-label="Close">
          <X size={18} />
        </button>

        <h3 className="kyc-title">KYC Verification</h3>
        <p className="kyc-sub">
          Required for PvP wagering and Cash Tournaments. See the{' '}
          <a href="/kyc" target="_blank" rel="noreferrer" style={{ color: 'var(--primary)' }}>
            KYC policy
          </a>{' '}
          for storage details.
        </p>

        <form onSubmit={handleSubmit} className="kyc-form">
          <label className="kyc-field">
            <span>Country of residence</span>
            <select
              value={form.country}
              onChange={(e) => setForm({ ...form, country: e.target.value })}
            >
              {COUNTRIES.map((c) => (
                <option key={c.code} value={c.code}>
                  {c.label}
                </option>
              ))}
            </select>
          </label>

          <label className="kyc-field">
            <span>Legal full name</span>
            <input
              type="text"
              required
              value={form.full_name}
              onChange={(e) => setForm({ ...form, full_name: e.target.value })}
            />
          </label>

          <label className="kyc-field">
            <span>Date of birth</span>
            <input
              type="date"
              required
              value={form.dob}
              onChange={(e) => setForm({ ...form, dob: e.target.value })}
            />
          </label>

          <label className="kyc-field">
            <span>Residential address</span>
            <input
              type="text"
              required
              value={form.residence}
              onChange={(e) => setForm({ ...form, residence: e.target.value })}
            />
          </label>

          <label className="kyc-field">
            <span>{currentCountry?.taxLabel ?? 'Tax ID'}</span>
            <input
              type="text"
              required={form.country !== 'US'}
              value={form.tax_id}
              onChange={(e) => setForm({ ...form, tax_id: e.target.value })}
            />
          </label>

          {error && <div className="kyc-error">{error}</div>}

          <button type="submit" className="btn btn-primary" disabled={submitting}>
            {submitting ? <Loader2 className="spinner" /> : 'Submit'}
          </button>
        </form>
      </div>
    </div>
  );
}
