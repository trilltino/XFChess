# 🎭 Opera Game On-Chain Test

Complete automated test to play Paul Morphy's famous "Opera Game" (1858) on the Solana blockchain with rich metadata.

## 📋 What This Tests

- ✅ **Game Creation**: Creating competitive games on-chain with wagers
- ✅ **Move Recording**: Recording each move with rich metadata and annotations
- ✅ **Session Validation**: Proper session management and security
- ✅ **Transaction Confirmation**: Real blockchain transaction processing
- ✅ **Historical Preservation**: Permanent storage of chess games with context

## 🎮 The Opera Game (1858)

**Paul Morphy vs. Duke Karl of Brunswick & Count Isouard**
*Played at the Paris Opera House*

A 17-move masterpiece demonstrating:
- Rapid piece development
- Brilliant queen sacrifice
- Tactical combinations
- Decisive checkmate

## 🚀 Quick Start

### 1. Fund Player Addresses

```bash
# Fund White (Morphy)
solana airdrop 1 8SMHifMFg3VFdC8rJ38yRbLwB612EgYk5MhNfxVYY3jc --url devnet

# Fund Black (Duke)  
solana airdrop 1 FJ74VQme1ymF1cSRYHeAi4aADNijcCdAZyPkzUzPVWz --url devnet
```

### 2. Run the Test

#### Option A: Simple Script (Recommended)
```bash
# Windows
run_opera_test.bat

# Or run directly
cargo run --bin run_opera_game_test
```

#### Option B: Full Test Suite
```bash
cargo test --test opera_game_test -- --nocapture
```

## 📊 Expected Output

```
🎭 XFChess Opera Game On-Chain Test
=====================================
👥 Players:
   White (Morphy): 8SMHifMFg3VFdC8rJ38yRbLwB612EgYk5MhNfxVYY3jc
   Black (Duke): FJ74VQme1ymF1cSRYHeAi4aADNijcCdAZyPkzUzPVWz

💰 Checking balances...
   White: 1.0 SOL
   Black: 1.0 SOL
✅ Both players funded - starting game!

🎮 Game ID: 123456
📜 Playing Opera Game (1858) - Paul Morphy vs Duke/Count

📍 Move 1: White (Morphy) - e2e4
   📝 King's Pawn Opening - Classical start
   ✅ Recorded: 3UEUpUQymFExhpAwgLcf4tY4qPs4iCGQ6fkDrhWwXjB87YbKiRUFP1uxTGH3TgLeG8ydynETUmJnrjLU6w9bUuVA
   🔗 https://explorer.solana.com/tx/3UEUpUQymFExhpAwgLcf4tY4qPs4iCGQ6fkDrhWwXjB87YbKiRUFP1uxTGH3TgLeG8ydynETUmJnrjLU6w9bUuVA?cluster=devnet

... (continues for all 17 moves) ...

🏆 Opera Game Complete!
🎯 Final Result: 1-0 (White wins - Morphy)
📖 Historical masterpiece preserved on Solana!
```

## 🔗 Important Links

- **Program**: https://explorer.solana.com/address/2cUpT4EQXT8D6dWQw6WGfxQm897CFKrvmwpjzCNm1Bix?cluster=devnet
- **White Player**: https://explorer.solana.com/address/8SMHifMFg3VFdC8rJ38yRbLwB612EgYk5MhNfxVYY3jc?cluster=devnet
- **Black Player**: https://explorer.solana.com/address/FJ74VQme1ymF1cSRYHeAi4aADNijcCdAZyPkzUzPVWz?cluster=devnet

## 📝 Move Annotations

Each move includes contextual annotations:

### Opening Phase
- `e2e4` → "King's Pawn Opening - Classical start"
- `g1f3` → "Knight development - controls center"
- `d7d6` → "Philidor Defense - Solid but passive"

### Middlegame
- `g4f3` → "Queen sacrifice - Brilliant move"
- `d1b3` → "Queen to b3 - Forks f7 and g4"
- `c3b5` → "Knight takes b5 - Tactical advantage"

### Endgame
- `d1d8` → "ROOK TO D8# - CHECKMATE!"

## 🎯 What Gets Stored On-Chain

### Game Metadata
- Game title and players
- Event location and year
- Opening identification
- Final result and checkmate move

### Move Metadata
- UCI notation for each move
- Contextual annotations
- Timestamps and timing
- Move numbers in proper chess format

### Transaction Data
- All 35+ transactions (1 game creation + 34 moves)
- Cryptographic move hashes for integrity
- Session validation and security
- Wager escrow and distribution

## 🏆 Historical Significance

This test demonstrates the permanent preservation of chess history on blockchain technology. The Opera Game, one of the most famous chess games ever played, is now immortalized on Solana with:

- **Educational Value**: Each move annotated for learning
- **Historical Context**: Full game metadata preserved
- **Blockchain Security**: Cryptographically verified move integrity
- **Global Access**: Anyone can view the game on Solana Explorer

## 🔧 Technical Details

### Program Features
- **Rich Metadata**: Move annotations, timestamps, game context
- **Session Security**: Proper validation and authorization
- **Wager System**: Competitive gameplay with SOL stakes
- **Move Integrity**: Hash chains prevent tampering

### Architecture
- **Anchor Framework**: Solana program development
- **Devnet Testing**: Safe testing environment
- **Type Safety**: Rust's memory safety guarantees
- **Async Operations**: Non-blocking blockchain interactions

## 🎮 Manual Testing

If you prefer to play manually instead of running the automated test:

```bash
# Terminal 1 - White (Morphy)
cargo run --bin xfchess -- \
  --competitive \
  --wager_amount 0.01 \
  --session_key morphy_session \
  --session_pubkey morphy_pubkey \
  --player_color white \
  --p2p_port 5001 \
  --debug

# Terminal 2 - Black (Duke)
cargo run --bin xfchess -- \
  --competitive \
  --wager_amount 0.01 \
  --session_key duke_session \
  --session_pubkey duke_pubkey \
  --player_color black \
  --bootstrap_node <GET_FROM_PLAYER1> \
  --debug
```

Then play the Opera Game moves manually to see the annotations appear in real-time!

## 📈 Test Results

When complete, you'll have:
- ✅ **35+ on-chain transactions**
- ✅ **Complete game with metadata**
- ✅ **Historical chess preservation**
- ✅ **Educational annotations**
- ✅ **Permanent blockchain record**

The Opera Game will live forever on the Solana blockchain! 🏆
