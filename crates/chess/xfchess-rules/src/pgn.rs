use crate::{Game, STARTING_FEN};
use pgn_reader::{Nag, RawComment, RawTag, Reader, SanPlus, Skip, Visitor};
use std::{collections::BTreeMap, io::Cursor, ops::ControlFlow};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveQuality {
    #[default]
    Normal,
    Good,
    Brilliant,
    Interesting,
    Dubious,
    Mistake,
    Blunder,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PerPlyAnnotation {
    pub arrows: Vec<(u8, u8, u8, u8, u8)>,
    pub highlights: Vec<(u8, u8, u8)>,
    pub quality: MoveQuality,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedPgnGame {
    pub tags: BTreeMap<String, String>,
    pub moves: Vec<String>,
    pub result: String,
    /// Index zero is the starting position; index N follows ply N.
    pub per_ply_annotations: Vec<PerPlyAnnotation>,
}
impl ParsedPgnGame {
    pub fn tag(&self, key: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
    pub fn starting_fen(&self) -> &str {
        self.tag("FEN").unwrap_or(STARTING_FEN)
    }
    pub fn snapshots(&self) -> Result<Vec<String>, String> {
        let mut position = Game::from_fen(self.starting_fen())?;
        let mut snapshots = vec![position.fen()];
        for san in &self.moves {
            let uci = position.uci_from_san(san)?;
            position.play_uci(&uci)?;
            snapshots.push(position.fen());
        }
        Ok(snapshots)
    }
}

pub fn ordered_tags(tags: &BTreeMap<String, String>) -> Vec<(&str, &str)> {
    let seven = ["Event", "Site", "Date", "Round", "White", "Black", "Result"];
    seven
        .iter()
        .filter_map(|k| tags.get(*k).map(|v| (*k, v.as_str())))
        .chain(
            tags.iter()
                .filter(|(k, _)| !seven.contains(&k.as_str()))
                .map(|(k, v)| (k.as_str(), v.as_str())),
        )
        .collect()
}

pub fn pgn_to_string(pgn: &ParsedPgnGame) -> String {
    let mut out = String::new();
    let mut tags = pgn.tags.clone();
    tags.insert("Result".into(), pgn.result.clone());
    for (key, value) in ordered_tags(&tags) {
        out.push_str(&format!(
            "[{key} \"{}\"]\n",
            value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace(['\n', '\r'], " ")
        ));
    }
    out.push('\n');
    if let Some(comment) = pgn
        .per_ply_annotations
        .first()
        .and_then(|ann| ann.comment.as_ref())
    {
        out.push_str(&format!("{{{}}} ", comment.replace(['{', '}'], "")));
    }
    let start: Vec<_> = pgn.starting_fen().split_whitespace().collect();
    let first_black = start.get(1) == Some(&"b");
    let first_number: usize = start.get(5).and_then(|v| v.parse().ok()).unwrap_or(1);
    for (ply, san) in pgn.moves.iter().enumerate() {
        let index = ply + usize::from(first_black);
        if index % 2 == 0 {
            out.push_str(&format!("{}. ", first_number + index / 2));
        } else if ply == 0 {
            out.push_str(&format!("{}... ", first_number));
        }
        out.push_str(san);
        if let Some(ann) = pgn.per_ply_annotations.get(ply + 1) {
            let nag = match ann.quality {
                MoveQuality::Good => 1,
                MoveQuality::Mistake => 2,
                MoveQuality::Brilliant => 3,
                MoveQuality::Blunder => 4,
                MoveQuality::Interesting => 5,
                MoveQuality::Dubious => 6,
                MoveQuality::Normal => 0,
            };
            if nag > 0 {
                out.push_str(&format!(" ${nag}"));
            }
            if let Some(comment) = &ann.comment {
                out.push_str(&format!(" {{{}}}", comment.replace(['{', '}'], "")));
            }
        }
        out.push(' ');
    }
    out.push_str(&pgn.result);
    out.push('\n');
    out
}

fn square(s: &str) -> Option<(u8, u8)> {
    let [file, rank] = *s.as_bytes() else {
        return None;
    };
    Some((
        file.checked_sub(b'a').filter(|v| *v < 8)?,
        rank.checked_sub(b'1').filter(|v| *v < 8)?,
    ))
}
fn annotation(comment: &str, ann: &mut PerPlyAnnotation) {
    let text = ann.comment.get_or_insert_with(String::new);
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(comment);
    for (tag, is_arrow) in [("[%cal ", true), ("[%csl ", false)] {
        for part in comment.split(tag).skip(1) {
            let Some((values, _)) = part.split_once(']') else {
                continue;
            };
            for item in values.split(',').map(str::trim) {
                if !item.is_ascii() || item.len() != if is_arrow { 5 } else { 3 } {
                    continue;
                }
                let color = match item.as_bytes()[0] {
                    b'G' => 0,
                    b'R' | b'Y' => 1,
                    b'B' => 2,
                    _ => continue,
                };
                let Some((x, y)) = square(&item[1..3]) else {
                    continue;
                };
                if is_arrow {
                    if let Some((dx, dy)) = square(&item[3..5]) {
                        ann.arrows.push((x, y, dx, dy, color));
                    }
                } else {
                    ann.highlights.push((x, y, color));
                }
            }
        }
    }
}

struct Parser;
struct Movetext {
    parsed: ParsedPgnGame,
    position: Game,
}
impl Visitor for Parser {
    type Tags = ParsedPgnGame;
    type Movetext = Movetext;
    type Output = Result<ParsedPgnGame, String>;
    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(ParsedPgnGame::default())
    }
    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        tags.tags.insert(
            String::from_utf8_lossy(name).into_owned(),
            value.decode_utf8_lossy().into_owned(),
        );
        ControlFlow::Continue(())
    }
    fn begin_movetext(
        &mut self,
        mut parsed: Self::Tags,
    ) -> ControlFlow<Self::Output, Self::Movetext> {
        let position = match Game::from_fen(parsed.starting_fen()) {
            Ok(p) => p,
            Err(e) => return ControlFlow::Break(Err(e)),
        };
        parsed.result = parsed.tag("Result").unwrap_or("*").to_string();
        parsed.per_ply_annotations.push(PerPlyAnnotation::default());
        ControlFlow::Continue(Movetext { parsed, position })
    }
    fn san(&mut self, state: &mut Self::Movetext, san: SanPlus) -> ControlFlow<Self::Output> {
        let san = san.to_string();
        let uci = match state.position.uci_from_san(&san) {
            Ok(m) => m,
            Err(e) => {
                return ControlFlow::Break(Err(format!(
                    "Ply {} ({san}): {e}",
                    state.parsed.moves.len() + 1
                )))
            }
        };
        if let Err(e) = state.position.play_uci(&uci) {
            return ControlFlow::Break(Err(e));
        }
        state.parsed.moves.push(san);
        state
            .parsed
            .per_ply_annotations
            .push(PerPlyAnnotation::default());
        ControlFlow::Continue(())
    }
    fn nag(&mut self, state: &mut Self::Movetext, nag: Nag) -> ControlFlow<Self::Output> {
        if let Some(ann) = state.parsed.per_ply_annotations.last_mut() {
            ann.quality = match nag.0 {
                1 => MoveQuality::Good,
                2 => MoveQuality::Mistake,
                3 => MoveQuality::Brilliant,
                4 => MoveQuality::Blunder,
                5 => MoveQuality::Interesting,
                6 => MoveQuality::Dubious,
                _ => ann.quality,
            };
        }
        ControlFlow::Continue(())
    }
    fn comment(
        &mut self,
        state: &mut Self::Movetext,
        comment: RawComment<'_>,
    ) -> ControlFlow<Self::Output> {
        if let Some(ann) = state.parsed.per_ply_annotations.last_mut() {
            annotation(&String::from_utf8_lossy(comment.as_bytes()), ann);
        }
        ControlFlow::Continue(())
    }
    fn begin_variation(&mut self, _: &mut Self::Movetext) -> ControlFlow<Self::Output, Skip> {
        ControlFlow::Continue(Skip(true))
    }
    fn outcome(
        &mut self,
        state: &mut Self::Movetext,
        outcome: pgn_reader::Outcome,
    ) -> ControlFlow<Self::Output> {
        state.parsed.result = outcome.to_string();
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, state: Self::Movetext) -> Self::Output {
        Ok(state.parsed)
    }
}
pub fn parse_pgn(text: &str) -> Result<ParsedPgnGame, String> {
    Reader::new(Cursor::new(text.as_bytes()))
        .read_game(&mut Parser)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No PGN game found".to_string())?
}

#[derive(Clone, Copy, Debug)]
pub enum PgnResult {
    WhiteWins,
    BlackWins,
    Draw,
    Unfinished,
}
impl PgnResult {
    fn as_str(self) -> &'static str {
        match self {
            Self::WhiteWins => "1-0",
            Self::BlackWins => "0-1",
            Self::Draw => "1/2-1/2",
            Self::Unfinished => "*",
        }
    }
}
#[derive(Default)]
pub struct PgnAssembler(ParsedPgnGame);
impl PgnAssembler {
    pub fn new() -> Self {
        let mut this = Self::default();
        this.set_result(PgnResult::Unfinished);
        this
    }
    pub fn tag(&mut self, key: &str, value: &str) -> &mut Self {
        self.0.tags.insert(key.into(), value.into());
        self
    }
    pub fn add_move(&mut self, san: String) {
        self.0.moves.push(san);
    }
    pub fn set_result(&mut self, result: PgnResult) {
        self.0.result = result.as_str().into();
    }
}
impl std::fmt::Display for PgnAssembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&pgn_to_string(&self.0))
    }
}
