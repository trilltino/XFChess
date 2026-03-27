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

        <div className="page-section">
          <h3><Calculator size={24} /> Input Variables</h3>
          
          <div className="grid-2">
            <div className="card">
              <div className="card-header">
                <h4 className="card-title">Number of Games per Month</h4>
              </div>
              <input 
                type="number" 
                value={inputs.numberOfGames}
                onChange={(e) => handleInputChange('numberOfGames', e.target.value)}
                className="btn btn-secondary"
                style={{marginBottom: '8px'}}
              />
              <p className="card-subtitle">Total games played monthly</p>
            </div>

            <div className="card">
              <div className="card-header">
                <h4 className="card-title">Platform Fee per Player (£)</h4>
              </div>
              <input 
                type="number" 
                value={inputs.platformFeePerPlayer}
                onChange={(e) => handleInputChange('platformFeePerPlayer', e.target.value)}
                className="btn btn-secondary"
                step="0.01"
                readOnly
                style={{marginBottom: '8px'}}
              />
              <p className="card-subtitle">Fixed at £0.50 per player</p>
            </div>
          </div>
        </div>

        <div className="page-section">
          <h3><TrendingUp size={24} /> Financial Results</h3>
          
          <div className="grid-2">
            <div className="card">
              <div className="card-header">
                <h4 className="card-title">Revenue</h4>
              </div>
              <div className="list-check">
                <li><strong>Total Monthly Revenue:</strong> £{totalRevenue.toLocaleString()}</li>
                <li><strong>Annual Revenue:</strong> £{annualRevenue.toLocaleString()}</li>
              </div>
            </div>
          </div>
        </div>
      </section>
    </motion.div>
  );
};

export default BusinessPage;
