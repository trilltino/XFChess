import { motion } from 'framer-motion';
import { ArrowLeft, Download, Github, Monitor, Terminal, Cpu } from 'lucide-react';
import { Link } from 'react-router-dom';

const DownloadPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Download</div>
        <h2>Get <span className="accent">XFChess</span></h2>

        <p>Download XFChess for your platform and start playing competitive chess with blockchain integration.</p>

        <div className="download-grid">
          <div className="download-card">
            <div className="download-icon">
              <Monitor size={48} color="#e63946" />
            </div>
            <h3>Windows</h3>
            <p>Native Windows application with full 3D graphics and Solana integration.</p>
            <a 
              href="https://github.com/trilltino/XFChess/releases" 
              target="_blank" 
              rel="noopener noreferrer"
              className="download-btn"
            >
              <Download size={18} />
              Download for Windows
            </a>
          </div>

          <div className="download-card">
            <div className="download-icon">
              <Cpu size={48} color="#e63946" />
            </div>
            <h3>macOS</h3>
            <p>Native macOS application with optimized performance for Apple Silicon.</p>
            <a 
              href="https://github.com/trilltino/XFChess/releases" 
              target="_blank" 
              rel="noopener noreferrer"
              className="download-btn"
            >
              <Download size={18} />
              Download for macOS
            </a>
          </div>

          <div className="download-card">
            <div className="download-icon">
              <Terminal size={48} color="#e63946" />
            </div>
            <h3>Linux</h3>
            <p>Native Linux application with support for major distributions.</p>
            <a 
              href="https://github.com/trilltino/XFChess/releases" 
              target="_blank" 
              rel="noopener noreferrer"
              className="download-btn"
            >
              <Download size={18} />
              Download for Linux
            </a>
          </div>
        </div>

        <div className="divider" />

        <div className="section-label">Source Code</div>
        <h3>Build from <span className="accent">Source</span></h3>

        <p>Get the complete source code and build XFChess yourself. The project is open source under MIT/Apache-2.0 license.</p>

        <div className="source-code-card">
          <div className="source-info">
            <Github size={32} color="#e63946" />
            <div>
              <h4>GitHub Repository</h4>
              <p>Complete source code, build instructions, and development documentation.</p>
            </div>
          </div>
          <a 
            href="https://github.com/trilltino/XFChess" 
            target="_blank" 
            rel="noopener noreferrer"
            className="source-btn"
          >
            <Github size={18} />
            View on GitHub
          </a>
        </div>

        <div className="divider" />

        <div className="section-label">Requirements</div>
        <h3>System <span className="accent">Requirements</span></h3>

        <div className="requirements-grid">
          <div className="requirement-item">
            <h4>Minimum</h4>
            <ul>
              <li>OS: Windows 10+, macOS 10.15+, Ubuntu 20.04+</li>
              <li>RAM: 4GB</li>
              <li>Storage: 500MB</li>
              <li>Graphics: OpenGL 3.3+</li>
            </ul>
          </div>
          <div className="requirement-item">
            <h4>Recommended</h4>
            <ul>
              <li>OS: Windows 11+, macOS 12+, Ubuntu 22.04+</li>
              <li>RAM: 8GB</li>
              <li>Storage: 1GB</li>
              <li>Graphics: Dedicated GPU with 2GB+ VRAM</li>
            </ul>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default DownloadPage;
