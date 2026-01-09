# Testing Overview

## Philosophy

XFChess testing follows these core principles:

1. **Test Pyramid**: More unit tests, fewer integration tests, minimal E2E tests
2. **Fast Feedback**: Tests should run quickly to encourage frequent execution
3. **Determinism**: Tests must produce consistent results across runs
4. **Isolation**: No test should depend on another test's execution

## Project Structure

```
XFChess/
├── src/                    # Main client application
│   ├── game/              # Game logic (systems, resources)
│   ├── networking/        # Lightyear client networking
│   ├── rendering/         # 3D rendering (pieces, board)
│   ├── states/            # Application states (menus)
│   └── ui/                # EGUI UI components
├── tests/                  # Integration tests for main crate
│   └── core_tests.rs      # State management tests
├── backend/               # Axum backend server
│   ├── src/               # Server source
│   └── tests/             # Backend integration tests
│       └── room_flow.rs   # Multiplayer room flow tests
├── crates/
│   ├── chess_engine/      # Chess AI and rules
│   │   └── src/           # Engine source (needs unit tests)
│   └── shared/            # Shared protocol types
│       └── src/           # Protocol definitions
└── web/                   # WASM build configuration
```

## Test Types

### Unit Tests
- Location: Inline in source files (`#[cfg(test)] mod tests`)
- Purpose: Test individual functions and components in isolation
- Speed: Fastest (milliseconds per test)

### Integration Tests
- Location: `tests/` directory
- Purpose: Test module interactions
- Speed: Fast (seconds per test)

### End-to-End Tests
- Location: `backend/tests/`
- Purpose: Test full system flows
- Speed: Slower (may involve network/IO)

## Testing Tools

### Bevy Testing
- `MinimalPlugins`: Headless app without rendering
- `ButtonInput<KeyCode>`: Mock keyboard input
- `Messages<T>`: Access sent messages
- `app.update()`: Advance simulation by one frame

### Axum Testing
- `Router::oneshot()`: Single request without server
- `MockConnectInfo`: Mock connection metadata
- `tower::ServiceExt`: Multiple request handling

### Lightyear Testing
- `ClientServerStepper`: Deterministic tick simulation
- `CrossbeamIo`: In-memory client-server channels
- `frame_step()` / `tick_step()`: Controlled time advancement

## File Naming Conventions

- Unit test modules: `mod tests { ... }` inside source files
- Integration test files: `tests/<module>_tests.rs`
- Test utilities: `tests/common/mod.rs`
