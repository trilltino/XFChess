import { useState } from 'react';

interface FundWalletProps {
  walletAddress?: string;
  requiredAmount?: number;
  onFundingComplete?: () => void;
}

export default function FundWallet({ 
  walletAddress, 
  requiredAmount = 0.5,
  onFundingComplete 
}: FundWalletProps) {
  const [selectedProvider, setSelectedProvider] = useState<'moonpay' | 'transak' | 'banxa'>('moonpay');
  const [amount, setAmount] = useState(requiredAmount);
  const [currency, setCurrency] = useState<'USD' | 'EUR' | 'GBP' | 'BRL'>('USD');
  const [isProcessing, setIsProcessing] = useState(false);

  const providers = {
    moonpay: {
      name: 'MoonPay',
      logo: '',
      url: 'https://buy.moonpay.com',
      apiKey: import.meta.env.VITE_MOONPAY_API_KEY || '',
      publishableKey: import.meta.env.VITE_MOONPAY_PUBLISHABLE_KEY || '',
      supportedRegions: ['UK', 'Brazil', 'Germany', 'Canada'],
    },
    transak: {
      name: 'Transak',
      logo: '',
      url: 'https://transak.com',
      apiKey: import.meta.env.VITE_TRANSAK_API_KEY || '',
      supportedRegions: ['UK', 'Brazil', 'Germany', 'Canada'],
    },
    banxa: {
      name: 'Banxa',
      logo: '',
      url: 'https://banxa.com',
      apiKey: import.meta.env.VITE_BANXA_API_KEY || '',
      supportedRegions: ['UK', 'Brazil', 'Germany', 'Canada'],
    },
  };

  const getFundingUrl = () => {
    const provider = providers[selectedProvider];
    const wallet = walletAddress || '';
    
    switch (selectedProvider) {
      case 'moonpay':
        if (!provider.apiKey) {
          console.warn('MoonPay API key not configured');
          return provider.url;
        }
        return `${provider.url}?apiKey=${provider.apiKey}&currencyCode=${currency}&walletAddress=${wallet}&amount=${amount}`;
      case 'transak':
        if (!provider.apiKey) {
          console.warn('Transak API key not configured');
          return provider.url;
        }
        return `${provider.url}/buy?apiKey=${provider.apiKey}&cryptoCurrency=SOL&fiatCurrency=${currency}&walletAddress=${wallet}&amount=${amount}`;
      case 'banxa':
        if (!provider.apiKey) {
          console.warn('Banxa API key not configured');
          return provider.url;
        }
        return `${provider.url}/buy?apiKey=${provider.apiKey}&coin=SOL&fiat=${currency}&wallet=${wallet}&amount=${amount}`;
      default:
        return provider.url;
    }
  };

  const handleFund = () => {
    setIsProcessing(true);
    const fundingUrl = getFundingUrl();
    window.open(fundingUrl, '_blank');
    
    // Simulate funding completion check
    setTimeout(() => {
      setIsProcessing(false);
      onFundingComplete?.();
    }, 30000);
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-purple-900 to-blue-900 flex items-center justify-center p-4">
      <div className="bg-white rounded-2xl shadow-2xl max-w-2xl w-full p-8">
        <h1 className="text-3xl font-bold text-gray-800 mb-2">Fund Your Wallet</h1>
        <p className="text-gray-600 mb-6">
          Purchase SOL to register for the tournament. Supports UK, Brazil, Germany, and Canada.
        </p>

        {walletAddress && (
          <div className="bg-gray-100 rounded-lg p-4 mb-6">
            <p className="text-sm text-gray-600 mb-1">Wallet Address</p>
            <p className="font-mono text-sm text-gray-800 break-all">{walletAddress}</p>
          </div>
        )}

        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Amount (SOL)
          </label>
          <input
            type="number"
            value={amount}
            onChange={(e) => setAmount(parseFloat(e.target.value))}
            min="0.1"
            step="0.1"
            className="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-transparent"
          />
          <p className="text-sm text-gray-500 mt-1">
            Required for tournament: {requiredAmount} SOL
          </p>
        </div>

        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Currency
          </label>
          <select
            value={currency}
            onChange={(e) => setCurrency(e.target.value as any)}
            className="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-purple-500 focus:border-transparent"
          >
            <option value="USD">USD ($)</option>
            <option value="EUR">EUR (€)</option>
            <option value="GBP">GBP (£)</option>
            <option value="BRL">BRL (R$)</option>
          </select>
        </div>

        <div className="mb-6">
          <label className="block text-sm font-medium text-gray-700 mb-3">
            Payment Provider
          </label>
          <div className="grid grid-cols-1 gap-3">
            {Object.entries(providers).map(([key, provider]) => (
              <button
                key={key}
                onClick={() => setSelectedProvider(key as any)}
                className={`p-4 rounded-lg border-2 transition-all ${
                  selectedProvider === key
                    ? 'border-purple-500 bg-purple-50'
                    : 'border-gray-200 hover:border-gray-300'
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="text-2xl">{provider.logo}</span>
                    <div className="text-left">
                      <p className="font-semibold text-gray-800">{provider.name}</p>
                      <p className="text-xs text-gray-500">
                        {provider.supportedRegions.join(', ')}
                      </p>
                    </div>
                  </div>
                  {selectedProvider === key && (
                    <span className="text-purple-500"></span>
                  )}
                </div>
              </button>
            ))}
          </div>
        </div>

        <button
          onClick={handleFund}
          disabled={isProcessing}
          className="w-full bg-gradient-to-r from-purple-600 to-blue-600 text-white py-3 px-6 rounded-lg font-semibold hover:from-purple-700 hover:to-blue-700 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isProcessing ? 'Processing...' : `Buy ${amount} SOL with ${providers[selectedProvider].name}`}
        </button>

        <div className="mt-6 text-center text-sm text-gray-500">
          <p>After funding, return here to complete registration.</p>
          <p className="mt-1">
            Need help?{' '}
            <a href="#" className="text-purple-600 hover:underline">
              Contact Support
            </a>
          </p>
        </div>
      </div>
    </div>
  );
}

