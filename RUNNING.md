# How to Run XFChess

## Stack Overflow Fix

This game requires a larger stack size than the default 2MB due to:
- Complex 3D mesh generation during game initialization
- Bevy's internal task pool operations
- Chess engine deep search algorithms

### Windows

**Option 1: PowerShell (Recommended)**
```powershell
.\run.ps1
```

**Option 2: Command Prompt**
```cmd
run.bat
```

### DO NOT use `cargo run` directly

Running `cargo run --release` directly will cause a stack overflow crash because the RUST_MIN_STACK environment variable must be set BEFORE the Rust program starts, not during runtime.

## Current Stack Size Settings

- **run.ps1 / run.bat**: 16MB stack (16777216 bytes)
- This should be sufficient for most gameplay scenarios

## If You Still Experience Crashes

If stack overflows persist, you can increase the stack size further:

1. Edit `run.ps1` or `run.bat`
2. Change `RUST_MIN_STACK` value:
   - 32MB: `33554432`
   - 64MB: `67108864`

## Technical Details

The stack overflow occurs in Bevy 0.17's Compute Task Pool threads during:
1. Initial board creation (64 squares with observers)
2. Piece spawning (32 pieces with parent/child entities)
3. Observer registration for click handling
4. AI computation after moves

The fix works by setting the `RUST_MIN_STACK` environment variable before program execution, which affects all subsequently spawned threads including Bevy's task pools.
