# web-solana/src/components

Shared UI components used across [../pages/](../pages/README.md).

| Component | Purpose |
|-----------|---------|
| [LoginModal.tsx](LoginModal.tsx) | Wallet/email sign-in modal (backend JWT flow) |
| [WalletSelectionModal.tsx](WalletSelectionModal.tsx) | Wallet-adapter picker |
| [KycModal.tsx](KycModal.tsx) | KYC prompt for wager-gated actions |
| [MatchHistory.tsx](MatchHistory.tsx) | Player match list from the archive API |
| [LichessLinkCard.tsx](LichessLinkCard.tsx) | Linked external-rating card |
| [TournamentFeeInfo.tsx](TournamentFeeInfo.tsx) | Entry-fee / prize-split breakdown |
| [CodeViewer.tsx](CodeViewer.tsx) | Syntax-highlighted source viewer |
| [Footer.tsx](Footer.tsx) | Site footer |

Components stay presentation-focused: data fetching lives in the pages or
[../lib/api/](../lib/api/).
