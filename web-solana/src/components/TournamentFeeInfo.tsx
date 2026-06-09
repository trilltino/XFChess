import React from 'react';

const TournamentFeeInfo: React.FC = () => {
  return (
    <div style={{ padding: '16px', borderRadius: '8px', border: '1px solid var(--border)', background: 'var(--glass)', marginBottom: '16px' }}>
      <p style={{ fontSize: '0.875rem', color: 'var(--text-dim)', margin: 0, lineHeight: 1.6 }}>
        A platform fee of £0.50 is included in the registration cost. This fee helps cover transaction and rent costs for the tournament. Any unused portion contributes to platform revenue. Rent refunds may be provided to players upon account closure.
      </p>
    </div>
  );
};

export default TournamentFeeInfo;
