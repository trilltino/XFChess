import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Buffer } from 'buffer'
import App from './App.tsx'

// Polyfill Buffer for Solana web3.js in Vite
window.Buffer = window.Buffer || Buffer;
window.global = window.global || window;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
