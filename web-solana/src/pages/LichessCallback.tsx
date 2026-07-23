import { useEffect, useState } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import { completeLichessLink } from '../lib/api/lichess';
import { SeoHead } from '../components/SeoHead';
import { PRIVATE_PAGE_METADATA } from '../lib/seo/metadata';

export function LichessCallback() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const [status, setStatus] = useState<'loading' | 'success' | 'error'>('loading');
  const [message, setMessage] = useState('Completing Lichess link...');
  const [result, setResult] = useState<{
    username: string;
    blitz: number;
    rapid: number;
    bullet: number;
    seededElo: number;
    txSig: string;
  } | null>(null);

  useEffect(() => {
    const code = searchParams.get('code');
    const state = searchParams.get('state');
    const error = searchParams.get('error');

    if (error) {
      setStatus('error');
      setMessage(`Lichess error: ${error}`);
      return;
    }

    if (!code || !state) {
      setStatus('error');
      setMessage('Missing authorization code or state from Lichess redirect.');
      return;
    }

    completeLichessLink(code, state)
      .then((res) => {
        setResult({
          username: res.lichess_username,
          blitz: res.blitz_rating,
          rapid: res.rapid_rating,
          bullet: res.bullet_rating,
          seededElo: res.seeded_elo,
          txSig: res.tx_signature,
        });
        setStatus('success');
        setMessage('Lichess account linked successfully!');
        // Redirect to profile after 3 seconds
        setTimeout(() => navigate('/profile'), 3000);
      })
      .catch((err: Error) => {
        setStatus('error');
        setMessage(err.message || 'Failed to link Lichess account.');
      });
  }, [searchParams, navigate]);

  return (
    <div className="min-h-screen bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900 flex items-center justify-center p-4">
      <SeoHead meta={PRIVATE_PAGE_METADATA.lichessCallback} />
      <div className="bg-slate-800/80 border border-slate-700 rounded-xl p-8 max-w-md w-full text-center shadow-2xl">
        {status === 'loading' && (
          <>
            <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-amber-400 mx-auto mb-4" />
            <h2 className="text-xl font-bold text-amber-400 mb-2">Linking Lichess...</h2>
            <p className="text-slate-300">{message}</p>
          </>
        )}

        {status === 'success' && result && (
          <>
            <div className="text-5xl mb-4">♟️</div>
            <h2 className="text-xl font-bold text-green-400 mb-2">Linked!</h2>
            <p className="text-slate-300 mb-4">{message}</p>
            <div className="bg-slate-900/50 rounded-lg p-4 text-left text-sm space-y-1">
              <p><span className="text-slate-400">Username:</span> <span className="text-white font-medium">{result.username}</span></p>
              <p><span className="text-slate-400">Blitz:</span> <span className="text-white">{result.blitz}</span></p>
              <p><span className="text-slate-400">Rapid:</span> <span className="text-white">{result.rapid}</span></p>
              <p><span className="text-slate-400">Bullet:</span> <span className="text-white">{result.bullet}</span></p>
              <p><span className="text-slate-400">Seeded ELO:</span> <span className="text-amber-400 font-bold">{result.seededElo}</span></p>
            </div>
            <p className="text-xs text-slate-500 mt-4">Redirecting to profile...</p>
          </>
        )}

        {status === 'error' && (
          <>
            <div className="text-5xl mb-4">⚠️</div>
            <h2 className="text-xl font-bold text-red-400 mb-2">Link Failed</h2>
            <p className="text-slate-300 mb-4">{message}</p>
            <button
              onClick={() => navigate('/profile')}
              className="bg-amber-500 hover:bg-amber-600 text-slate-900 font-bold py-2 px-6 rounded-lg transition-colors"
            >
              Back to Profile
            </button>
          </>
        )}
      </div>
    </div>
  );
}
