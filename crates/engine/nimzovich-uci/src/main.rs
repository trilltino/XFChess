//! UCI protocol adapter for the nimzovich chess engine.
//!
//! Minimal synchronous implementation for engine-vs-engine testing with
//! cutechess-cli and GUIs. The search itself is time-bounded, so `stop` is a
//! no-op (the engine always returns within its allotted budget).
//!
//! Extra non-UCI commands:
//!   bench   — fixed position set, prints total nodes + NPS
//!   perft N — perft from the current position
//!
//! Usage: cargo run --release --bin nimzovich-uci

use std::io::{self, BufRead, Write};

use futures_lite::future::block_on;
use nimzovich_engine::api::game::{game_to_fen, new_game, set_game_from_fen, set_tt_size_mb};
use nimzovich_engine::book::book_move;
use nimzovich_engine::perft::perft;
use nimzovich_engine::{do_move_with_promo, is_legal_move, reply, Game};

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

const ENGINE_NAME: &str = "Nimzovich 0.1";
const ENGINE_AUTHOR: &str = "XFChess";

/// Standard bench positions: a spread of opening / middlegame / endgame.
const BENCH_FENS: &[&str] = &[
    START_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "3r1rk1/p4ppp/2pb4/1p6/3P4/1QP2N2/PP3PPP/R4RK1 b - - 0 18",
    "8/8/1p1k4/p1p2p2/P1P2P2/1P1K4/8/8 w - - 0 1",
    "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
];

struct Engine {
    game: Game,
    /// Side to move: 1 = white, -1 = black.
    stm: i64,
    /// Move list since startpos (None when the game began from a FEN) —
    /// used for opening-book lookups.
    startpos_moves: Option<Vec<String>>,
}

impl Engine {
    fn new() -> Self {
        Self { game: new_game(), stm: 1, startpos_moves: Some(Vec::new()) }
    }

    fn set_position(&mut self, fen: &str, moves: &[&str]) {
        set_game_from_fen(&mut self.game, fen);
        self.stm = if fen.split_whitespace().nth(1) == Some("b") { -1 } else { 1 };
        self.startpos_moves = if fen == START_FEN {
            Some(moves.iter().map(|m| m.to_string()).collect())
        } else {
            None
        };
        for uci in moves {
            if let Some((src, dst, promo)) = parse_uci_move(uci) {
                do_move_with_promo(&mut self.game, src, dst, true, promo);
                self.stm = -self.stm;
            } else {
                eprintln!("info string ignoring unparseable move {uci}");
            }
        }
    }

    /// Try the opening book. Returns true if a (validated legal) book move
    /// was emitted.
    fn try_book(&mut self) -> bool {
        let Some(history) = &self.startpos_moves else { return false };
        let played: Vec<&str> = history.iter().map(|s| s.as_str()).collect();
        let Some(book) = book_move(&played) else { return false };
        let Some((src, dst, _promo)) = parse_uci_move(book) else { return false };
        // A book typo must degrade to "out of book", never to a forfeit.
        if !is_legal_move(&mut self.game, src, dst, self.stm) {
            eprintln!("info string book move {book} failed legality check — searching");
            return false;
        }
        println!("info depth 0 score cp 0 nodes 0 time 0 pv {book} string book");
        println!("bestmove {book}");
        let _ = io::stdout().flush();
        true
    }

    fn go(&mut self, budget_secs: f32, depth_limit: Option<i64>) {
        if depth_limit.is_none() && self.try_book() {
            return;
        }
        self.game.abs_max_depth = depth_limit.unwrap_or(64);
        self.game.secs_per_move = budget_secs;
        let start = std::time::Instant::now();
        let mv = block_on(reply(&mut self.game, self.stm));
        let elapsed = start.elapsed().as_secs_f64().max(1e-6);

        let nodes = self.game.calls;
        let nps = (nodes as f64 / elapsed) as u64;
        let uci = move_to_uci(mv.src as i8, mv.dst as i8, mv.promo);
        println!(
            "info depth {} score cp {} nodes {} nps {} time {} pv {}",
            self.game.max_depth_so_far,
            mv.score,
            nodes,
            nps,
            (elapsed * 1000.0) as u64,
            uci,
        );
        println!("bestmove {uci}");
        let _ = io::stdout().flush();
    }
}

/// "e2e4" / "e7e8q" → (src, dst, promo_id). Promo ids: n=2 b=3 r=4 q=5, 0 = none.
fn parse_uci_move(s: &str) -> Option<(i8, i8, i8)> {
    let b = s.as_bytes();
    if b.len() < 4 {
        return None;
    }
    let sq = |f: u8, r: u8| -> Option<i8> {
        if (b'a'..=b'h').contains(&f) && (b'1'..=b'8').contains(&r) {
            Some(((r - b'1') * 8 + (f - b'a')) as i8)
        } else {
            None
        }
    };
    let src = sq(b[0], b[1])?;
    let dst = sq(b[2], b[3])?;
    let promo = if b.len() >= 5 {
        match b[4] {
            b'n' => 2,
            b'b' => 3,
            b'r' => 4,
            b'q' => 5,
            _ => 0,
        }
    } else {
        0
    };
    Some((src, dst, promo))
}

fn move_to_uci(src: i8, dst: i8, promo_id: i8) -> String {
    let file = |sq: i8| (b'a' + (sq % 8) as u8) as char;
    let rank = |sq: i8| (b'1' + (sq / 8) as u8) as char;
    let promo = match promo_id {
        5 => "q",
        4 => "r",
        3 => "b",
        2 => "n",
        _ => "",
    };
    format!("{}{}{}{}{}", file(src), rank(src), file(dst), rank(dst), promo)
}

