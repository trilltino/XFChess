# XFChess AI Service

AI service for XFChess with Stockfish integration.

## Features

- REST API for chess move evaluation
- Stockfish engine integration
- Health check endpoints
- Async/await support

## API Endpoints

### GET /health
Returns service health status.

### POST /move
Accepts a FEN position and returns the best move.

Request:
```json
{
  "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
  "player_side": "white"
}
```

Response:
```json
{
  "best_move": "e2e4",
  "evaluation": 0,
  "depth": 15
}
```

## Running

```bash
cargo run --bin xfchess-ai-service
```

The service will start on `http://localhost:8080`.

## Development

This is a placeholder implementation. In production, it would integrate with the Stockfish chess engine for accurate move evaluation.
