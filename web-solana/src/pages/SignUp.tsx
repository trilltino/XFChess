import { useState } from 'react';
import { motion } from 'framer-motion';
import { ArrowRight, Check } from 'lucide-react';

export function SignUp() {
  const [formData, setFormData] = useState({
    email: '',
    referral: '',
  });
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isSubmitted, setIsSubmitted] = useState(false);

  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSubmitting(true);
    setError(null);
    
    try {
      // Get backend URL from env or use default
      const backendUrl = import.meta.env.VITE_BACKEND_URL || 'http://localhost:8090';
      
      // Call backend to send welcome PDF email
      const response = await fetch(`${backendUrl}/signup`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: formData.email,
          referral: formData.referral || null,
        }),
      });
      
      if (response.ok) {
        setIsSubmitted(true);
      } else {
        const errText = await response.text();
        setError(`Failed to send: ${errText}`);
      }
    } catch (err) {
      setError('Network error. Please try again.');
    } finally {
      setIsSubmitting(false);
    }
  };

  if (isSubmitted) {
    return (
      <div className="signup-container">
        <div className="signup-success">
          <motion.div
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            transition={{ type: "spring", stiffness: 200, damping: 15 }}
          >
            <Check size={64} style={{ color: '#00ffa3', marginBottom: '24px' }} />
          </motion.div>
          <h2>You're on the List!</h2>
          <p>Thanks for signing up. Check your email for the XFChess tournament guide PDF.</p>
          <button 
            className="signup-btn"
            onClick={() => window.location.href = '/'}
          >
            Back to Home
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="signup-container">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
        className="signup-header"
      >
        <h1>
          Join the Tournament
        </h1>
      </motion.div>

      <motion.form
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5, delay: 0.1 }}
        className="signup-form"
        onSubmit={handleSubmit}
      >
        <div className="form-group">
          <label>EMAIL</label>
          <input
            type="email"
            value={formData.email}
            onChange={(e) => setFormData({ ...formData, email: e.target.value })}
            required
          />
        </div>

        <div className="form-group">
          <label>HOW'D YOU HEAR ABOUT US? <span className="optional">OPTIONAL</span></label>
          <input
            type="text"
            value={formData.referral}
            onChange={(e) => setFormData({ ...formData, referral: e.target.value })}
          />
        </div>

        {error && (
          <div style={{ 
            color: '#ff4444', 
            fontSize: '14px', 
            marginBottom: '16px',
            padding: '12px',
            background: 'rgba(255, 68, 68, 0.1)',
            borderRadius: '8px',
            border: '1px solid rgba(255, 68, 68, 0.2)'
          }}>
            {error}
          </div>
        )}

        <button 
          type="submit" 
          className="signup-btn"
          disabled={isSubmitting}
        >
          {isSubmitting ? (
            'Registering...'
          ) : (
            <>
              Register <ArrowRight size={16} style={{ marginLeft: '8px' }} />
            </>
          )}
        </button>
      </motion.form>

      <div className="onboarding-bg"></div>

      <style>{`
        .signup-container {
          min-height: 100vh;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 40px 20px;
          background: var(--bg, #081a14);
          position: relative;
        }

        .signup-header {
          text-align: center;
          max-width: 480px;
          margin-bottom: 40px;
        }

        .signup-logo {
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 8px;
          margin-bottom: 32px;
          font-size: 14px;
          font-weight: 700;
          letter-spacing: 0.1em;
          color: #fff;
        }

        .signup-header h1 {
          font-size: 36px;
          font-weight: 800;
          margin-bottom: 16px;
        }

        .gradient-text {
          background: linear-gradient(135deg, #00ffa3 0%, #00d4aa 100%);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
          background-clip: text;
        }

        .signup-subtitle {
          font-size: 15px;
          line-height: 1.6;
          color: #888;
          margin-bottom: 24px;
        }

        .signup-perks {
          display: inline-flex;
          flex-direction: column;
          gap: 8px;
          background: rgba(255, 255, 255, 0.03);
          border: 1px solid rgba(255, 255, 255, 0.08);
          border-radius: 12px;
          padding: 16px 20px;
          font-family: 'SF Mono', monospace;
          font-size: 12px;
        }

        .perk {
          color: #00ffa3;
        }

        .perk + .perk {
          color: #666;
        }

        .signup-form {
          width: 100%;
          max-width: 400px;
          background: rgba(173, 92, 47, 0.1);
          border: 1px solid rgba(173, 92, 47, 0.3);
          border-radius: 16px;
          padding: 32px;
        }

        .form-group {
          margin-bottom: 20px;
        }

        .form-group label {
          display: block;
          font-size: 11px;
          font-weight: 600;
          letter-spacing: 0.1em;
          color: #fff;
          margin-bottom: 8px;
        }

        .form-group .optional {
          color: rgba(255, 255, 255, 0.5);
          font-weight: 400;
        }

        .form-group input {
          width: 100%;
          padding: 14px 16px;
          background: rgba(0, 0, 0, 0.3);
          border: 1px solid rgba(173, 92, 47, 0.4);
          border-radius: 10px;
          color: #fff;
          font-size: 15px;
          transition: all 0.2s;
        }

        .form-group input:focus {
          outline: none;
          border-color: #00ffa3;
          background: rgba(0, 0, 0, 0.5);
        }

        .form-group input::placeholder {
          color: rgba(255, 255, 255, 0.4);
        }

        .signup-btn {
          width: 100%;
          padding: 16px;
          background: rgba(173, 92, 47, 0.8);
          color: #fff;
          font-size: 14px;
          font-weight: 700;
          letter-spacing: 0.05em;
          border: 1px solid rgba(173, 92, 47, 0.5);
          border-radius: 10px;
          cursor: pointer;
          display: flex;
          align-items: center;
          justify-content: center;
          transition: all 0.2s;
        }

        .signup-btn:hover:not(:disabled) {
          transform: translateY(-2px);
          background: rgba(173, 92, 47, 1);
          box-shadow: 0 8px 24px rgba(173, 92, 47, 0.4);
        }

        .signup-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .signup-success {
          text-align: center;
          max-width: 400px;
        }

        .signup-success h2 {
          font-size: 28px;
          font-weight: 700;
          color: #00ffa3;
          margin-bottom: 16px;
        }

        .signup-success p {
          color: rgba(255, 255, 255, 0.7);
          margin-bottom: 32px;
          line-height: 1.6;
        }
      `}</style>
    </div>
  );
}
