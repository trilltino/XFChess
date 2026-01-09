# XFChess Testing Strategy

This document provides a comprehensive guide to testing the XFChess project. It covers all modules and provides a staged implementation plan.

## Table of Contents

1. [00-overview.md](./00-overview.md) - Testing philosophy and structure
2. [01-unit-testing.md](./01-unit-testing.md) - Unit testing fundamentals
3. [02-ecs-testing.md](./02-ecs-testing.md) - Bevy ECS and game logic testing
4. [03-chess-engine-testing.md](./03-chess-engine-testing.md) - Chess engine crate testing
5. [04-backend-api-testing.md](./04-backend-api-testing.md) - Axum backend API testing
6. [05-networking-testing.md](./05-networking-testing.md) - Lightyear multiplayer testing
7. [06-ui-testing.md](./06-ui-testing.md) - bevy_egui UI testing
8. [07-integration-testing.md](./07-integration-testing.md) - End-to-end integration tests
9. [08-wasm-testing.md](./08-wasm-testing.md) - WebAssembly/browser testing
10. [09-performance-testing.md](./09-performance-testing.md) - Benchmarking and profiling
11. [10-cicd-pipeline.md](./10-cicd-pipeline.md) - CI/CD automation

## Implementation Stages

### Stage 1: Foundation (Week 1)
- Set up test infrastructure
- Add unit tests for `chess_engine` crate
- Add unit tests for `shared` protocol crate

### Stage 2: Game Logic (Week 2)
- ECS component and system tests
- State management tests
- Game rules validation

### Stage 3: Backend (Week 3)
- API endpoint unit tests
- Database integration tests
- WebSocket/message handler tests

### Stage 4: Networking (Week 4)
- Client-server communication tests
- Multiplayer flow tests
- Connection reliability tests

### Stage 5: Integration (Week 5)
- Full game flow tests
- Cross-module integration
- WASM build verification

### Stage 6: Performance and CI (Week 6)
- Benchmark suite
- CI/CD pipeline setup
- Coverage reporting

## Quick Start

Run all tests:
```bash
cargo test --workspace
```

Run specific crate tests:
```bash
cargo test -p chess_engine
cargo test -p shared
cargo test -p backend
```

Run with verbose output:
```bash
cargo test -- --nocapture
```

## Test Coverage Goals

| Module | Target Coverage |
|--------|-----------------|
| chess_engine | 90% |
| shared | 85% |
| backend | 80% |
| src/game | 75% |
| src/networking | 70% |
| src/ui | 60% |

## Key Principles

1. **Headless Testing**: Use `MinimalPlugins` instead of `DefaultPlugins` for fast, deterministic tests.
2. **Isolation**: Each test should be independent with no shared mutable state.
3. **Mocking**: Mock external dependencies (network, filesystem, time).
4. **Assertions**: Use descriptive assertion messages for clear failure diagnostics.
5. **Documentation**: Every test should have a doc comment explaining what it verifies.
