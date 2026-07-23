import { useState } from 'react';
import { CheckCircle2, Loader2, Mail } from 'lucide-react';
import { submitWaitlist } from '../lib/api';
import { SeoHead } from '../components/SeoHead';
import { PAGE_METADATA } from '../lib/seo/metadata';

const isValidEmail = (e: string) => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(e.trim());

export function Waitlist() {
  const [email, setEmail] = useState('');
  const [status, setStatus] = useState<'idle' | 'loading' | 'done' | 'error'>('idle');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!isValidEmail(email)) {
      setError('Please enter a valid email address.');
      return;
    }
    setStatus('loading');
    setError(null);
    try {
      await submitWaitlist(email.trim());
      setStatus('done');
    } catch (err) {
      setStatus('error');
      setError(err instanceof Error ? err.message : 'Something went wrong. Please try again.');
    }
  };

  return (
    <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
      <SeoHead meta={PAGE_METADATA.waitlist} />
      <div style={{ maxWidth: '520px', margin: '0 auto', padding: '0 20px' }}>
        <div className="section-label">EARLY ACCESS</div>
        <h2 style={{ fontSize: '2.5rem', marginBottom: '8px' }}>Join the Waitlist<span className="accent">.</span></h2>
        <p style={{ color: 'var(--text-dim)', lineHeight: 1.7, marginBottom: '32px' }}>
          Be first in line for XFChess. Drop your email and we'll let you know the moment your spot opens up.
        </p>

        <div style={{ background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: '16px', padding: '32px' }}>
          {status === 'done' ? (
            <div style={{ textAlign: 'center', padding: '12px 0' }}>
              <CheckCircle2 size={40} color="var(--primary)" style={{ marginBottom: '12px' }} />
              <h3 style={{ fontSize: '1.3rem', fontWeight: 800, color: '#fff', marginBottom: '8px' }}>You're on the list</h3>
              <p style={{ color: 'var(--text-dim)', lineHeight: 1.6 }}>
                Check your inbox for a confirmation. We'll be in touch soon.
              </p>
            </div>
          ) : (
            <form onSubmit={handleSubmit}>
              <div style={{ position: 'relative', marginBottom: '12px' }}>
                <Mail size={18} style={{ position: 'absolute', left: '14px', top: '50%', transform: 'translateY(-50%)', color: 'var(--text-dim)' }} />
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="you@email.com"
                  required
                  disabled={status === 'loading'}
                  style={{
                    width: '100%',
                    padding: '14px 14px 14px 44px',
                    borderRadius: '8px',
                    border: `1px solid ${error ? '#ff4444' : 'var(--border)'}`,
                    background: 'var(--glass)',
                    color: '#fff',
                    fontSize: '1rem',
                  }}
                />
              </div>
              {error && (
                <p style={{ color: '#ff4444', fontSize: '0.85rem', margin: '0 0 12px' }}>{error}</p>
              )}
              <button
                type="submit"
                className="btn btn-primary"
                disabled={status === 'loading'}
                style={{ width: '100%', textAlign: 'center', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}
              >
                {status === 'loading' ? <Loader2 size={18} className="spinner" /> : 'Join Waitlist'}
              </button>
            </form>
          )}
        </div>
      </div>
    </main>
  );
}

export default Waitlist;
