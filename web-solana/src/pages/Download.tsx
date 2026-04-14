import { motion } from 'framer-motion';
import { ArrowLeft, Download, MessageCircle, Play } from 'lucide-react';
import { Link } from 'react-router-dom';

const DownloadPage = () => {
  const handleLaunch = () => {
    window.location.href = 'xfchess://play';
  };

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Download</div>
        <h2>Get <span className="accent">XFChess</span></h2>

        <div style={{ display: 'flex', gap: '24px', flexWrap: 'wrap', justifyContent: 'center', marginBottom: '40px' }}>
          <div className="card card-centered" style={{ flex: '1', minWidth: '280px', maxWidth: '400px' }}>
            <div className="card-icon">
              <Download size={48} color="#e63946" />
            </div>
            <div className="card-header">
              <h3 className="card-title">Download XFChess</h3>
            </div>
            <p className="card-content">Download XFChess for your platform from GitHub releases.</p>
            <a 
              href="https://github.com/trilltino/XFChess/releases" 
              target="_blank" 
              rel="noopener noreferrer"
              className="btn btn-primary"
              style={{ width: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}
            >
              <Download size={18} />
              Download from GitHub
            </a>
          </div>

          <div className="card card-centered" style={{ flex: '1', minWidth: '280px', maxWidth: '400px' }}>
            <div className="card-icon" style={{ background: 'rgba(244, 187, 68, 0.1)', borderColor: 'rgba(244, 187, 68, 0.3)' }}>
              <Play size={48} color="#f4bb44" />
            </div>
            <div className="card-header">
              <h3 className="card-title">Already Installed?</h3>
            </div>
            <p className="card-content">Launch XFChess directly from your browser.</p>
            <button 
              onClick={handleLaunch}
              className="btn"
              style={{ width: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px', backgroundColor: '#f4bb44', borderColor: '#f4bb44', color: '#000' }}
            >
              <Play size={18} />
              Launch Game
            </button>
          </div>
        </div>

        <div className="divider" />

        <div className="section-label">Community</div>
        <h3>Join the <span className="accent">Discord</span></h3>

        <div className="card card-centered" style={{ maxWidth: '400px', margin: '0 auto' }}>
          <div className="card-icon" style={{ background: 'rgba(88, 101, 242, 0.1)', borderColor: 'rgba(88, 101, 242, 0.3)' }}>
            <MessageCircle size={48} color="#5865F2" />
          </div>
          <div className="card-header">
            <h3 className="card-title">Join Our Discord</h3>
          </div>
          <p className="card-content">Connect with other players, get support, and stay updated on the latest features.</p>
          <a 
            href="https://discord.gg/erZJCPCm" 
            target="_blank" 
            rel="noopener noreferrer"
            className="btn"
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
