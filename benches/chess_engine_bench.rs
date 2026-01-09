//! Chess Engine Benchmarks
//!
//! Performance benchmarks for critical engine functions using Criterion.

use chess_engine::api::new_game;
use chess_engine::constants::{COLOR_BLACK, COLOR_WHITE};
use chess_engine::evaluation::evaluate_position;
use chess_engine::move_gen::generate_pseudo_legal_moves;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_new_game(c: &mut Criterion) {
    c.bench_function("new_game", |b| b.iter(|| black_box(new_game())));
}

fn bench_move_generation_starting(c: &mut Criterion) {
    let game = new_game();

    c.bench_function("generate_moves_starting_position", |b| {
        b.iter(|| black_box(generate_pseudo_legal_moves(&game, COLOR_WHITE)))
    });
}

fn bench_move_generation_both_colors(c: &mut Criterion) {
    let game = new_game();

    c.bench_function("generate_moves_both_colors", |b| {
        b.iter(|| {
            let white = generate_pseudo_legal_moves(&game, COLOR_WHITE);
            let black = generate_pseudo_legal_moves(&game, COLOR_BLACK);
            black_box((white.len(), black.len()))
        })
    });
}

fn bench_evaluate_position_starting(c: &mut Criterion) {
    let game = new_game();

    c.bench_function("evaluate_position_starting", |b| {
        b.iter(|| black_box(evaluate_position(&game)))
    });
}

fn bench_full_move_cycle(c: &mut Criterion) {
    c.bench_function("full_move_cycle", |b| {
        b.iter(|| {
            let game = new_game();
            let moves = generate_pseudo_legal_moves(&game, COLOR_WHITE);
            let score = evaluate_position(&game);
            black_box((moves.len(), score))
        })
    });
}

criterion_group!(
    benches,
    bench_new_game,
    bench_move_generation_starting,
    bench_move_generation_both_colors,
    bench_evaluate_position_starting,
    bench_full_move_cycle,
);
criterion_main!(benches);
