import { motion } from 'framer-motion';
import { ArrowLeft, Calculator, TrendingUp } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useState } from 'react';

const BusinessPage = () => {
  const [inputs, setInputs] = useState({
    numberOfGames: 1000,
    platformFeePerPlayer: 0.50,
    fcaBaseFee: 2229,
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
  
  const vatThreshold = 90000;
  const shouldChargeVAT = annualRevenue > vatThreshold;
  const vatAmount = shouldChargeVAT ? totalRevenue * 0.20 : 0;
  
  const monthlyFixedCosts = inputs.fcaBaseFee / 12; // Only FCA fee
  
  const netOperatingIncome = totalRevenue - vatAmount - monthlyFixedCosts;
  
  const annualNetProfit = netOperatingIncome * 12;
  const corporationTax = annualNetProfit > 0 ? annualNetProfit * (inputs.corporationTaxRate / 100) : 0;
  const annualNetAfterTax = annualNetProfit - corporationTax;
  
  // Break-even calculation
  const breakEvenFeePerPlayer = monthlyFixedCosts / (inputs.numberOfGames * 2);
  const isProfitable = netOperatingIncome > 0;

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

              <div className="input-group">
                <label>FCA Base Fee (£/year)</label>
                <input 
                  type="number" 
                  value={inputs.fcaBaseFee}
                  onChange={(e) => handleInputChange('fcaBaseFee', e.target.value)}
                  className="calculator-input"
                  readOnly
                />
                <span className="input-hint">Fixed at £2,229 per year</span>
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

              <div className="result-card costs">
                <h4>Monthly Costs</h4>
                <div className="result-item">
                  <span>Fixed Costs (Monthly):</span>
                  <span className="value negative">-£{monthlyFixedCosts.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}</span>
                </div>
                {shouldChargeVAT && (
                  <div className="result-item vat">
                    <span>VAT (20%):</span>
                    <span className="value negative">-£{vatAmount.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}</span>
                  </div>
                )}
              </div>
            <div className="result-card profit">
                <h4>Profitability</h4>
                <div className="result-item">
                  <span>Net Operating Income:</span>
                  <span className={`value ${isProfitable ? 'positive' : 'negative'}`}>
                    £{netOperatingIncome.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}
                  </span>
                </div>
                <div className="result-item">
                  <span>Annual Net Profit:</span>
                  <span className={`value ${annualNetProfit > 0 ? 'positive' : 'negative'}`}>
                    £{annualNetProfit.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}
                  </span>
                </div>
                <div className="result-item">
                  <span>Corporation Tax (19%):</span>
                  <span className="value negative">-£{corporationTax.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}</span>
                </div>
                <div className="result-item">
                  <span>Net After Tax:</span>
                  <span className={`value ${annualNetAfterTax > 0 ? 'positive' : 'negative'}`}>
                    £{annualNetAfterTax.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}
                  </span>
                </div>
              </div>

              <div className="result-card analysis">
                <h4>Key Metrics</h4>
                <div className="result-item">
                  <span>Break-even Fee per Player:</span>
                  <span className="value">£{breakEvenFeePerPlayer.toLocaleString(undefined, {minimumFractionDigits: 2, maximumFractionDigits: 2})}</span>
                </div>
                <div className="result-item">
                  <span>Current vs Break-even:</span>
                  <span className={`value ${isProfitable ? 'positive' : 'negative'}`}>
                    {inputs.platformFeePerPlayer >= breakEvenFeePerPlayer ? '✓ Profitable' : '✗ Loss-making'}
                  </span>
                </div>
                <div className="result-item">
                  <span>VAT Threshold Status:</span>
                  <span className={`value ${shouldChargeVAT ? 'negative' : 'positive'}`}>
                    {shouldChargeVAT ? 'Above £90k (VAT due)' : 'Below £90k (No VAT)'}
                  </span>
                </div>
                <div className="result-item">
                  <span>Monthly Margin:</span>
                  <span className={`value ${isProfitable ? 'positive' : 'negative'}`}>
                    {totalRevenue > 0 ? ((netOperatingIncome / totalRevenue) * 100).toFixed(1) : '0'}%
                  </span>
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
