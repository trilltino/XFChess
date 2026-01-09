# Performance Testing Guide

Benchmarking and profiling to ensure XFChess runs smoothly.

## Benchmark Infrastructure

### Using Criterion

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "chess_engine_bench"
harness = false
```

### Chess Engine Benchmarks

```rust
// benches/chess_engine_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chess_engine::{new_game, reply, do_move, generate_pseudo_legal_moves};

fn bench_new_game(c: &mut Criterion) {
    c.bench_function("new_game", |b| {
        b.iter(|| black_box(new_game()))
    });
}

fn bench_move_generation(c: &mut Criterion) {
    let game = new_game();
    
    c.bench_function("generate_moves_starting_position", |b| {
        b.iter(|| {
            for square in 0..64 {
                black_box(generate_pseudo_legal_moves(&game, square));
            }
        })
    });
}

fn bench_ai_reply(c: &mut Criterion) {
    let mut group = c.benchmark_group("ai_reply");
    group.sample_size(10); // AI is slow
    
    group.bench_function("depth_4", |b| {
        b.iter(|| {
            let mut game = new_game();
            game.secs_per_move = 0.1;
            black_box(reply(&mut game))
        })
    });
    
    group.finish();
}

fn bench_evaluation(c: &mut Criterion) {
    let game = new_game();
    
    c.bench_function("evaluate_position", |b| {
        b.iter(|| black_box(evaluate(&game)))
    });
}

criterion_group!(
    benches,
    bench_new_game,
    bench_move_generation,
    bench_ai_reply,
    bench_evaluation
);
criterion_main!(benches);
```

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p chess_engine

# Run specific benchmark
cargo bench -p chess_engine -- ai_reply

# Generate HTML report
cargo bench -p chess_engine -- --save-baseline main

# Compare against baseline
cargo bench -p chess_engine -- --baseline main
```

## Frame Time Profiling

### Built-in Bevy Diagnostics

```rust
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(LogDiagnosticsPlugin::default())
        .run();
}
```

### Custom Performance Metrics

```rust
#[derive(Resource, Default)]
pub struct PerformanceMetrics {
    pub frame_times: Vec<f64>,
    pub system_times: HashMap<String, Vec<f64>>,
}

fn measure_system_time(
    name: &str,
    mut metrics: ResMut<PerformanceMetrics>,
) {
    let start = std::time::Instant::now();
    
    // ... system work ...
    
    let elapsed = start.elapsed().as_secs_f64() * 1000.0; // ms
    metrics.system_times
        .entry(name.to_string())
        .or_default()
        .push(elapsed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_time_under_16ms() {
        let mut app = create_game_app();
        
        let start = std::time::Instant::now();
        for _ in 0..100 {
            app.update();
        }
        let elapsed = start.elapsed();
        
        let avg_frame_time = elapsed.as_secs_f64() / 100.0 * 1000.0;
        assert!(
            avg_frame_time < 16.0,
            "Average frame time {}ms exceeds 16ms budget",
            avg_frame_time
        );
    }
}
```

## Memory Profiling

### Tracking Allocations

```rust
#[cfg(feature = "profiling")]
use dhat::{Dhat, DhatAlloc};

#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: DhatAlloc = DhatAlloc;

#[test]
fn test_memory_usage() {
    #[cfg(feature = "profiling")]
    let _dhat = Dhat::start_heap_profiling();
    
    let mut app = create_game_app();
    for _ in 0..1000 {
        app.update();
    }
    
    // dhat will report on drop
}
```

### Entity Count Monitoring

```rust
#[test]
fn test_no_entity_leak() {
    let mut app = create_game_app();
    
    // Start game
    transition_to(&mut app, GameState::InGame);
    let initial_count = app.world().entities().len();
    
    // Play some moves
    for _ in 0..10 {
        simulate_random_move(&mut app);
    }
    
    // Return to menu
    transition_to(&mut app, GameState::MainMenu);
    let final_count = app.world().entities().len();
    
    // Should not leak entities
    assert!(
        final_count <= initial_count + 10,
        "Entity leak: {} -> {} entities",
        initial_count,
        final_count
    );
}
```

## Load Testing (Backend)

```rust
// backend/benches/load_test.rs
use tokio::time::{Duration, Instant};

#[tokio::test]
async fn test_concurrent_connections() {
    let server = spawn_test_server().await;
    
    let mut handles = vec![];
    let start = Instant::now();
    
    // Spawn 100 concurrent clients
    for i in 0..100 {
        let addr = server.addr.clone();
        handles.push(tokio::spawn(async move {
            let client = connect_client(&addr).await;
            client.send(LobbyMessage::JoinRoom { 
                code: format!("ROOM{}", i % 10) 
            }).await;
            client.recv().await
        }));
    }
    
    // Wait for all clients
    let results: Vec<_> = futures::future::join_all(handles).await;
    let elapsed = start.elapsed();
    
    // All should succeed
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 100);
    
    // Should complete in reasonable time
    assert!(elapsed < Duration::from_secs(5));
}
```

## Profiling Tools

### Tracy Integration

```toml
[features]
profiling = ["bevy/trace_tracy"]
```

```rust
#[cfg(feature = "profiling")]
use bevy::log::tracing_subscriber::layer::SubscriberExt;

fn setup_profiling() {
    #[cfg(feature = "profiling")]
    {
        let tracy = tracing_tracy::TracyLayer::new();
        let subscriber = tracing_subscriber::registry().with(tracy);
        tracing::subscriber::set_global_default(subscriber).ok();
    }
}
```

### Running with Tracy

```bash
# Build with profiling
cargo run --release --features profiling

# Open Tracy profiler and connect
```

## Performance Regression Tests

```rust
#[test]
fn test_ai_performance_regression() {
    let mut game = new_game();
    game.secs_per_move = 1.0;
    
    let start = std::time::Instant::now();
    let _move = reply(&mut game);
    let elapsed = start.elapsed();
    
    // AI should respond within allocated time + margin
    assert!(
        elapsed < Duration::from_secs_f64(1.5),
        "AI took {}s, expected < 1.5s",
        elapsed.as_secs_f64()
    );
}
```
