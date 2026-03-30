import { motion } from 'framer-motion';
import { ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';

const DemoPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Demo</div>
        <h2>XFChess <span className="accent">Demo</span></h2>

        <p>Experience XFChess in different modes - from standalone desktop gameplay to blockchain-powered Solana integration.</p>
      </section>
    </motion.div>
  );
};

export default DemoPage;
