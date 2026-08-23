//! Swiss tournament vocabulary over Braid resources.
//!
//! A tournament is a set of Braid resources, exactly like a game is:
//!
//! ```text
//! tournament/{id}/meta              TournamentResource::Meta
//! tournament/{id}/schedule-status   TournamentResource::ScheduleStatus
//! tournament/{id}/roster            TournamentResource::Roster
//! tournament/{id}/standings         TournamentResource::Standings
//! tournament/{id}/pairings/{round}  TournamentResource::Pairings
//! tournament/{id}/results           TournamentResource::Results
//! ```
//!
//! # Why these types live here and not in `braid-iroh`
//!
//! They used to be `braid_iroh::tournament`, broadcast over gossip as bare
//! tagged JSON — no `Version`, no `Parents`, no relation to any resource. That
//! put an application vocabulary inside a *transport* crate, and meant the
//! tournament stream had none of the properties the rest of the Braid stack
//! has, most of all that a late subscriber could not catch up. (The backend
//! carried an unfinished SQLite replay table for that gap; nothing ever wrote
//! to it.)
//!
//! Now the server writes each fact once, into its resource, and the resulting
//! Braid update is what travels — over HTTP `209` to browsers and over gossip
//! to peers. [`SwissMessage`] is no longer a wire format: it is the *decoded*
//! event a client gets back out of an update, via [`SwissMessage::from_braid`].
//! The resource path carries the identity that the old enum tag used to.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One of a tournament's Braid resources, parsed from its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TournamentResource {
    Meta { tournament_id: u64 },
    ScheduleStatus { tournament_id: u64 },
    Roster { tournament_id: u64 },
    Standings { tournament_id: u64 },
    Pairings { tournament_id: u64, round: u8 },
    Results { tournament_id: u64 },
}

impl TournamentResource {
    /// The resource path, without leading slash — the key the server's
    /// `ResourceHub` registers and the suffix of the `/braid/…` URL.
    #[must_use]
    pub fn path(&self) -> String {
        match self {
            Self::Meta { tournament_id } => format!("tournament/{tournament_id}/meta"),
            Self::ScheduleStatus { tournament_id } => {
                format!("tournament/{tournament_id}/schedule-status")
            }
            Self::Roster { tournament_id } => format!("tournament/{tournament_id}/roster"),
            Self::Standings { tournament_id } => format!("tournament/{tournament_id}/standings"),
            Self::Pairings {
                tournament_id,
                round,
            } => format!("tournament/{tournament_id}/pairings/{round}"),
            Self::Results { tournament_id } => format!("tournament/{tournament_id}/results"),
        }
    }

    /// The tournament this resource belongs to.
    #[must_use]
    pub fn tournament_id(&self) -> u64 {
        match self {
            Self::Meta { tournament_id }
            | Self::ScheduleStatus { tournament_id }
            | Self::Roster { tournament_id }
            | Self::Standings { tournament_id }
            | Self::Results { tournament_id }
            | Self::Pairings { tournament_id, .. } => *tournament_id,
        }
    }

    /// Parse a resource path back into a typed resource.
    ///
    /// Accepts an optional leading `/` and an optional `braid/` prefix, so the
    /// same function handles a hub key (`tournament/42/standings`) and a URL
    /// path (`/braid/tournament/42/standings`).
    #[must_use]
    pub fn parse(path: &str) -> Option<Self> {
        let path = path.trim_start_matches('/');
        let path = path.strip_prefix("braid/").unwrap_or(path);

        let mut parts = path.split('/');
        if parts.next()? != "tournament" {
            return None;
        }
        let tournament_id: u64 = parts.next()?.parse().ok()?;

        match parts.next()? {
            "meta" => Some(Self::Meta { tournament_id }),
            "schedule-status" => Some(Self::ScheduleStatus { tournament_id }),
            "roster" => Some(Self::Roster { tournament_id }),
            "standings" => Some(Self::Standings { tournament_id }),
            "results" => Some(Self::Results { tournament_id }),
            "pairings" => {
                let round: u8 = parts.next()?.parse().ok()?;
                Some(Self::Pairings {
                    tournament_id,
                    round,
                })
            }
            _ => None,
        }
    }
}

