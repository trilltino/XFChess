# chess_engine

## Purpose

Chess engine wrapper providing move generation, position evaluation, and game state validation.

## Role in XFChess

**Core chess logic validation and AI move computation.**

| Usage | Location |
|-------|----------|
| Move validation | `src/game/systems/` |
| AI computation | `src/game/ai/` |
| FEN parsing | Throughout codebase |

## Functionality

- **Move generation**: Legal moves from a position
- **Position evaluation**: Static evaluation scores
- **FEN handling**: Parse and generate FEN strings
- **Game state**: Check, checkmate, stalemate detection

## Architecture

```
┌─────────────────────────────────────────┐
│         XFChess Application             │
├─────────────────────────────────────────┤
│  src/game/ai/      src/game/systems/    │
│  - ChessAIResource  - Move validation   │
│  - Move computation - State updates     │
└────────────┬────────────────────────────┘
             │
             ▼
┌─────────────────────┐
│   chess_engine      │ ◄── YOU ARE HERE
│   - Move gen        │
│   - FEN handling    │
│   - Validation      │
└─────────────────────┘
           │
           ▼
┌─────────────────────┐
│   shakmaty          │
│   (Rust chess lib)  │
└─────────────────────┘
```

## Dependencies

- `shakmaty` - Rust chess library

## Notes

- **Required** for all game modes
- Used by both main app AND on-chain program
- Lightweight wrapper around `shakmaty`
- Critical for ER integration (move validation before commit)
