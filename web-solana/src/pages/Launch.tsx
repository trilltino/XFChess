import { motion } from 'framer-motion';
import { Link } from 'react-router-dom';
import { LogIn, UserPlus } from 'lucide-react';

const Launch = () => {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0 }}
      className="launch-wrap"
    >
      <div className="launch-card">
        <div className="launch-logo">XFChess</div>
        <h1 className="launch-title">Welcome</h1>
        <p className="launch-sub">Choose how to start your session.</p>

        <div className="launch-actions">
          <Link to="/profile" className="launch-btn primary">
            <UserPlus size={18} /> Create Account
          </Link>
          <Link to="/login" className="launch-btn">
            <LogIn size={18} /> Login
          </Link>
        </div>

        <p className="launch-note">
          A wallet, email and KYC are required for PvP wagering and Cash Tournaments.
        </p>
      </div>
    </motion.div>
  );
};

export default Launch;