/// A decoded Swiss tournament event.
///
/// This is **not** a wire format. It is what a client gets back from
/// [`SwissMessage::from_braid`] after reading a Braid update off any transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SwissMessage {
    /// New round pairings available.
    RoundStarted {
        tournament_id: u64,
        round: u8,
        pairings: Vec<SwissPairing>,
    },
    /// Match result recorded.
    ResultRecorded {
        tournament_id: u64,
        round: u8,
        board: u16,
        result: MatchResult,
    },
    /// Standings updated.
    StandingsUpdated {
        tournament_id: u64,
        standings: Vec<SwissStandingsEntry>,
    },
    /// Bracket has fired (async fill or scheduled start).
    BracketFired {
        tournament_id: u64,
        /// Number of players who entered the bracket.
        player_count: u16,
        /// Unix timestamp of the start.
        started_at: i64,
    },
}

/// Media type marking a body that is an RFC 6902 patch document rather than
/// the resource's own state.
pub const JSON_PATCH_MEDIA_TYPE: &str = "application/json-patch+json";

impl SwissMessage {
    /// Decode a Braid update received off any transport.
    ///
    /// Reads the resource path from the update's `url` — over gossip there is
    /// no request line to carry it — and treats the body as state or as a
    /// patch according to its media type.
    #[must_use]
    pub fn from_update(update: &braid_http::types::Update) -> Option<Self> {
        let url = update.url.as_deref()?;
        let body: Value = serde_json::from_slice(update.body.as_ref()?).ok()?;
        let is_snapshot = update.content_type.as_deref() != Some(JSON_PATCH_MEDIA_TYPE);
        Self::from_braid(url, &body, is_snapshot)
    }

    /// Decode one Braid update into a tournament event.
    ///
    /// `url` is the update's resource path and `body` its JSON payload —
    /// a full document for a snapshot, an RFC 6902 patch array otherwise.
    /// Returns `None` for updates this vocabulary has no event for (roster and
    /// meta changes, or a patch shape we do not recognize); a caller streaming
    /// a whole tournament is expected to skip those rather than treat them as
    /// an error.
    #[must_use]
    pub fn from_braid(url: &str, body: &Value, is_snapshot: bool) -> Option<Self> {
        let resource = TournamentResource::parse(url)?;
        let tournament_id = resource.tournament_id();

        match resource {
            TournamentResource::Pairings { round, .. } => {
                let doc = if is_snapshot {
                    body.clone()
                } else {
                    patched_value(body)?
                };
                let pairings: Vec<SwissPairing> = serde_json::from_value(doc).ok()?;
                Some(Self::RoundStarted {
                    tournament_id,
                    round,
                    pairings,
                })
            }
            TournamentResource::Standings { .. } => {
                let doc = if is_snapshot {
                    body.clone()
                } else {
                    patched_value(body)?
                };
                let standings: Vec<SwissStandingsEntry> = serde_json::from_value(doc).ok()?;
                Some(Self::StandingsUpdated {
                    tournament_id,
                    standings,
                })
            }
            TournamentResource::Results { .. } => {
                // The results log is append-only: a live update is an
                // `add /-` patch carrying the one new entry. A snapshot is
                // the whole log, whose last entry is the newest result.
                let entry = if is_snapshot {
                    body.as_array()?.last()?.clone()
                } else {
                    patched_value(body)?
                };
                let recorded: ResultEntry = serde_json::from_value(entry).ok()?;
                Some(Self::ResultRecorded {
                    tournament_id,
                    round: recorded.round,
                    board: recorded.board,
                    result: recorded.result,
                })
            }
            TournamentResource::ScheduleStatus { .. } => {
                let doc = if is_snapshot {
                    body.clone()
                } else {
                    patched_value(body)?
                };
                let status: ScheduleStatus = serde_json::from_value(doc).ok()?;
                if status.status != "started" {
                    return None;
                }
                Some(Self::BracketFired {
                    tournament_id,
                    player_count: status.player_count,
                    started_at: status.started_at,
                })
            }
            TournamentResource::Meta { .. } | TournamentResource::Roster { .. } => None,
        }
    }
}

/// Pull the `value` out of a single-op RFC 6902 patch array.
///
/// Both writers this vocabulary decodes emit exactly one op per update — an
/// `add /-` for the results log, a whole-document `replace` elsewhere — so a
/// patch carrying anything else is not something we can turn into an event.
fn patched_value(body: &Value) -> Option<Value> {
    let ops = body.as_array()?;
    let [op] = ops.as_slice() else { return None };
    op.get("value").cloned()
}

