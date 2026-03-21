import { motion } from 'framer-motion';
import { ArrowLeft, Download } from 'lucide-react';
import { Link } from 'react-router-dom';

const DownloadPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Download</div>
        <h2>Get <span className="accent">XFChess</span></h2>

        <div className="download-card" style={{ maxWidth: '400px', margin: '0 auto' }}>
          <div className="download-icon">
            <Download size={48} color="#e63946" />
          </div>
          <h3>Download XFChess</h3>
          <p>Download XFChess for your platform from GitHub releases.</p>
          <a 
            href="https://github.com/trilltino/XFChess/releases/download/latest/XFChess.exe" 
            target="_blank" 
            rel="noopener noreferrer"
            className="download-btn"
          >
            <Download size={18} />
            Download from GitHub
          </a>
        </div>
      </section>
    </motion.div>
  );
};

export default DownloadPage;
