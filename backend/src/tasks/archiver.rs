use crate::db::repository::{GameRecord, GameRepository};
use anyhow::{anyhow, Result};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;
use tracing::{error, info};

/// Path to the binary archive file. Lives under data/ — in production the
/// hardened systemd unit only permits writes to /opt/xfchess/data, and data/
/// is what the nightly backup covers.
const ARCHIVE_PATH: &str = "data/archive/games.xfg";
/// Path to the wallet index file
const WALLET_INDEX_PATH: &str = "data/archive/wallets.idx";

/// Compact binary record for a single game
pub struct BinaryGameRecord {
    pub game_id: u64,
    pub white_idx: u16,
    pub black_idx: u16,
    pub start_time: u64,
    pub stake_lamports: u64,
    pub result: u8, // 0=Draw, 1=White, 2=Black
    pub move_count: u16,
    pub moves: Vec<u16>, // Packed moves: 6 bits from, 6 bits to, 4 bits flags
}

impl BinaryGameRecord {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(31 + (self.moves.len() * 2));
        bytes.extend_from_slice(&self.game_id.to_le_bytes());
        bytes.extend_from_slice(&self.white_idx.to_le_bytes());
        bytes.extend_from_slice(&self.black_idx.to_le_bytes());
        bytes.extend_from_slice(&self.start_time.to_le_bytes());
        bytes.extend_from_slice(&self.stake_lamports.to_le_bytes());
        bytes.push(self.result);
        bytes.extend_from_slice(&self.move_count.to_le_bytes());
        for &m in &self.moves {
            bytes.extend_from_slice(&m.to_le_bytes());
        }
        bytes
    }
}

pub struct Archiver {
    repo: GameRepository,
    wallet_map: HashMap<String, u16>,
    wallets: Vec<String>,
}

impl Archiver {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        if let Some(dir) = Path::new(ARCHIVE_PATH).parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut archiver = Self {
            repo: GameRepository::new(pool),
            wallet_map: HashMap::new(),
            wallets: Vec::new(),
        };
        archiver.load_wallet_index()?;
        Ok(archiver)
    }

    fn load_wallet_index(&mut self) -> Result<()> {
        // Fixed wallet index length method and match syntax
        if Path::new(WALLET_INDEX_PATH).exists() {
            let mut file = File::open(WALLET_INDEX_PATH)?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            for (i, line) in content.lines().enumerate() {
                let wallet = line.trim().to_string();
                if !wallet.is_empty() {
                    self.wallet_map.insert(wallet.clone(), i as u16);
                    self.wallets.push(wallet);
                }
            }
            info!(
                "[Archiver] Loaded {} wallets from index",
                self.wallets.len()
            );
        }
        Ok(())
    }

    fn get_or_create_wallet_idx(&mut self, wallet: &str) -> Result<u16> {
        if let Some(&idx) = self.wallet_map.get(wallet) {
            return Ok(idx);
        }

        let idx = self.wallets.len() as u16;
        self.wallet_map.insert(wallet.to_string(), idx);
        self.wallets.push(wallet.to_string());

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(WALLET_INDEX_PATH)?;
        writeln!(file, "{}", wallet)?;

        Ok(idx)
    }

    pub async fn run_once(&mut self) -> Result<usize> {
        let games = self.repo.get_unarchived_games(100).await?;
        if games.is_empty() {
            return Ok(0);
        }

        let mut archive_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(ARCHIVE_PATH)?;

        let mut count = 0;
        let now = chrono::Utc::now().timestamp();

        for game in games {
            if let Err(e) = self.archive_game(&mut archive_file, &game).await {
                error!("[Archiver] Failed to archive game {}: {}", game.id, e);
                continue;
            }
            self.repo.mark_as_archived(&game.id, now).await?;
            count += 1;
        }

        if count > 0 {
            info!("[Archiver] Successfully archived {} games", count);
        }

        Ok(count)
    }

    async fn archive_game(&mut self, file: &mut File, game: &GameRecord) -> Result<()> {
        let white_wallet = game
            .player_white
            .as_ref()
            .ok_or_else(|| anyhow!("missing white wallet"))?;
        let black_wallet = game
            .player_black
            .as_ref()
            .ok_or_else(|| anyhow!("missing black wallet"))?;

        let white_idx = self.get_or_create_wallet_idx(white_wallet)?;
        let black_idx = self.get_or_create_wallet_idx(black_wallet)?;

        let moves = self.repo.get_moves(&game.id).await?;
        let packed_moves: Vec<u16> = moves.iter().map(|m| pack_move(&m.move_uci)).collect();

        let result = match game.winner.as_deref() {
            Some(w) if Some(w) == game.player_white.as_deref() => 1,
            Some(w) if Some(w) == game.player_black.as_deref() => 2,
            _ => 0,
        };

        let binary_record = BinaryGameRecord {
            game_id: game.id.parse::<u64>().unwrap_or(0), // Assuming numeric ID strings
            white_idx,
            black_idx,
            start_time: game.start_time as u64,
            stake_lamports: (game.stake_amount * 1_000_000_000.0) as u64,
            result,
            move_count: packed_moves.len() as u16,
            moves: packed_moves,
        };

        file.write_all(&binary_record.to_bytes())?;
        Ok(())
    }
}

/// Packs a UCI move (e.g. "e2e4") into 16 bits
/// Format: 6 bits FROM, 6 bits TO, 4 bits FLAGS
fn pack_move(uci: &str) -> u16 {
    if uci.len() < 4 {
        return 0;
    }
    let from_sq = parse_sq(&uci[0..2]);
    let to_sq = parse_sq(&uci[2..4]);

    let mut flags = 0;
    if uci.len() == 5 {
        // Promotion
        flags = match &uci[4..5] {
            "q" => 1,
            "r" => 2,
            "b" => 3,
            "n" => 4,
            _ => 0,
        };
    }

    ((from_sq as u16) & 0x3F) | (((to_sq as u16) & 0x3F) << 6) | ((flags & 0x0F) << 12)
}

fn parse_sq(sq: &str) -> u8 {
    if sq.len() != 2 {
        return 0;
    }
    let file = sq.as_bytes()[0] - b'a';
    let rank = sq.as_bytes()[1] - b'1';
    (rank * 8) + file
}

pub async fn run_archiver_service(pool: SqlitePool) {
    info!("[Archiver] Starting game archiver service");
    let mut archiver = match Archiver::new(pool).await {
        Ok(a) => a,
        Err(e) => {
            error!("[Archiver] Initialization failed: {}", e);
            return;
        }
    };

    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        if let Err(e) = archiver.run_once().await {
            error!("[Archiver] Run failed: {}", e);
        }
    }
}
