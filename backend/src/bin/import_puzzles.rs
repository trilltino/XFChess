//! One-off importer for the Lichess open puzzle database (docs/PUZZLES.md §4).
//!
//! Usage:
//!   DATABASE_URL=sqlite://backend.db \
//!     cargo run --bin import_puzzles -- ./lichess_db_puzzle.csv[.zst] \
//!       [--max-dev N] [--min-rating N] [--max-rating N]
//!
//! Streams the Lichess puzzle CSV into the `puzzles` table (migration 018).
//! Idempotent: INSERT OR REPLACE on the primary key. Accepts either a plain
//! `.csv` or a zstd-compressed `.csv.zst` (decompressed on the fly via the
//! `zstd` crate that the backend already depends on).
//!
//! Lichess CSV columns:
//!   0 PuzzleId  1 FEN  2 Moves  3 Rating  4 RatingDeviation
//!   5 Popularity  6 NbPlays  7 Themes  8 GameUrl  9 OpeningTags
//! We store only id, fen, line(=Moves), rating, rating_dev, themes.

use std::fs::File;
use std::io::{BufReader, Read};

use sqlx::sqlite::SqlitePoolOptions;

#[derive(Default)]
struct Filter {
    max_dev: Option<i64>,
    min_rating: Option<i64>,
    max_rating: Option<i64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or_else(|| {
        anyhow::anyhow!(
            "usage: import_puzzles <csv|csv.zst> [--max-dev N] [--min-rating N] [--max-rating N]"
        )
    })?;

    let mut filter = Filter::default();
    while let Some(flag) = args.next() {
        let val = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))?;
        let n: i64 = val.parse()?;
        match flag.as_str() {
            "--max-dev" => filter.max_dev = Some(n),
            "--min-rating" => filter.min_rating = Some(n),
            "--max-rating" => filter.max_rating = Some(n),
            other => anyhow::bail!("unknown flag {other}"),
        }
    }

    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set (e.g. sqlite://backend.db)"))?;
    let pool = SqlitePoolOptions::new().connect(&db_url).await?;

    // Reader that transparently handles a zstd-compressed CSV.
    let file = File::open(&path)?;
    let reader: Box<dyn Read> = if path.ends_with(".zst") {
        Box::new(zstd::Decoder::new(BufReader::new(file))?)
    } else {
        Box::new(BufReader::new(file))
    };
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(reader);

    let mut tx = pool.begin().await?;
    let mut imported = 0u64;
    let mut skipped = 0u64;

    for rec in rdr.records() {
        let r = rec?;
        if r.len() < 8 {
            skipped += 1;
            continue;
        }

        let rating: i64 = r[3].parse().unwrap_or(1500);
        let dev: i64 = r[4].parse().unwrap_or(80);

        if filter.max_dev.is_some_and(|m| dev > m)
            || filter.min_rating.is_some_and(|m| rating < m)
            || filter.max_rating.is_some_and(|m| rating > m)
        {
            skipped += 1;
            continue;
        }

        sqlx::query(
            "INSERT OR REPLACE INTO puzzles (id, fen, line, rating, rating_dev, themes)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&r[0])
        .bind(&r[1])
        .bind(&r[2])
        .bind(rating)
        .bind(dev)
        .bind(&r[7])
        .execute(&mut *tx)
        .await?;

        imported += 1;
        if imported % 50_000 == 0 {
            tx.commit().await?;
            tx = pool.begin().await?;
            eprintln!("  … {imported} imported ({skipped} skipped)");
        }
    }

    tx.commit().await?;
    println!("imported {imported} puzzles ({skipped} skipped)");
    Ok(())
}
