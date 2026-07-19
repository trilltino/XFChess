import { useState, useEffect, useCallback } from 'react';
import { useWallet, useConnection } from '@solana/wallet-adapter-react';
import { PublicKey } from '@solana/web3.js';
import { Link } from 'react-router-dom';
import {
  Shield,
  Loader2,
  LogIn,
  UserPlus,
  CheckCircle2,
  XCircle,
  AlertCircle,
} from 'lucide-react';
import {
  getAnchorProgram,
  fetchPlayerProfile,
  createPlayerProfile,
  PROGRAM_ID,
} from '../lib/anchor_client';
import bs58 from 'bs58';
import { submitSignup, getUserStatus, registerWithWallet, checkUsernameAvailable, addEmail, syncProfile, type UserStatus } from '../lib/api';
import { KycModal } from '../components/KycModal';
import { MatchHistory } from '../components/MatchHistory';
import { LichessLinkCard } from '../components/LichessLinkCard';

const COUNTRIES = [
  { code: 'GB', label: 'United Kingdom' },
  { code: 'CA', label: 'Canada' },
  { code: 'DE', label: 'Germany' },
  { code: 'BR', label: 'Brazil' },
];

interface SignupFormProps {
  newUsername: string;
  setNewUsername: (v: string) => void;
  email: string;
  setEmail: (v: string) => void;
  country: string;
  setCountry: (v: string) => void;
  usernameError: string | null;
  setUsernameError: (v: string | null) => void;
  emailError: string | null;
  setEmailError: (v: string | null) => void;
  creationLoading: boolean;
  handleCreateProfile: (e: React.FormEvent) => void;
}

// Username validation: 3-20 chars, alphanumeric + underscore/dot/hyphen, no pure symbols, must start with letter
const validateUsername = (username: string): string | null => {
  if (!username) return 'Username is required';
  if (username.length < 3) return 'Username must be at least 3 characters';
  if (username.length > 20) return 'Username must be at most 20 characters';
  if (!/^[a-zA-Z]/.test(username)) return 'Username must start with a letter';
  if (!/^[a-zA-Z0-9._-]+$/.test(username)) return 'Username can only contain letters, numbers, underscores, dots, and hyphens';
  if (/^[._-]+$/.test(username)) return 'Username cannot contain only special characters';
  return null;
};

const validateEmail = (email: string): string | null => {
  if (!email) return 'Email is required';
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  if (!emailRegex.test(email)) return 'Please enter a valid email address';
  return null;
};

