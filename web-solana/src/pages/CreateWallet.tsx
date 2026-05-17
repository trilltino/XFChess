import { useState } from 'react';

interface CreateWalletProps {
  onWalletCreated?: (address: string) => void;
  onCancel?: () => void;
}

export default function CreateWallet({ onWalletCreated, onCancel }: CreateWalletProps) {
  const [selectedWallet, setSelectedWallet] = useState<'phantom' | 'solflare'>('phantom');
  const [step, setStep] = useState<'select' | 'create' | 'backup'>('select');
  const [backupPhrase, setBackupPhrase] = useState<string[]>([]);
  const [verifiedPhrase, setVerifiedPhrase] = useState<string[]>([]);
  const [isVerified, setIsVerified] = useState(false);

  const wallets = {
    phantom: {
      name: 'Phantom',
      logo: '',
      description: 'Most popular Solana wallet with excellent mobile support',
      downloadUrl: 'https://phantom.app',
      deepLink: 'https://phantom.app/ul/browse/https://xfchess.com',
    },
    solflare: {
      name: 'Solflare',
      logo: '?',
      description: 'Secure wallet with built-in DEX and staking features',
      downloadUrl: 'https://solflare.com',
      deepLink: 'https://solflare.com/ul/browse/https://xfchess.com',
    },
  };

  const generateBackupPhrase = () => {
    const words = [
      'abandon', 'ability', 'able', 'about', 'above', 'absent', 'absorb', 'abstract',
      'absurd', 'abuse', 'access', 'accident', 'account', 'accuse', 'achieve', 'acid',
      'acoustic', 'acquire', 'across', 'act', 'action', 'actor', 'actress', 'actual',
      'adapt', 'add', 'addict', 'address', 'adjust', 'admit', 'adult', 'advance',
      'advice', 'aerobic', 'affair', 'afford', 'afraid', 'again', 'age', 'agent',
    ];
    const phrase = [];
    for (let i = 0; i < 12; i++) {
      const randomIndex = Math.floor(Math.random() * words.length);
      phrase.push(words[randomIndex]);
    }
    setBackupPhrase(phrase);
    setStep('backup');
  };

  const handleWordClick = (word: string) => {
    if (verifiedPhrase.includes(word)) return;
    
    const newVerified = [...verifiedPhrase, word];
    setVerifiedPhrase(newVerified);
    
    if (newVerified.length === 12) {
      setIsVerified(newVerified.join(' ') === backupPhrase.join(' '));
    }
  };

  const handleRemoveWord = (index: number) => {
    const newVerified = verifiedPhrase.filter((_, i) => i !== index);
    setVerifiedPhrase(newVerified);
    setIsVerified(false);
  };

  const handleComplete = () => {
    // In a real implementation, this would create the wallet using the backup phrase
    // For now, we simulate wallet creation
    const mockAddress = 'Mock' + Math.random().toString(36).substring(2, 15);
    onWalletCreated?.(mockAddress);
  };

  const handleDownload = () => {
    window.open(wallets[selectedWallet].downloadUrl, '_blank');
  };

  const handleMobileDeepLink = () => {
    window.location.href = wallets[selectedWallet].deepLink;
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-purple-900 to-blue-900 flex items-center justify-center p-4">
      <div className="bg-white rounded-2xl shadow-2xl max-w-2xl w-full p-8">
        {step === 'select' && (
          <>
            <h1 className="text-3xl font-bold text-gray-800 mb-2">Create Your Wallet</h1>
            <p className="text-gray-600 mb-6">
              Choose a wallet provider to create your Solana wallet. Both support UK, Brazil, Germany, and Canada.
            </p>

            <div className="space-y-4 mb-6">
              {Object.entries(wallets).map(([key, wallet]) => (
                <button
                  key={key}
                  onClick={() => setSelectedWallet(key as any)}
                  className={`w-full p-6 rounded-lg border-2 transition-all ${
                    selectedWallet === key
                      ? 'border-purple-500 bg-purple-50'
                      : 'border-gray-200 hover:border-gray-300'
                  }`}
                >
                  <div className="flex items-center gap-4">
                    <span className="text-4xl">{wallet.logo}</span>
                    <div className="text-left">
                      <p className="font-semibold text-gray-800 text-lg">{wallet.name}</p>
                      <p className="text-sm text-gray-500">{wallet.description}</p>
                    </div>
                    {selectedWallet === key && (
                      <span className="ml-auto text-purple-500 text-2xl"></span>
                    )}
                  </div>
                </button>
              ))}
            </div>

            <div className="space-y-3">
              <button
                onClick={handleDownload}
                className="w-full bg-gradient-to-r from-purple-600 to-blue-600 text-white py-3 px-6 rounded-lg font-semibold hover:from-purple-700 hover:to-blue-700 transition-all"
              >
                Download {wallets[selectedWallet].name}
              </button>
              
              <button
                onClick={handleMobileDeepLink}
                className="w-full bg-gray-100 text-gray-800 py-3 px-6 rounded-lg font-semibold hover:bg-gray-200 transition-all"
              >
                Open in Mobile App
              </button>

              <button
                onClick={generateBackupPhrase}
                className="w-full border-2 border-purple-500 text-purple-600 py-3 px-6 rounded-lg font-semibold hover:bg-purple-50 transition-all"
              >
                Create Wallet with Backup Phrase
              </button>

              {onCancel && (
                <button
                  onClick={onCancel}
                  className="w-full text-gray-500 py-3 px-6 rounded-lg hover:text-gray-700 transition-all"
                >
                  Cancel
                </button>
              )}
            </div>
          </>
        )}

        {step === 'backup' && (
          <>
            <h1 className="text-3xl font-bold text-gray-800 mb-2">Backup Your Recovery Phrase</h1>
            <p className="text-gray-600 mb-6">
              Write down these 12 words in order. This is the ONLY way to recover your wallet if you lose access.
              <span className="text-red-500 font-semibold"> Never share this with anyone.</span>
            </p>

            <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-4 mb-6">
              <div className="grid grid-cols-3 gap-3">
                {backupPhrase.map((word, index) => (
                  <div key={index} className="bg-white p-3 rounded border border-gray-200">
                    <span className="text-xs text-gray-500">{index + 1}.</span>
                    <span className="ml-2 font-mono text-gray-800">{word}</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="mb-6">
              <label className="flex items-center gap-2 mb-4">
                <input
                  type="checkbox"
                  id="writtenDown"
                  className="w-4 h-4 text-purple-600"
                />
                <span className="text-sm text-gray-700">I have written down my recovery phrase</span>
              </label>
            </div>

            <div className="space-y-3">
              <button
                onClick={() => setStep('create')}
                className="w-full bg-gradient-to-r from-purple-600 to-blue-600 text-white py-3 px-6 rounded-lg font-semibold hover:from-purple-700 hover:to-blue-700 transition-all"
              >
                Continue
              </button>

              <button
                onClick={() => setStep('select')}
                className="w-full text-gray-500 py-3 px-6 rounded-lg hover:text-gray-700 transition-all"
              >
                Go Back
              </button>
            </div>
          </>
        )}

        {step === 'create' && (
          <>
            <h1 className="text-3xl font-bold text-gray-800 mb-2">Verify Your Recovery Phrase</h1>
            <p className="text-gray-600 mb-6">
              Click the words in the correct order to verify you've saved your backup phrase.
            </p>

            <div className="mb-6">
              <p className="text-sm text-gray-600 mb-2">Select words:</p>
              <div className="flex flex-wrap gap-2 min-h-[100px] bg-gray-50 p-4 rounded-lg">
                {backupPhrase
                  .filter(word => !verifiedPhrase.includes(word))
                  .sort(() => Math.random() - 0.5)
                  .map((word, index) => (
                    <button
                      key={index}
                      onClick={() => handleWordClick(word)}
                      className="px-4 py-2 bg-white border border-gray-300 rounded-lg hover:border-purple-500 hover:bg-purple-50 transition-all"
                    >
                      {word}
                    </button>
                  ))}
              </div>
            </div>

            <div className="mb-6">
              <p className="text-sm text-gray-600 mb-2">Your selection ({verifiedPhrase.length}/12):</p>
              <div className="flex flex-wrap gap-2 min-h-[60px] bg-gray-50 p-4 rounded-lg">
                {verifiedPhrase.map((word, index) => (
                  <button
                    key={index}
                    onClick={() => handleRemoveWord(index)}
                    className="px-4 py-2 bg-purple-100 border border-purple-300 rounded-lg hover:bg-purple-200 transition-all"
                  >
                    {word} ×
                  </button>
                ))}
              </div>
            </div>

            {isVerified && (
              <div className="bg-green-50 border border-green-200 rounded-lg p-4 mb-6">
                <p className="text-green-700 font-semibold"> Recovery phrase verified!</p>
              </div>
            )}

            <div className="space-y-3">
              <button
                onClick={handleComplete}
                disabled={!isVerified}
                className="w-full bg-gradient-to-r from-purple-600 to-blue-600 text-white py-3 px-6 rounded-lg font-semibold hover:from-purple-700 hover:to-blue-700 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
              >
                Create Wallet
              </button>

              <button
                onClick={() => setStep('backup')}
                className="w-full text-gray-500 py-3 px-6 rounded-lg hover:text-gray-700 transition-all"
              >
                Go Back
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

