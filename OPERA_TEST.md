# Opera Game On-Chain Test

This test replays the famous **1858 Opera Game** (Paul Morphy vs Duke of Brunswick and Count Isouard) on Solana devnet using XFChess with MagicBlock Ephemeral Rollups.

## What It Does

The test:
1. Creates a game on Solana devnet
2. Both players join and fund the wager escrow
3. Delegates the game to MagicBlock ER for sub-second move processing
4. Records all 33 moves of the historic Opera Game with annotations
5. Undelegates and finalizes the game on devnet
6. Provides Solana Explorer links for every transaction

## Requirements

### Wallet Files
You need two funded keypair files:
- `playtest_white.json` - White player (Morphy)
- `playtest_black.json` - Black player (Duke)

Both wallets need **at least 0.005 SOL** on devnet.

### Get Devnet SOL
```bash
solana airdrop 1 <PUBKEY> --url devnet
```

Or use the [Solana Faucet](https://faucet.solana.com/).

## Running the Test

### Quick Start
```bash
run_opera_test.bat
```

### Manual Run
```bash
cargo run --bin opera_test --features solana --release
```

## The Opera Game

**Paul Morphy vs Duke of Brunswick & Count Isouard**  
Paris Opera House, 1858

This game is considered one of the most brilliant examples of chess tactics in history. Morphy, playing white, sacrifices material to achieve a devastating attack culminating in a back-rank checkmate.

### Key Moves
- **Move 10**: Morphy sacrifices his knight (Nxb5)
- **Move 12**: Sacrifices his bishop (Bxb5)
- **Move 17**: Delivers checkmate with Rd8#

The game demonstrates:
- Rapid development
- Piece coordination
- Tactical brilliance
- Forcing sequences
- Back-rank mate patterns

## Technical Details

### Architecture
```
opera_test.rs
  ├─ Creates game on Solana devnet
  ├─ Delegates to MagicBlock ER
  ├─ Records moves via ER (sub-second)
  ├─ Undelegates back to devnet
  └─ Finalizes game (wager payout)
```

### Move Recording
Each move is recorded with:
- **UCI notation** (e.g., "e2e4")
- **Resulting FEN** (board state after move)
- **Annotation** (tactical/strategic comment)
- **Move hash chain** (cryptographic verification)

### On-Chain Data
- Game PDA: `["game", game_id]`
- MoveLog PDA: `["move_log", game_id]`
- PlayerProfile PDAs: `["profile", player_pubkey]`
- WagerEscrow PDA: `["escrow", game_id]`

## Output

The test provides:
- Real-time move-by-move commentary
- Solana Explorer links for each transaction
- ER transaction signatures
- Final game result and wager distribution
- Complete move history with annotations

### Example Output
```
Move 1: White (Morphy) - e2e4
  Annotation: King's Pawn Opening - Classical start
  Next FEN: rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1
  ✓ Recorded (ER): 5K7x...abc123
  Explorer: https://explorer.solana.com/tx/5K7x...abc123?cluster=devnet&customUrl=https://devnet-eu.magicblock.app

Move 17: White (Morphy) - d1d8
  Annotation: ROOK TO D8# - CHECKMATE!! The Opera Game concludes!
  Next FEN: 1n1Rkb1r/p4ppp/4q3/4p1B1/4P3/8/PPP2PPP/2K5 b k - 1 17
  ✓ Recorded (ER): 9M2k...xyz789
  CHECKMATE! White wins!
```

## Verification

After the test completes, you can verify:

1. **Game State** - Check the Game PDA on Solana Explorer
2. **Move History** - Inspect the MoveLog PDA
3. **Wager Payout** - Verify escrow was paid to winner
4. **ELO Updates** - Check PlayerProfile PDAs for updated ratings

## Troubleshooting

### "Insufficient balance"
Both wallets need at least 0.005 SOL. Request more from the faucet.

### "Account not found"
The ER may need more time to sync. Increase the sleep duration after delegation.

### "Transaction failed"
Check that:
- The program is deployed to devnet
- Program ID matches in `instructions.rs`
- RPC endpoints are accessible

### "Delegation failed"
Ensure:
- MagicBlock ER endpoint is reachable
- Game PDA exists before delegation
- Wallet has sufficient SOL for delegation TX

## Code Structure

```rust
opera_test.rs
├─ main()                          Entry point
├─ load_keypair()                  Load wallet from JSON
├─ create_game_on_chain()          Step 1: Create game
├─ join_game_on_chain()            Step 2: Join game
├─ record_move_on_chain()          Step 3: Record moves on ER
├─ undelegate_game_on_chain()      Step 4: Commit ER → devnet
└─ finalize_game_on_chain()        Step 5: Set winner, pay wager
```

## Integration with XFChess

This test demonstrates the full game flow that the XFChess client uses:

1. **Game Creation** - Same IX builders as the client
2. **ER Delegation** - Uses `MagicBlockResolver`
3. **Move Recording** - Via VPS or direct ER submission
4. **Finalization** - Automatic wager distribution

The test serves as:
- **Integration test** for the full stack
- **Reference implementation** for game flows
- **Demonstration** of ER performance
- **Historical showcase** of a famous chess game

## Links

- **Program ID**: `FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX`
- **Solana Explorer**: https://explorer.solana.com/?cluster=devnet
- **MagicBlock ER**: https://devnet-eu.magicblock.app/
- **Opera Game (Wikipedia)**: https://en.wikipedia.org/wiki/Opera_Game

## Next Steps

After running this test, you can:
1. Inspect the on-chain game state
2. Verify move history in the MoveLog PDA
3. Check ELO updates in PlayerProfile PDAs
4. Use the same flow in the XFChess client
5. Build custom game scenarios

---

**Note**: This test uses real Solana devnet transactions. While devnet SOL is free, the test demonstrates production-ready code that works identically on mainnet.