function SignupForm({
  newUsername,
  setNewUsername,
  email,
  setEmail,
  country,
  setCountry,
  usernameError,
  setUsernameError,
  emailError,
  setEmailError,
  creationLoading,
  handleCreateProfile,
}: SignupFormProps) {
  const [checkingUsername, setCheckingUsername] = useState(false);

  const handleUsernameChange = (value: string) => {
    setNewUsername(value);
    setUsernameError(validateUsername(value));
  };

  const handleEmailChange = (value: string) => {
    setEmail(value);
    setEmailError(validateEmail(value));
  };

  // Debounced backend uniqueness check (500ms after user stops typing)
  useEffect(() => {
    const formatErr = validateUsername(newUsername);
    if (formatErr || !newUsername) return;
    setCheckingUsername(true);
    const timer = setTimeout(() => {
      checkUsernameAvailable(newUsername)
        .then(({ taken }) => {
          setUsernameError(taken ? 'Username already taken' : null);
        })
        .catch(() => { /* network error � don't block UX */ })
        .finally(() => setCheckingUsername(false));
    }, 500);
    return () => clearTimeout(timer);
  }, [newUsername, setUsernameError]);

  const isFormValid = !usernameError && !emailError && newUsername && email && country && !checkingUsername;

  return (
    <form
      onSubmit={handleCreateProfile}
      className="profile-card"
      style={{ marginBottom: '32px', margin: '0 auto', maxWidth: '600px' }}
    >
      <h3 style={{ fontSize: '1.4rem', fontWeight: 800, marginBottom: '12px', textAlign: 'center' }}>
        Create your Profile
      </h3>
      <p style={{ color: 'var(--text-dim)', marginBottom: '20px', fontSize: '0.95rem', textAlign: 'center' }}>
        Register your Solana wallet and create an on-chain profile. Your username will be tied to your profile for wagered games.
      </p>

      {/* Username field with validation */}
      <div style={{ marginBottom: '12px' }}>
        <input
          type="text"
          value={newUsername}
          onChange={(e) => handleUsernameChange(e.target.value)}
          placeholder="Username (3-20 chars, letters/numbers/_/./-)"
          required
          maxLength={20}
          style={{
            ...inputStyle,
            borderColor: usernameError ? '#ff4444' : newUsername ? '#ffffff' : 'var(--border)',
            width: '100%',
            textAlign: 'center',
          }}
        />
        {usernameError && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginTop: '6px', fontSize: '0.8rem', color: '#ff4444', justifyContent: 'center' }}>
            <AlertCircle size={14} />
            {usernameError}
          </div>
        )}
        {!usernameError && newUsername && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginTop: '6px', fontSize: '0.8rem', color: '#ffffff', justifyContent: 'center' }}>
            {checkingUsername
              ? <><Loader2 size={14} className="spinner" /> Checking availability�</>
              : <><CheckCircle2 size={14} /> Username available</>
            }
          </div>
        )}
      </div>

      {/* Country selector */}
      <div style={{ marginBottom: '12px' }}>
        <select
          value={country}
          onChange={e => setCountry(e.target.value)}
          required
          style={{ ...inputStyle, width: '100%', textAlign: 'center', cursor: 'pointer' }}
        >
          <option value="" style={{ background: '#1a1a1a', color: '#fff' }}>Select your country</option>
          {COUNTRIES.map(c => (
            <option key={c.code} value={c.code} style={{ background: '#1a1a1a', color: '#fff' }}>{c.label}</option>
          ))}
        </select>
      </div>

      {/* Email field with validation */}
      <div style={{ marginBottom: '12px' }}>
        <input
          type="email"
          value={email}
          onChange={(e) => handleEmailChange(e.target.value)}
          placeholder="Email (required for account recovery)"
          required
          style={{
            ...inputStyle,
            borderColor: emailError ? '#ff4444' : email ? '#ffffff' : 'var(--border)',
            width: '100%',
            textAlign: 'center',
          }}
        />
        {emailError && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginTop: '6px', fontSize: '0.8rem', color: '#ff4444', justifyContent: 'center' }}>
            <AlertCircle size={14} />
            {emailError}
          </div>
        )}
        {!emailError && email && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginTop: '6px', fontSize: '0.8rem', color: '#ffffff', justifyContent: 'center' }}>
            <CheckCircle2 size={14} />
            Valid email format
          </div>
        )}
      </div>

      {/* Create Account Button */}
      <button
        type="submit"
        className="btn btn-primary"
        disabled={creationLoading || !isFormValid}
        style={{ marginBottom: '12px', width: '100%', textAlign: 'center' }}
      >
        {creationLoading ? <Loader2 className="spinner" /> : 'Create Account'}
      </button>

      <p style={{ fontSize: '0.8rem', color: 'var(--text-dim)', textAlign: 'center', margin: '8px 0 0' }}>
        Complete KYC verification after account creation to unlock wagered play.
      </p>
    </form>
  );
}

