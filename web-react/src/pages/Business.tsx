import { motion } from 'framer-motion';
import { ArrowLeft, Calculator, TrendingUp } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';

const BusinessPage = () => {
  const [inputs, setInputs] = useState({
    numberOfGames: 1000,
    platformFeePerPlayer: 0.50,
    corporationTaxRate: 19
  });

  const handleInputChange = (field: string, value: string) => {
    setInputs(prev => ({
      ...prev,
      [field]: parseFloat(value) || 0
    }));
  };

  // Calculations
  const totalRevenue = inputs.numberOfGames * inputs.platformFeePerPlayer * 2; // 2 players per game
  const annualRevenue = totalRevenue * 12;

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="content-wrap page-overlay">
      <section className="section">
        <Link to="/" className="back-btn"><ArrowLeft size={18} /> Back</Link>

        <div className="section-label">Business</div>
        <h2>XFChess <span className="accent">Profit Calculator</span></h2>

        <p>Comprehensive profit calculator for XFChess wagered gaming operations, accounting for UK regulatory costs and transaction fees.</p>

        <div className="calculator-container">
          {/* Input Section */}
          <div className="calculator-section inputs-section">
            <h3><Calculator size={24} /> Input Variables</h3>
            
            <div className="inputs-grid">
              <div className="input-group">
                <label>Number of Games per Month</label>
                <input 
                  type="number" 
                  value={inputs.numberOfGames}
                  onChange={(e) => handleInputChange('numberOfGames', e.target.value)}
                  className="calculator-input"
                />
                <span className="input-hint">Total games played monthly</span>
              </div>

              <div className="input-group">
                <label>Platform Fee per Player (£)</label>
                <input 
                  type="number" 
                  value={inputs.platformFeePerPlayer}
                  onChange={(e) => handleInputChange('platformFeePerPlayer', e.target.value)}
                  className="calculator-input"
                  step="0.01"
                  readOnly
                />
                <span className="input-hint">Fixed at £0.50 per player</span>
              </div>
            </div>
          </div>

          {/* Results Section */}
          <div className="calculator-section results-section">
            <h3><TrendingUp size={24} /> Financial Results</h3>
            
            <div className="results-grid">
              <div className="result-card revenue">
                <h4>Revenue</h4>
                <div className="result-item">
                  <span>Total Monthly Revenue:</span>
                  <span className="value">£{totalRevenue.toLocaleString()}</span>
                </div>
                <div className="result-item">
                  <span>Annual Revenue:</span>
                  <span className="value">£{annualRevenue.toLocaleString()}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default BusinessPage;
