# XFChess Website

## Purpose
The XFChess website is the marketing and documentation hub for the project. It showcases features, provides testing guides, and displays on-chain evidence of the game's blockchain integration.

## Impact on Project
This is the **public-facing presence** of XFChess:
- **Marketing:** Showcases features to potential players
- **Documentation:** Provides testing guides and instructions
- **Evidence:** Displays real Solana transaction data
- **Navigation:** Links to game lobby and resources

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    XFChess Website                           │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │    Home      │  │     Demo     │  │   Evidence   │      │
│  │   (Hero)     │  │  (Testing    │  │ (On-chain    │      │
│  │              │  │   Guide)     │  │   proof)     │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Who/What/   │  │  Multiplayer │  │   Wagering   │      │
│  │    Why       │  │   (P2P)      │  │   (SOL)      │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Pages

### Home (`/`)
- Animated hero section with cycling text
- "Play Anywhere. Own your History." tagline
- Quick navigation to key sections

### Demo (`/demo`)
- Video placeholder (coming soon)
- **Multiplayer Testing Guide** - Step-by-step instructions
- Transaction evidence section

### Evidence (`/evidence`)
- Solana devnet transactions
- MagicBlock ER testing status
- On-chain verification

### Information Pages
- **Who** - Project team and vision
- **What** - Technical overview
- **Why** - Problem statement and solution
- **Multiplayer** - P2P architecture
- **MagicBlock** - ER integration
- **Contracts** - Solana program details
- **Wagering** - Betting mechanics
- **NFT Wagers** - Future NFT integration
- **Ecosystem** - Bots and players
- **Charity** - Giving back

## Key Features

### Multiplayer Testing Guide
Located on Demo page, provides:
1. Start both player UIs (`magicblock_e2e_test.bat`)
2. Player 1 creates game (port 5173)
3. Player 2 joins game (port 5174)
4. Both launch game clients
5. Play and verify on-chain

### Evidence Display
Shows real transaction data:
- Game Delegation
- Wager Initialization
- Player 2 Join
- Move recording
- Finalize/Payout

### Navigation
Dropdown menu structure:
- Demo
- Evidence (Solana/MagicBlock ER)
- 3 W's (Who/What/Why)
- Networking (Multiplayer/MagicBlock)
- Financialised Layer (Contracts/Wagering/NFT/Ecosystem/Charity)

## Technology Stack

- **React 18** - UI framework
- **React Router** - Client-side routing
- **Framer Motion** - Animations
- **Lucide React** - Icons

## Development

```bash
cd web-react
npm install
npm run dev
```

## Build

```bash
npm run build
```

Output goes to `dist/` folder.

## Deployment

The site is deployed to GitHub Pages at:
`https://<username>.github.io/XFChess/`

## Content Strategy

### Target Audience
1. **Chess Players** - Want competitive play with stakes
2. **Crypto Users** - Familiar with Solana wallets
3. **Developers** - Interested in blockchain gaming

### Key Messages
- "Play Anywhere. Own your History."
- "Decentralized chess with real stakes"
- "Every move on-chain, every game provably fair"

## Links to Other Components

| Link | Destination | Purpose |
|------|-------------|---------|
| Demo | `/demo` | Testing instructions |
| Evidence | `/evidence` | Transaction proof |
| Play | `../web-solana` | Game lobby |

## Future Enhancements

- [ ] Demo video
- [ ] Live transaction explorer
- [ ] Leaderboard integration
- [ ] Tournament listings
- [ ] NFT marketplace preview

## Maintenance

Update these when releasing:
- Program ID in Evidence page
- Test transaction signatures
- Feature flags and capabilities
