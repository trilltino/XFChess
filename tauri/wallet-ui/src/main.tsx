import { Buffer } from "buffer";
(window as any).Buffer = Buffer;

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { PrivyProviderWrapper } from "./privy/PrivyProviderWrapper";
import { trackWalletPopups } from "./wallet/popups";

trackWalletPopups();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <PrivyProviderWrapper>
      <App />
    </PrivyProviderWrapper>
  </React.StrictMode>
);