/// One entry in a tournament's `results` append-log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResultEntry {
    pub round: u8,
    pub board: u16,
    pub result: MatchResult,
}

/// The body of a tournament's `schedule-status` resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduleStatus {
    pub status: String,
    pub player_count: u16,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissPairing {
    pub white: String,
    pub black: String,
    pub board: u16,
    /// Set once the orchestrator has created the game for this board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchResult {
    Win { winner: String },
    Draw,
}

impl std::fmt::Display for MatchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchResult::Win { winner } => write!(f, "win:{winner}"),
            MatchResult::Draw => write!(f, "draw"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwissStandingsEntry {
    pub player_id: String,
    pub score: f64,
    pub rank: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resource_paths_round_trip() {
        let cases = [
            TournamentResource::Meta { tournament_id: 1 },
            TournamentResource::ScheduleStatus { tournament_id: 2 },
            TournamentResource::Roster { tournament_id: 3 },
            TournamentResource::Standings { tournament_id: 4 },
            TournamentResource::Results { tournament_id: 5 },
            TournamentResource::Pairings {
                tournament_id: 6,
                round: 7,
            },
        ];
        for case in cases {
            assert_eq!(TournamentResource::parse(&case.path()), Some(case.clone()));
        }
    }

    #[test]
    fn parse_accepts_url_and_hub_key_forms() {
        let want = Some(TournamentResource::Standings { tournament_id: 42 });
        assert_eq!(TournamentResource::parse("tournament/42/standings"), want);
        assert_eq!(TournamentResource::parse("/tournament/42/standings"), want);
        assert_eq!(
            TournamentResource::parse("/braid/tournament/42/standings"),
            want
        );
        assert_eq!(TournamentResource::parse("game/42/moves"), None);
    }

    #[test]
    fn standings_snapshot_decodes() {
        let body = json!([{ "player_id": "alice", "score": 2.5, "rank": 1 }]);
        let msg = SwissMessage::from_braid("tournament/9/standings", &body, true)
            .expect("standings snapshot should decode");
        assert_eq!(
            msg,
            SwissMessage::StandingsUpdated {
                tournament_id: 9,
                standings: vec![SwissStandingsEntry {
                    player_id: "alice".into(),
                    score: 2.5,
                    rank: 1,
                }],
            }
        );
    }

    #[test]
    fn pairings_carry_round_from_the_path() {
        // The round used to travel inside the message tag; now the path is
        // the only thing that says which round these pairings belong to.
        let body = json!([{ "white": "a", "black": "b", "board": 1, "game_id": 77 }]);
        let msg = SwissMessage::from_braid("tournament/9/pairings/3", &body, true)
            .expect("pairings snapshot should decode");
        match msg {
            SwissMessage::RoundStarted {
                round, pairings, ..
            } => {
                assert_eq!(round, 3);
                assert_eq!(pairings[0].game_id, Some(77));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn results_decode_from_the_append_patch() {
        let body = json!([{
            "op": "add",
            "path": "/-",
            "value": { "round": 2, "board": 4, "result": { "Win": { "winner": "white" } } }
        }]);
        let msg = SwissMessage::from_braid("tournament/9/results", &body, false)
            .expect("append patch should decode");
        assert_eq!(
            msg,
            SwissMessage::ResultRecorded {
                tournament_id: 9,
                round: 2,
                board: 4,
                result: MatchResult::Win {
                    winner: "white".into()
                },
            }
        );
    }

    #[test]
    fn schedule_status_only_fires_on_started() {
        let pending = json!({ "status": "scheduled", "player_count": 8, "started_at": 0 });
        assert_eq!(
            SwissMessage::from_braid("tournament/9/schedule-status", &pending, true),
            None
        );

        let started =
            json!({ "status": "started", "player_count": 8, "started_at": 1_700_000_000i64 });
        assert_eq!(
            SwissMessage::from_braid("tournament/9/schedule-status", &started, true),
            Some(SwissMessage::BracketFired {
                tournament_id: 9,
                player_count: 8,
                started_at: 1_700_000_000,
            })
        );
    }
}
