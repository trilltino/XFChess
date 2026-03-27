import { motion } from 'framer-motion';
import { ArrowLeft, Palette, Upload, DollarSign, Rocket, CheckCircle, AlertCircle, Star } from 'lucide-react';
import { Link } from 'react-router-dom';

const CreatorPage = () => {
  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Creator Portal</div>
        <h2>Creator <span className="accent">Onboarding Guide</span></h2>
        <p className="section-subtitle">Empower 3D artists to create and monetize chess assets on the XFChess marketplace.</p>

        {/* Step 1: Design Your Assets */}
        <div className="card" style={{ marginBottom: '2rem' }}>
          <div className="card-header">
            <div className="card-icon" style={{ background: 'rgba(99, 102, 241, 0.1)', borderColor: 'rgba(99, 102, 241, 0.3)' }}>
              <Palette size={48} color="#6366f1" />
            </div>
            <div className="card-title-area">
              <h3 className="card-title">1. Design Your Assets</h3>
              <p className="card-subtitle">Create high-quality 3D models of chess boards and pieces following our technical specifications.</p>
            </div>
          </div>

          <div className="card-content">
            <div className="requirements-grid">
              <div className="requirement-category">
                <h4><CheckCircle size={20} color="#10b981" /> Requirements</h4>
                <ul className="requirement-list">
                  <li><strong>Board:</strong> 2048x2048 texture, PBR materials</li>
                  <li><strong>Pieces:</strong> Individual 3D models, 1024x1024 textures</li>
                  <li><strong>File formats:</strong> .glb, .fbx, or .obj</li>
                  <li><strong>Poly count:</strong> Boards &lt;10K, Pieces &lt;2K each</li>
                </ul>
              </div>

              <div className="requirement-category">
                <h4><Star size={20} color="#f59e0b" /> Recommended Tools</h4>
                <div className="tools-grid">
                  <div className="tool-item">Blender</div>
                  <div className="tool-item">Maya</div>
                  <div className="tool-item">3ds Max</div>
                  <div className="tool-item">Cinema 4D</div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Step 2: Submit for Review */}
        <div className="card" style={{ marginBottom: '2rem' }}>
          <div className="card-header">
            <div className="card-icon" style={{ background: 'rgba(34, 197, 94, 0.1)', borderColor: 'rgba(34, 197, 94, 0.3)' }}>
              <Upload size={48} color="#22c55e" />
            </div>
            <div className="card-title-area">
              <h3 className="card-title">2. Submit for Review</h3>
              <p className="card-subtitle">Upload your designs through our creator portal for quality and performance testing.</p>
            </div>
          </div>

          <div className="card-content">
            <div className="review-criteria">
              <h4><AlertCircle size={20} color="#3b82f6" /> Review Criteria</h4>
              <div className="criteria-grid">
                <div className="criteria-item">
                  <div className="criteria-icon">🎨</div>
                  <div className="criteria-text">
                    <strong>Visual quality and detail</strong>
                    <p>High-quality textures, proper lighting, and attention to detail</p>
                  </div>
                </div>
                <div className="criteria-item">
                  <div className="criteria-icon">⚡</div>
                  <div className="criteria-text">
                    <strong>Performance optimization</strong>
                    <p>Optimized poly counts and efficient texture usage</p>
                  </div>
                </div>
                <div className="criteria-item">
                  <div className="criteria-icon">💡</div>
                  <div className="criteria-text">
                    <strong>Originality and creativity</strong>
                    <p>Unique designs that stand out in the marketplace</p>
                  </div>
                </div>
                <div className="criteria-item">
                  <div className="criteria-icon">🔧</div>
                  <div className="criteria-text">
                    <strong>Technical compliance</strong>
                    <p>Adherence to our technical specifications</p>
                  </div>
                </div>
                <div className="criteria-item">
                  <div className="criteria-icon">📄</div>
                  <div className="criteria-text">
                    <strong>Licensing verification</strong>
                    <p>Proper licensing and ownership verification</p>
                  </div>
                </div>
              </div>
              <div className="timeline-info">
                <strong>Timeline:</strong> 2-3 business days
              </div>
            </div>
          </div>
        </div>

        {/* Step 3: Set Pricing & Royalties */}
        <div className="card" style={{ marginBottom: '2rem' }}>
          <div className="card-header">
            <div className="card-icon" style={{ background: 'rgba(245, 158, 11, 0.1)', borderColor: 'rgba(245, 158, 11, 0.3)' }}>
              <DollarSign size={48} color="#f59e0b" />
            </div>
            <div className="card-title-area">
              <h3 className="card-title">3. Set Pricing & Royalties</h3>
              <p className="card-subtitle">Choose your pricing model and royalty structure. We handle payments and distribution.</p>
            </div>
          </div>

          <div className="card-content">
            <div className="pricing-options">
              <h4><DollarSign size={20} color="#f59e0b" /> Pricing Options</h4>
              <div className="pricing-grid">
                <div className="pricing-card">
                  <div className="pricing-header">
                    <h5>Fixed Price Sales</h5>
                    <div className="royalty-split">
                      <span className="creator-share">70% Creator</span>
                      <span className="platform-share">30% Platform</span>
                    </div>
                  </div>
                  <p>One-time purchases with immediate access to assets</p>
                </div>
                <div className="pricing-card">
                  <div className="pricing-header">
                    <h5>Subscription Access</h5>
                    <div className="royalty-split">
                      <span className="creator-share">60% Creator</span>
                      <span className="platform-share">40% Platform</span>
                    </div>
                  </div>
                  <p>Monthly recurring revenue from premium asset collections</p>
                </div>
                <div className="pricing-card">
                  <div className="pricing-header">
                    <h5>Tournament Exclusives</h5>
                    <div className="royalty-split">
                      <span className="creator-share">50% Creator</span>
                      <span className="platform-share">50% Platform</span>
                    </div>
                  </div>
                  <p>Limited edition assets for special tournaments and events</p>
                </div>
                <div className="pricing-card">
                  <div className="pricing-header">
                    <h5>Charity Collaborations</h5>
                    <div className="royalty-split">
                      <span className="creator-share">80% Creator</span>
                      <span className="platform-share">20% Platform</span>
                    </div>
                  </div>
                  <p>Special charity partnerships with enhanced creator revenue</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Step 4: Launch & Promote */}
        <div className="card" style={{ marginBottom: '2rem' }}>
          <div className="card-header">
            <div className="card-icon" style={{ background: 'rgba(239, 68, 68, 0.1)', borderColor: 'rgba(239, 68, 68, 0.3)' }}>
              <Rocket size={48} color="#ef4444" />
            </div>
            <div className="card-title-area">
              <h3 className="card-title">4. Launch & Promote</h3>
              <p className="card-subtitle">Go live on the XFChess marketplace and access our player community.</p>
            </div>
          </div>

          <div className="card-content">
            <div className="marketing-support">
              <h4><Rocket size={20} color="#ef4444" /> Marketing Support</h4>
              <div className="marketing-grid">
                <div className="marketing-item">
                  <div className="marketing-icon">⭐</div>
                  <div className="marketing-text">
                    <strong>Featured placement</strong>
                    <p>New releases section and homepage highlights</p>
                  </div>
                </div>
                <div className="marketing-item">
                  <div className="marketing-icon">📱</div>
                  <div className="marketing-text">
                    <strong>Social media promotion</strong>
                    <p>Twitter, Instagram, and Discord features</p>
                  </div>
                </div>
                <div className="marketing-item">
                  <div className="marketing-icon">🏆</div>
                  <div className="marketing-text">
                    <strong>Tournament spotlight</strong>
                    <p>Assets featured in competitive tournaments</p>
                  </div>
                </div>
                <div className="marketing-item">
                  <div className="marketing-icon">👤</div>
                  <div className="marketing-text">
                    <strong>Creator profile</strong>
                    <p>Portfolio and creator showcase pages</p>
                  </div>
                </div>
                <div className="marketing-item">
                  <div className="marketing-icon">📊</div>
                  <div className="marketing-text">
                    <strong>Analytics dashboard</strong>
                    <p>Sales data and performance insights</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Quality Standards */}
        <div className="card">
          <div className="card-header">
            <h3 className="card-title">Quality Standards</h3>
          </div>

          <div className="card-content">
            <div className="standards-grid">
              <div className="standard-section">
                <h4><CheckCircle size={20} color="#10b981" /> Technical Requirements</h4>
                <ul className="standards-list">
                  <li>Board textures: 2048x2048 minimum resolution</li>
                  <li>Piece models: Optimized under 2K polygons each</li>
                  <li>PBR materials with metallic/roughness maps</li>
                  <li>Consistent UV unwrapping for texture mapping</li>
                  <li>LOD models for performance optimization</li>
                  <li>File formats: .glb, .fbx, .obj with textures</li>
                </ul>
              </div>

              <div className="standard-section">
                <h4><Star size={20} color="#f59e0b" /> Artistic Standards</h4>
                <ul className="standards-list">
                  <li>Original designs - no copyrighted material</li>
                  <li>Consistent art style within themed sets</li>
                  <li>High-quality textures with proper mipmapping</li>
                  <li>Realistic or stylized with clear visual hierarchy</li>
                  <li>Proper contrast for visibility during gameplay</li>
                  <li>Cultural sensitivity and historical accuracy</li>
                </ul>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default CreatorPage;