export function ProfileViewer() {
  const { connection } = useConnection();
  const wallet = useWallet();

  const [profile, setProfile] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Signup state
  const [newUsername, setNewUsername] = useState('');
  const [email, setEmail] = useState('');
  const [country, setCountry] = useState('');
  const [creationLoading, setCreationLoading] = useState(false);
  const [usernameError, setUsernameError] = useState<string | null>(null);
  const [emailError, setEmailError] = useState<string | null>(null);

  // Verification state
  const [status, setStatus] = useState<UserStatus | null>(null);
  const [kycOpen, setKycOpen] = useState(false);

  // Add-email state
  const [emailAddOpen, setEmailAddOpen] = useState(false);
  const [emailAddValue, setEmailAddValue] = useState('');
  const [emailAddLoading, setEmailAddLoading] = useState(false);
  const [emailAddError, setEmailAddError] = useState<string | null>(null);

  const handleAddEmail = async () => {
    const token = localStorage.getItem('xfchess_token');
    if (!token || !emailAddValue) return;
    setEmailAddLoading(true);
    setEmailAddError(null);
    try {
      await addEmail(emailAddValue, token);
      localStorage.setItem('xfchess_email', emailAddValue);
      setEmailAddOpen(false);
      setStatus(prev => prev ? { ...prev, has_email: true } : prev);
    } catch (e: any) {
      setEmailAddError(e.message || 'Failed to add email');
    } finally {
      setEmailAddLoading(false);
    }
  };

  const loadProfile = useCallback(
    async (pubkey: PublicKey) => {
      setLoading(true);
      setError(null);
      try {
        const program = getAnchorProgram(connection, wallet);
        const p = await fetchPlayerProfile(program, pubkey);
        setProfile(p ?? null);
        if (!p && wallet.publicKey?.toBase58() !== pubkey.toBase58()) {
          setError('Profile not found for this address.');
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to load profile.';
        setError(msg);
      } finally {
        setLoading(false);
      }
    },
    [connection, wallet],
  );

  const refreshStatus = useCallback(async () => {
    if (!wallet.publicKey) {
      setStatus(null);
      return;
    }
    try {
      const s = await getUserStatus(wallet.publicKey.toBase58());
      setStatus(s);
    } catch {
      setStatus({ has_profile: false, has_email: false, has_kyc: false, can_wager: false });
    }
  }, [wallet.publicKey]);

  useEffect(() => {
    if (wallet.connected && wallet.publicKey) {
      loadProfile(wallet.publicKey);
      refreshStatus();
    } else {
      setProfile(null);
      setStatus(null);
      setError(null);
    }
  }, [wallet.connected, wallet.publicKey, loadProfile, refreshStatus]);

  const handleCreateProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!wallet.connected || !wallet.publicKey || !wallet.signMessage) return;
    setCreationLoading(true);
    setError(null);
    try {
      // 1. Check username uniqueness against backend before on-chain tx
      const { taken } = await checkUsernameAvailable(newUsername);
      if (taken) {
        setError('Username already taken. Please choose another.');
        setCreationLoading(false);
        return;
      }

      // 2. Create on-chain Anchor profile
      const program = getAnchorProgram(connection, wallet);
      await createPlayerProfile(program, wallet.publicKey, newUsername, country, 0);

      // 3. Register in backend DB (wallet signature proves ownership, no password)
      const timestamp = Math.floor(Date.now() / 1000);
      const msg = new TextEncoder().encode(`xfchess:register:${timestamp}`);
      const sigBytes = await wallet.signMessage(msg);
      const signature = bs58.encode(sigBytes);

      let token = localStorage.getItem('xfchess_token');
      try {
        const auth = await registerWithWallet({
          wallet: wallet.publicKey.toBase58(),
          signature,
          timestamp,
          username: newUsername,
          email: email || null,
        });
        // Fresh registration � store token and username
        token = auth.token;
        localStorage.setItem('xfchess_token', auth.token);
        localStorage.setItem('xfchess_username', auth.username);
        localStorage.setItem('xfchess_wallet', wallet.publicKey.toBase58());
      } catch (regErr: any) {
        // 409 = wallet already registered � existing token is still valid
        if (!regErr.message?.includes('409') && !regErr.message?.includes('already')) {
          console.warn('Backend registration call failed:', regErr);
        }
      }

      // 4. Sync on-chain username ? SQLite (canonical source of truth).
      // Runs for both new and existing wallets � idempotent.
      if (token) {
        try {
          const { username: synced } = await syncProfile(token);
          if (synced) localStorage.setItem('xfchess_username', synced);
        } catch (syncErr) {
          console.warn('sync-profile failed (non-critical):', syncErr);
        }
      }

      // 5. Send welcome email with PDF guide via SendGrid (non-blocking)
      if (email) {
        try {
          await submitSignup({
            email,
            wallet_pubkey: wallet.publicKey.toBase58(),
            username: newUsername,
          });
        } catch (mailErr) {
          console.warn('Welcome email call failed:', mailErr);
        }
      }

      setTimeout(() => {
        if (wallet.publicKey) loadProfile(wallet.publicKey);
        refreshStatus();
      }, 1500);
    } catch (err) {
      const raw = err instanceof Error ? err.message : 'Failed to create profile.';
      const logs = (err as { logs?: string[] }).logs;
      const isDup =
        raw.includes('already in use') ||
        (Array.isArray(logs) && logs.some((l) => l.includes('already in use')));
      setError(
        isDup
          ? 'Username already taken or outdated profile. Please try a different username or a new wallet.'
          : raw,
      );
    } finally {
      setCreationLoading(false);
    }
  };

  const disconnected = !wallet.connected || !wallet.publicKey;
  const hasProfile = !!profile;
  const isSelf = !disconnected && hasProfile;

  return (
    <main className="section" style={{ minHeight: '100vh', paddingTop: '140px' }}>
      <div style={{ maxWidth: '800px', margin: '0 auto', padding: '0 20px' }}>
        {/* Disconnected: Guest / Create / Login */}
        {disconnected && (
          <div className="launch-card" style={{ maxWidth: '600px', marginBottom: '32px', margin: '0 auto' }}>
            <h3 className="launch-title" style={{ fontSize: '1.6rem', textAlign: 'center' }}>Get Started</h3>
            <p className="launch-sub" style={{ textAlign: 'center' }}>Choose how to continue.</p>
            <div className="launch-actions" style={{ justifyContent: 'center' }}>
              <button
                type="button"
                className="launch-btn primary"
                onClick={() => {
                  // Trigger wallet connect modal via adapter visibility
                  const btn = document.querySelector<HTMLButtonElement>(
                    '.wallet-adapter-button-trigger',
                  );
                  btn?.click();
                }}
              >
                <UserPlus size={18} /> Create Account
              </button>
              <Link to="/login" className="launch-btn">
                <LogIn size={18} /> Login
              </Link>
            </div>
          </div>
        )}

        {/* Connected: signup form (no profile yet) */}
        {!disconnected && !loading && !hasProfile && (
          <SignupForm
            newUsername={newUsername}
            setNewUsername={setNewUsername}
            email={email}
            setEmail={setEmail}
            country={country}
            setCountry={setCountry}
            usernameError={usernameError}
            setUsernameError={setUsernameError}
            emailError={emailError}
            setEmailError={setEmailError}
            creationLoading={creationLoading}
            handleCreateProfile={handleCreateProfile}
          />
        )}

        {/* Connected + profile exists: verification checklist */}
        {isSelf && (
          <div className="verify-checklist" style={{ marginBottom: '32px', margin: '0 auto', maxWidth: '600px' }}>
            <h3 style={{ fontSize: '1.2rem', fontWeight: 800, marginBottom: '4px', textAlign: 'center' }}>
              Verification
            </h3>
            <ChecklistRow label="Wallet connected" ok={true} />
            <ChecklistRow
              label="Email registered"
              ok={status?.has_email ?? false}
              action={
                !(status?.has_email) ? (
                  emailAddOpen ? (
                    <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <input
                        type="email"
                        value={emailAddValue}
                        onChange={e => setEmailAddValue(e.target.value)}
                        placeholder="your@email.com"
                        style={{ padding: '4px 8px', borderRadius: 6, border: '1px solid var(--border)', background: 'var(--glass)', color: '#fff', fontSize: 13 }}
                        onKeyDown={e => { if (e.key === 'Enter') handleAddEmail(); }}
                      />
                      <button className="btn-small" onClick={handleAddEmail} disabled={emailAddLoading}>
                        {emailAddLoading ? '�' : 'Save'}
                      </button>
                      <button className="btn-small" style={{ background: 'transparent', opacity: 0.6 }} onClick={() => { setEmailAddOpen(false); setEmailAddError(null); }}></button>
                      {emailAddError && <span style={{ color: '#ff4444', fontSize: 11 }}>{emailAddError}</span>}
                    </span>
                  ) : (
                    <button className="btn-small" onClick={() => setEmailAddOpen(true)}>Add Email</button>
                  )
                ) : undefined
              }
            />
            <ChecklistRow
              label="KYC verified"
              ok={status?.has_kyc ?? false}
              action={
                !status?.has_kyc ? (
                  <button
                    type="button"
                    className="btn-small"
                    onClick={() => setKycOpen(true)}
                  >
                    Complete KYC
                  </button>
                ) : undefined
              }
            />
            <ChecklistRow
              label="Eligible for wagered play"
              ok={status?.can_wager ?? false}
            />
            <ChecklistRow
              label="Lichess linked"
              ok={!!(profile?.data.lichessUsername)}
              action={
                !profile?.data.lichessUsername && wallet.publicKey ? (
                  <button
                    className="btn-small"
                    onClick={async () => {
                      try {
                        const { initLichessLink } = await import('../lib/api/lichess');
                        const { authUrl } = await initLichessLink(wallet.publicKey!.toBase58());
                        window.open(authUrl, 'lichess_oauth', 'width=600,height=700');
                      } catch (err) {
                        alert(err instanceof Error ? err.message : 'Failed to start Lichess link');
                      }
                    }}
                  >
                    Link
                  </button>
                ) : undefined
              }
            />
          </div>
        )}

        {/* Your on-chain profile card */}
        {!disconnected && (loading || hasProfile || error) && (
          <div className="profile-section-wrap" style={{ marginTop: 0, padding: 0, display: 'block' }}>
            <div className="profile-card" style={{ margin: '0 auto', maxWidth: '600px' }}>
              {loading && (
                <div style={{ textAlign: 'center' }}>
                  <Loader2
                    className="spinner"
                    style={{ margin: '0 auto', width: 30, height: 30, color: 'var(--primary)' }}
                  />
                </div>
              )}

              {!loading && profile && (
                <div>
                  <div className="connected-header" style={{ justifyContent: 'center' }}>
                    <div className="connected-avatar">
                      <Shield color="#fff" />
                    </div>
                    <div className="connected-meta" style={{ textAlign: 'center' }}>
                      <h3 style={{ margin: 0, fontSize: '2rem', fontWeight: 900 }}>
                        {profile.data.username ||
                          (isSelf ? 'Set Your Username' : 'Anonymous')}
                      </h3>
                    </div>
                  </div>

                  <div className="connected-stats" style={{ justifyContent: 'center' }}>
                    <div className="cs">
                      <div className="v">{Math.round((profile.data.eloRating ?? 120000) / 100)}</div>
                      <div className="l">Elo Rating</div>
                    </div>
                    <div className="cs">
                      <div className="v">{profile.data.wins || 0}</div>
                      <div className="l">Wins</div>
                    </div>
                    <div className="cs">
                      <div className="v">{profile.data.losses || 0}</div>
                      <div className="l">Losses</div>
                    </div>
                    <div className="cs">
                      <div className="v">{profile.data.winStreak || 0}</div>
                      <div className="l">Streak</div>
                    </div>
                  </div>

                  <LichessLinkCard
                    walletPubkey={wallet.publicKey?.toBase58() ?? null}
                    lichessUsername={profile?.data.lichessUsername}
                    lichessBlitz={profile?.data.lichessBlitz}
                    lichessRapid={profile?.data.lichessRapid}
                    lichessBullet={profile?.data.lichessBullet}
                    lichessVerified={profile?.data.lichessVerified}
                  />

                  <div style={{ marginTop: 24, textAlign: 'center', display: 'flex', flexDirection: 'column', gap: 12, alignItems: 'center' }}>
                    <a
                      href={`xfchess://launch?pubkey=${wallet.publicKey?.toBase58()}&username=${profile.data.username || ''}&token=${localStorage.getItem('xfchess_token') || ''}`}
                      className="btn btn-primary"
                      style={{ display: 'inline-flex', alignItems: 'center', gap: 8, padding: '16px 32px', fontSize: '1.1rem' }}
                    >
                      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                        <polygon points="5 3 19 12 5 21 5 3" />
                      </svg>
                      Launch Game
                    </a>
                    {wallet.publicKey && (
                      <a
                        href={`https://solscan.io/account/${PublicKey.findProgramAddressSync(
                          [Buffer.from("profile"), wallet.publicKey.toBuffer()],
                          PROGRAM_ID
                        )[0].toBase58()}?cluster=devnet`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="btn btn-secondary"
                        style={{ display: 'inline-flex', alignItems: 'center', gap: 8, padding: '12px 24px', fontSize: '0.9rem' }}
                      >
                        View Profile on Solscan
                      </a>
                    )}
                  </div>
                </div>
              )}

              {error && !loading && (
                <div
                  style={{
                    color: 'var(--primary)',
                    marginTop: 20,
                    padding: 16,
                    background: 'rgba(230, 57, 70, 0.1)',
                    borderRadius: 8,
                    border: '1px solid rgba(230, 57, 70, 0.3)',
                    textAlign: 'center',
                  }}
                >
                  {error}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Match history + dispute UI */}
        {!disconnected && wallet.publicKey && hasProfile && (
          <div style={{ marginTop: 40 }}>
            <MatchHistory wallet={wallet.publicKey.toBase58()} />
          </div>
        )}

      </div>

      {kycOpen && wallet.publicKey && (
        <KycModal
          walletPubkey={wallet.publicKey.toBase58()}
          onClose={() => setKycOpen(false)}
          onSuccess={() => {
            setKycOpen(false);
            refreshStatus();
          }}
        />
      )}
    </main>
  );
}

function ChecklistRow({
  label,
  ok,
  action,
}: {
  label: string;
  ok: boolean;
  action?: React.ReactNode;
}) {
  return (
    <div className="verify-row">
      <span className="label">
        {ok ? <CheckCircle2 size={18} color="#ffffff" /> : <XCircle size={18} color="#ff4444" />}
        {label}
      </span>
      {action ?? <span className={`status ${ok ? 'ok' : 'miss'}`}>{ok ? 'OK' : 'Missing'}</span>}
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  padding: '14px 18px',
  borderRadius: 8,
  border: '1px solid var(--border)',
  background: 'var(--glass)',
  color: '#fff',
  fontSize: '1rem',
};



