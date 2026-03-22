import { motion } from 'framer-motion';
import { ArrowLeft, Download, MessageCircle } from 'lucide-react';
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
            href="https://github.com/trilltino/XFChess/releases" 
            target="_blank" 
            rel="noopener noreferrer"
            className="download-btn"
          >
            <Download size={18} />
            Download from GitHub
          </a>
        </div>

        <div className="divider" />

        <div className="section-label">Community</div>
        <h3>Join the <span className="accent">Discord</span></h3>

        <div className="download-card" style={{ maxWidth: '400px', margin: '0 auto' }}>
          <div className="download-icon">
            <MessageCircle size={48} color="#5865F2" />
          </div>
          <h3>Join Our Discord</h3>
          <p>Connect with other players, get support, and stay updated on the latest features.</p>
          <a 
            href="https://discord.gg/erZJCPCm" 
            target="_blank" 
            rel="noopener noreferrer"
            className="download-btn"
            style={{ backgroundColor: '#5865F2', borderColor: '#5865F2' }}
          >
            <MessageCircle size={18} />
            Join Discord
          </a>
        </div>
      </section>
    </motion.div>
  );
};

export default DownloadPage;
