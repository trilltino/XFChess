import { useState, type FormEvent } from 'react';
import { signUp, SignupError } from '../lib/api/signup';

type Status =
  | { kind: 'idle' }
  | { kind: 'sending' }
  | { kind: 'done'; email: string }
  | { kind: 'error'; message: string };

/** Email sign-up. The backend queues a welcome email through Resend. */
export function SignupForm() {
  const [email, setEmail] = useState('');
  const [status, setStatus] = useState<Status>({ kind: 'idle' });

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (status.kind === 'sending') return;
    setStatus({ kind: 'sending' });
    try {
      await signUp({ email });
      setStatus({ kind: 'done', email: email.trim() });
      setEmail('');
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof SignupError ? err.message : 'Something went wrong. Please try again.',
      });
    }
  }

  if (status.kind === 'done') {
    return (
      <p className="signup-done" role="status">
        You are signed up. A welcome email is on its way to {status.email}.
      </p>
    );
  }

  return (
    <form className="signup-form" onSubmit={onSubmit} noValidate>
      <label className="signup-field">
        <span>Email</span>
        <input
          type="email"
          name="email"
          autoComplete="email"
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
      </label>
      <button type="submit" className="signup-submit" disabled={status.kind === 'sending' || !email.trim()}>
        {status.kind === 'sending' ? 'Signing up...' : 'Sign up'}
      </button>
      {status.kind === 'error' && (
        <p className="signup-error" role="alert">
          {status.message}
        </p>
      )}
    </form>
  );
}
