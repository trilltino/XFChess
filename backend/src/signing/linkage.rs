use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct LinkageStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Default)]
pub struct RegistrationSignals {
    pub funder: Option<String>,
    pub device_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedWallet {
    pub wallet: String,
    pub reason: LinkReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkReason {
    SharedFunder,
    SharedDevice,
}

impl LinkageStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn record_registration(&self, wallet: &str, sig: &RegistrationSignals) {
        let _ = sqlx::query(
            r#"INSERT INTO account_linkage (wallet, funder, device_hash, ip_count, last_seen)
               VALUES (?, ?, ?, 0, strftime('%s','now'))
               ON CONFLICT(wallet) DO UPDATE SET
                   funder      = COALESCE(excluded.funder, account_linkage.funder),
                   device_hash = COALESCE(excluded.device_hash, account_linkage.device_hash),
                   last_seen   = strftime('%s','now')"#,
        )
        .bind(wallet)
        .bind(&sig.funder)
        .bind(&sig.device_hash)
        .execute(&self.pool)
        .await;
    }

    pub async fn bump_ip_count(&self, wallet: &str) {
        let _ = sqlx::query("UPDATE account_linkage SET ip_count = ip_count + 1 WHERE wallet = ?")
            .bind(wallet)
            .execute(&self.pool)
            .await;
    }

    pub async fn linked_wallets(&self, wallet: &str) -> Vec<LinkedWallet> {
        let row = sqlx::query("SELECT funder, device_hash FROM account_linkage WHERE wallet = ?")
            .bind(wallet)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        let Some(row) = row else { return Vec::new() };
        let funder: Option<String> = row.try_get("funder").ok();
        let device: Option<String> = row.try_get("device_hash").ok();

        let mut out = Vec::new();
        if let Some(f) = funder.filter(|s| !s.is_empty()) {
            if let Ok(rows) =
                sqlx::query("SELECT wallet FROM account_linkage WHERE funder = ? AND wallet != ?")
                    .bind(&f)
                    .bind(wallet)
                    .fetch_all(&self.pool)
                    .await
            {
                for r in rows {
                    if let Ok(w) = r.try_get::<String, _>("wallet") {
                        out.push(LinkedWallet {
                            wallet: w,
                            reason: LinkReason::SharedFunder,
                        });
                    }
                }
            }
        }
        if let Some(d) = device.filter(|s| !s.is_empty()) {
            if let Ok(rows) = sqlx::query(
                "SELECT wallet FROM account_linkage WHERE device_hash = ? AND wallet != ?",
            )
            .bind(&d)
            .bind(wallet)
            .fetch_all(&self.pool)
            .await
            {
                for r in rows {
                    if let Ok(w) = r.try_get::<String, _>("wallet") {
                        // Avoid duplicate if already linked by funder.
                        if !out.iter().any(|l| l.wallet == w) {
                            out.push(LinkedWallet {
                                wallet: w,
                                reason: LinkReason::SharedDevice,
                            });
                        }
                    }
                }
            }
        }
        out
    }

    pub async fn flag(&self, wallet: &str) {
        let _ = sqlx::query("UPDATE account_linkage SET flagged = 1 WHERE wallet = ?")
            .bind(wallet)
            .execute(&self.pool)
            .await;
    }

    pub async fn is_hard_blocked(&self, wallet: &str) -> bool {
        sqlx::query_as::<_, (i64,)>("SELECT hard_blocked FROM account_linkage WHERE wallet = ?")
            .bind(wallet)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .map(|(b,)| b != 0)
            .unwrap_or(false)
    }
}

// ── Collusion detection (pure) ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameOutcome {
    pub white: String,
    pub black: String,
    pub winner: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollusionPair {
    pub a: String,
    pub b: String,
    pub games: u32,
    pub directional_bias: f64,
}

const MIN_PAIR_GAMES: u32 = 5;
const DIRECTIONAL_THRESHOLD: f64 = 0.9;

pub fn detect_collusion(outcomes: &[GameOutcome]) -> Vec<CollusionPair> {
    use std::collections::HashMap;

    // Key each pair by (smaller_wallet, larger_wallet) so colors don't split
    // the same two players into two buckets. Value: (total games, games won
    // by the smaller-keyed wallet `a`).
    let mut pairs: HashMap<(String, String), (u32, u32)> = HashMap::new();
    for o in outcomes {
        if o.white == o.black {
            continue;
        }
        let (a, b) = if o.white < o.black {
            (o.white.clone(), o.black.clone())
        } else {
            (o.black.clone(), o.white.clone())
        };
        let entry = pairs.entry((a.clone(), b)).or_insert((0, 0));
        entry.0 += 1;
        if o.winner.as_deref() == Some(a.as_str()) {
            entry.1 += 1;
        }
    }

    let mut flagged = Vec::new();
    for ((a, b), (total, wins_a)) in pairs {
        if total < MIN_PAIR_GAMES {
            continue;
        }
        let wins_a_f = wins_a as f64;
        let total_f = total as f64;
        // Dominant side's share (max of either side's wins / decisive-or-all).
        let bias = (wins_a_f / total_f).max((total_f - wins_a_f) / total_f);
        if bias >= DIRECTIONAL_THRESHOLD {
            flagged.push(CollusionPair {
                a,
                b,
                games: total,
                directional_bias: bias,
            });
        }
    }
    flagged.sort_by_key(|p| std::cmp::Reverse(p.games));
    flagged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(white: &str, black: &str, winner: Option<&str>) -> GameOutcome {
        GameOutcome {
            white: white.into(),
            black: black.into(),
            winner: winner.map(|s| s.to_string()),
        }
    }

    #[test]
    fn flags_one_sided_farming_pair() {
        // "alice" beats "bob" in 8 of 8 games — dumping signature.
        let games: Vec<GameOutcome> = (0..8)
            .map(|i| {
                // alternate colors but alice always wins
                if i % 2 == 0 {
                    g("alice", "bob", Some("alice"))
                } else {
                    g("bob", "alice", Some("alice"))
                }
            })
            .collect();
        let flagged = detect_collusion(&games);
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].games, 8);
        assert!(flagged[0].directional_bias >= 0.9);
    }

    #[test]
    fn balanced_rivalry_is_not_flagged() {
        // Two players, many games, but ~50/50 — a normal rivalry.
        let games: Vec<GameOutcome> = (0..10)
            .map(|i| {
                let w = if i % 2 == 0 { "alice" } else { "bob" };
                g("alice", "bob", Some(w))
            })
            .collect();
        assert!(detect_collusion(&games).is_empty());
    }

    #[test]
    fn too_few_games_abstains() {
        let games = vec![
            g("alice", "bob", Some("alice")),
            g("alice", "bob", Some("alice")),
        ];
        assert!(detect_collusion(&games).is_empty());
    }

    #[test]
    fn diverse_opponents_not_flagged() {
        // alice wins a lot but against many different opponents — skill, not farming.
        let games = vec![
            g("alice", "bob", Some("alice")),
            g("alice", "carol", Some("alice")),
            g("alice", "dave", Some("alice")),
            g("alice", "erin", Some("alice")),
            g("alice", "frank", Some("alice")),
            g("alice", "grace", Some("alice")),
        ];
        assert!(detect_collusion(&games).is_empty());
    }
}
