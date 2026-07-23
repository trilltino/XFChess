/**
 * Email + password login modal.
 *
 * Submits to the backend via `loginWithEmail`, stores the returned token
 * and display name in `localStorage` under the `xfchess_*` keys, and
 * invokes the `onLoginSuccess` callback so the navbar can update its
 * logged-in state without a page reload.
 */

import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { loginWithEmail } from '../lib/api';

interface Props {
  onClose: () => void;
  onLoginSuccess: (email: string, username: string) => void;
}

export function LoginModal({ onClose, onLoginSuccess }: Props) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!email || !password) {
      setError('Email and password are required');
      return;
    }
    setLoading(true);
    try {
      const res = await loginWithEmail({ email, password });
      localStorage.setItem('xfchess_token', res.token);
      localStorage.setItem('xfchess_username', res.username);
      localStorage.setItem('xfchess_email', email);
      onLoginSuccess(email, res.username);
      onClose();
    } catch (err: any) {
      setError(err.message || 'Login failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="custom-wallet-modal"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: '400px' }}
      >
        <div className="modal-header">
          <h3>Login</h3>
          <button className="modal-close" onClick={onClose}>
            &times;
          </button>
        </div>
        <form
          onSubmit={handleSubmit}
          style={{
            padding: '24px',
            display: 'flex',
            flexDirection: 'column',
            gap: '16px',
          }}
        >
          {error && (
            <div
              style={{
                color: '#ffd0d0',
                background: 'rgba(255, 80, 80, 0.12)',
                border: '1px solid rgba(255, 80, 80, 0.3)',
                borderRadius: '8px',
                padding: '12px',
                fontSize: '14px',
              }}
            >
              {error}
            </div>
          )}
          <div>
            <label
              style={{
                display: 'block',
                fontSize: '12px',
                fontWeight: 700,
                color: 'rgba(255,255,255,0.6)',
                marginBottom: '6px',
                textTransform: 'uppercase',
                letterSpacing: '0.08em',
              }}
            >
              Email
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              style={{
                width: '100%',
                padding: '12px 14px',
                borderRadius: '8px',
                border: '1px solid rgba(255,255,255,0.1)',
                background: 'rgba(255,255,255,0.04)',
                color: '#fff',
                fontSize: '14px',
                outline: 'none',
              }}
            />
          </div>
          <div>
            <label
              style={{
                display: 'block',
                fontSize: '12px',
                fontWeight: 700,
                color: 'rgba(255,255,255,0.6)',
                marginBottom: '6px',
                textTransform: 'uppercase',
                letterSpacing: '0.08em',
              }}
            >
              Password
            </label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••"
              style={{
                width: '100%',
                padding: '12px 14px',
                borderRadius: '8px',
                border: '1px solid rgba(255,255,255,0.1)',
                background: 'rgba(255,255,255,0.04)',
                color: '#fff',
                fontSize: '14px',
                outline: 'none',
              }}
            />
          </div>
          <button
            type="submit"
            disabled={loading}
            style={{
              width: '100%',
              padding: '14px',
              borderRadius: '8px',
              border: 'none',
              background: '#ffffff',
              color: '#000000',
              fontWeight: 700,
              fontSize: '14px',
              cursor: loading ? 'not-allowed' : 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '8px',
            }}
          >
            {loading ? <Loader2 size={16} className="spinner" /> : null}
            {loading ? 'Signing in...' : 'Sign In'}
          </button>
        </form>
      </div>
    </div>
  );
}
