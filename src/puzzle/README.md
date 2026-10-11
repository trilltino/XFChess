# puzzle — server-checked chess puzzles

The client draws the board and collects moves; the backend
(`backend/src/signing/routes/puzzle.rs`) is the judge. It checks each move and only
then reveals the opponent's reply, so solutions never ship in the client.

## Kinds

| Kind | Endpoint | Source | Notes |
|------|----------|--------|-------|
| Rated | `GET /puzzle/next?mode=solve` | imported Lichess puzzle bank | theme + difficulty filters |
| Daily | `GET /puzzle/daily` | same bank | one per day, same for everyone |
| Lichess | `GET /puzzle/next?source=lichess` | the player's Lichess account | needs wallet JWT + linked Lichess (`puzzle:read`); results sync back (`puzzle:write`) |
| Earn | `GET /puzzle/next?mode=earn` | puzzles with an active SOL bounty | needs a wallet |

Guests can play Rated and Daily under their local node id.

## Flow

1. The menu hub (`hub.rs`) sends `StartPuzzle`; `session.rs` fetches on a background
   thread (`core::net_task::Pending`).
2. On load: `GameMode::Puzzle`, `AiMode::Multiplayer` (the local AI stays idle), the
   player's side goes into `PuzzleBoard`, and the app enters `InGame`. Player setup
   (`game_init::initialize_players`) and the camera read that side.
3. Once pieces spawn, the position is set from the FEN and the opponent's setup move
   plays.
4. Each player move goes to `POST /puzzle/move`: a correct move returns the reply; a
   wrong one ends the puzzle; the last correct move solves it. The server updates the
   puzzle rating (and pays any bounty).
5. `POST /puzzle/hint` reveals the square to move from and makes the round unrated.
6. "Next" re-enters `InGame` with a fresh puzzle; leaving gameplay ends the session.

The chess clock and the game-over prompt are off in `GameMode::Puzzle`; the HUD
(`hud.rs`) shows the result.

## Puzzle bank

The bank is the CC0 Lichess puzzle dump, imported by `backend/src/bin/import_puzzles.rs`
(`just puzzles-refresh`). Lichess asks API users not to enumerate puzzles through the
API, so only the per-player "Lichess" kind calls Lichess live.
