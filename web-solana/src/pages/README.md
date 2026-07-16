# web-solana/src/pages

One component per route, registered in [../App.tsx](../App.tsx).

## Main flows

| Pages | Flow |
|-------|------|
| [SignIn.tsx](SignIn.tsx), [CreateWallet.tsx](CreateWallet.tsx), [VerifyProfile.tsx](VerifyProfile.tsx) | Auth: wallet connect → backend challenge/JWT → on-chain profile |
| [Play.tsx](Play.tsx), [Launch.tsx](Launch.tsx) | Game launch — hands off to the desktop app via the `xfchess://` deep link / localhost bridge |
| [Tournaments.tsx](Tournaments.tsx), [TournamentDetail.tsx](TournamentDetail.tsx), [TournamentPlay.tsx](TournamentPlay.tsx), [TournamentStandings.tsx](TournamentStandings.tsx) | Tournament browse → register (on-chain) → play → standings |
| [Spectate.tsx](Spectate.tsx), [Players.tsx](Players.tsx), [ProfileViewer.tsx](ProfileViewer.tsx) | Watch live games (delayed feed), browse players |
| [FundWallet.tsx](FundWallet.tsx), [Kyc.tsx](Kyc.tsx), [Compliance.tsx](Compliance.tsx), [Legal.tsx](Legal.tsx) | Funding + regulatory |
| [ChessComputer.tsx](ChessComputer.tsx), [AntiCheat.tsx](AntiCheat.tsx), [Features.tsx](Features.tsx), [Forum.tsx](Forum.tsx), [Home.tsx](Home.tsx), [NewsRelease.tsx](NewsRelease.tsx), [Waitlist.tsx](Waitlist.tsx) | Marketing/info pages |
| [LichessCallback.tsx](LichessCallback.tsx) | OAuth return leg for linked Lichess ratings |

## Conventions

- Pages compose [../components/](../components/) and call the backend only through
  [../lib/api/](../lib/api/).
- Tournament/game pages must route ER-delegated actions through
  [../lib/magicblock.ts](../lib/magicblock.ts) (`getProgramForDelegated`).