/// Compute a think budget in seconds from `go` arguments.
fn parse_go_budget(tokens: &[&str], stm: i64) -> f32 {
    let get = |name: &str| -> Option<f64> {
        tokens
            .iter()
            .position(|t| *t == name)
            .and_then(|i| tokens.get(i + 1))
            .and_then(|v| v.parse::<f64>().ok())
    };

    if let Some(mt) = get("movetime") {
        return (mt / 1000.0 * 0.9).max(0.02) as f32;
    }

    let (time, inc) = if stm > 0 {
        (get("wtime"), get("winc").unwrap_or(0.0))
    } else {
        (get("btime"), get("binc").unwrap_or(0.0))
    };

    if let Some(t) = time {
        // The engine enforces a hard in-search deadline at 95% of the budget,
        // so a moderately generous allocation is safe.
        let budget = t / 1000.0 / 28.0 + inc / 1000.0 * 0.8;
        return budget.clamp(0.02, 30.0) as f32;
    }

    // "go depth N" / "go infinite": the engine's internal depth cap bounds this.
    10.0
}

fn fen_color(fen: &str) -> i64 {
    if fen.split_whitespace().nth(1) == Some("b") { -1 } else { 1 }
}

fn bench(hash_mb: usize) {
    let mut total_nodes: i64 = 0;
    let start = std::time::Instant::now();
    for fen in BENCH_FENS {
        let mut eng = Engine::new();
        set_tt_size_mb(&mut eng.game, hash_mb);
        eng.set_position(fen, &[]);
        eng.game.secs_per_move = 1.0;
        let _ = block_on(reply(&mut eng.game, fen_color(fen)));
        total_nodes += eng.game.calls;
    }
    let elapsed = start.elapsed().as_secs_f64().max(1e-6);
    println!(
        "bench: {} positions, {} nodes, {:.0} nps, {:.2}s",
        BENCH_FENS.len(),
        total_nodes,
        total_nodes as f64 / elapsed,
        elapsed,
    );
}

fn main() {
    let mut engine = Engine::new();
    let mut hash_mb: usize = 64;
    set_tt_size_mb(&mut engine.game, hash_mb);

    // Allow `nimzovich-uci bench` from the command line (standard convention).
    if std::env::args().nth(1).as_deref() == Some("bench") {
        bench(hash_mb);
        return;
    }

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        // Tolerate UTF-8/UTF-16 BOMs and stray NULs from Windows shells.
        let line = line.trim_start_matches(['\u{feff}', '\0']);
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(&cmd) = tokens.first() else { continue };

        match cmd {
            "uci" => {
                println!("id name {ENGINE_NAME}");
                println!("id author {ENGINE_AUTHOR}");
                println!("option name Hash type spin default 64 min 1 max 1024");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "setoption" => {
                // setoption name Hash value N
                if let (Some(ni), Some(vi)) = (
                    tokens.iter().position(|t| *t == "name"),
                    tokens.iter().position(|t| *t == "value"),
                ) {
                    let name = tokens.get(ni + 1).copied().unwrap_or("");
                    let value = tokens.get(vi + 1).copied().unwrap_or("");
                    if name.eq_ignore_ascii_case("hash") {
                        if let Ok(mb) = value.parse::<usize>() {
                            hash_mb = mb.clamp(1, 1024);
                            set_tt_size_mb(&mut engine.game, hash_mb);
                        }
                    }
                }
            }
            "ucinewgame" => {
                engine = Engine::new();
                set_tt_size_mb(&mut engine.game, hash_mb);
            }
            "position" => {
                let moves_idx = tokens.iter().position(|t| *t == "moves");
                let moves: Vec<&str> = match moves_idx {
                    Some(i) => tokens[i + 1..].to_vec(),
                    None => Vec::new(),
                };
                match tokens.get(1) {
                    Some(&"startpos") => engine.set_position(START_FEN, &moves),
                    Some(&"fen") => {
                        let end = moves_idx.unwrap_or(tokens.len());
                        let fen = tokens[2..end].join(" ");
                        engine.set_position(&fen, &moves);
                    }
                    _ => eprintln!("info string bad position command"),
                }
            }
            "go" => {
                let budget = parse_go_budget(&tokens[1..], engine.stm);
                let depth_limit = tokens
                    .iter()
                    .position(|t| *t == "depth")
                    .and_then(|i| tokens.get(i + 1))
                    .and_then(|v| v.parse::<i64>().ok());
                engine.go(budget, depth_limit);
            }
            "stop" => {
                // Search is synchronous and time-bounded; bestmove was already
                // printed by the time we read this. Nothing to do.
            }
            "perft" => {
                let depth: u32 = tokens.get(1).and_then(|d| d.parse().ok()).unwrap_or(4);
                let color = engine.stm;
                let start = std::time::Instant::now();
                let nodes = perft(&mut engine.game, depth, color);
                println!(
                    "perft({depth}) = {nodes}  ({:.2}s)",
                    start.elapsed().as_secs_f64()
                );
            }
            "bench" => bench(hash_mb),
            "d" => {
                println!("fen: {}", game_to_fen(&engine.game));
                println!("stm: {}", if engine.stm > 0 { "white" } else { "black" });
            }
            "quit" => break,
            _ => {}
        }
        let _ = io::stdout().flush();
    }
}
