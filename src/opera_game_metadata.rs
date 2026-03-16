//! 🎭 Opera Game Metadata System
//! Rich metadata and historical context for the Opera Game on-chain recording

use std::time::{SystemTime, UNIX_EPOCH};

/// Opera Game move metadata with rich annotations
#[derive(Debug, Clone)]
pub struct OperaGameMove {
    pub move_number: usize,
    pub player: Player,
    pub move_notation: String,
    pub annotation: String,
    pub fen: String,
    pub timestamp: String,
    pub historical_significance: String,
    pub tactical_analysis: String,
}

#[derive(Debug, Clone)]
pub enum Player {
    White,
    Black,
}

impl Player {
    pub fn name(&self) -> &'static str {
        match self {
            Player::White => "Paul Morphy",
            Player::Black => "Duke of Brunswick & Count Isouard",
        }
    }
    
    pub fn color(&self) -> &'static str {
        match self {
            Player::White => "White",
            Player::Black => "Black",
        }
    }
}

/// Complete Opera Game metadata
pub struct OperaGameMetadata {
    pub game_info: GameInfo,
    pub moves: Vec<OperaGameMove>,
    pub historical_context: HistoricalContext,
    pub wager_info: WagerInfo,
}

#[derive(Debug, Clone)]
pub struct GameInfo {
    pub title: String,
    pub date: String,
    pub location: String,
    pub event: String,
    pub result: String,
    pub total_moves: usize,
    pub program_id: String,
    pub network: String,
}

#[derive(Debug, Clone)]
pub struct HistoricalContext {
    pub significance: String,
    pub background: String,
    pub legacy: String,
    pub interesting_facts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WagerInfo {
    pub amount_per_player: f64,
    pub total_pool: f64,
    pub currency: String,
    pub winner: Player,
    pub payout: f64,
}

impl OperaGameMetadata {
    pub fn new() -> Self {
        let moves = Self::generate_opera_game_moves();
        
        Self {
            game_info: GameInfo {
                title: "The Opera Game".to_string(),
                date: "1858".to_string(),
                location: "Paris Opera House, France".to_string(),
                event: "Simultaneous exhibition during opera performance".to_string(),
                result: "1-0 (White wins)".to_string(),
                total_moves: moves.len(),
                program_id: "2cUpT4EQXT8D6dWQw6WGfxQm897CFKrvmwpjzCNm1Bix".to_string(),
                network: "Solana Devnet".to_string(),
            },
            moves,
            historical_context: HistoricalContext {
                significance: "One of the most brilliant chess games ever played".to_string(),
                background: "Paul Morphy played this game while simultaneously playing blindfold chess in another room at the Paris Opera House".to_string(),
                legacy: "This game is studied by chess players worldwide as a masterpiece of tactical brilliancy".to_string(),
                interesting_facts: vec![
                    "Morphy was playing this game while blindfolded against another opponent".to_string(),
                    "The game includes a famous queen sacrifice (Qb8+!!)".to_string(),
                    "The final position is a beautiful checkmate pattern".to_string(),
                    "This game helped establish Morphy's reputation as the best player of his time".to_string(),
                    "The Opera House audience was reportedly more interested in Morphy's chess than the opera".to_string(),
                ],
            },
            wager_info: WagerInfo {
                amount_per_player: 0.001,
                total_pool: 0.002,
                currency: "SOL".to_string(),
                winner: Player::White,
                payout: 0.002,
            },
        }
    }
    
    fn generate_opera_game_moves() -> Vec<OperaGameMove> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        
        vec![
            OperaGameMove {
                move_number: 1,
                player: Player::White,
                move_notation: "e2e4".to_string(),
                annotation: "King's Pawn Opening - Classical start".to_string(),
                fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Morphy's favorite opening, controlling the center".to_string(),
                tactical_analysis: "Opens lines for pieces, prepares development, fights for center control".to_string(),
            },
            OperaGameMove {
                move_number: 1,
                player: Player::Black,
                move_notation: "e7e5".to_string(),
                annotation: "Open Game - Symmetrical response".to_string(),
                fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Standard response, creates open position".to_string(),
                tactical_analysis: "Symmetrical response, leads to open game where tactics dominate".to_string(),
            },
            OperaGameMove {
                move_number: 2,
                player: Player::White,
                move_notation: "g1f3".to_string(),
                annotation: "Knight development - controls center".to_string(),
                fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Develops knight, prepares castling, controls center".to_string(),
                tactical_analysis: "Optimal knight development, eyes d4 and e5 squares, prepares castling".to_string(),
            },
            OperaGameMove {
                move_number: 2,
                player: Player::Black,
                move_notation: "d7d6".to_string(),
                annotation: "Philidor Defense - Solid but passive".to_string(),
                fen: "rnbqkb1r/ppp2ppp/3p1n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 1 3".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Philidor's defense, prepares e5 support".to_string(),
                tactical_analysis: "Solid but somewhat passive, supports e5, prepares development".to_string(),
            },
            OperaGameMove {
                move_number: 3,
                player: Player::White,
                move_notation: "d2d4".to_string(),
                annotation: "Central break - Challenges Black's setup".to_string(),
                fen: "rnbqkb1r/ppp2ppp/3p1n2/4p3/3P4/5N2/PPP2PPP/RNBQKB1R b KQkq - 0 3".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Creates tension in center, opens position".to_string(),
                tactical_analysis: "Challenges Black's central control, opens lines for pieces".to_string(),
            },
            OperaGameMove {
                move_number: 3,
                player: Player::Black,
                move_notation: "c8g4".to_string(),
                annotation: "Pins knight to queen - Developing with tempo".to_string(),
                fen: "rnbqk1r1/ppp2ppp/3p1n2/4p3/3P4/5N2/PPP2PPP/RNBQKB1R w KQkq - 1 3".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Active development with pin, gains tempo".to_string(),
                tactical_analysis: "Pins knight to queen, develops bishop, creates problems for White".to_string(),
            },
            OperaGameMove {
                move_number: 4,
                player: Player::White,
                move_notation: "d4e5".to_string(),
                annotation: "Captures center pawn - Opens position".to_string(),
                fen: "rnbqk1r1/ppp2ppp/3p1n2/4Pp3/3P4/5N2/PPP2PPP/RNBQKB1R b KQkq - 0 4".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Accepts challenge, opens center, creates complications".to_string(),
                tactical_analysis: "Captures with pawn, opens d-file, creates tactical opportunities".to_string(),
            },
            OperaGameMove {
                move_number: 4,
                player: Player::Black,
                move_notation: "g4f3".to_string(),
                annotation: "Captures knight - Damages White's structure".to_string(),
                fen: "rnbqk1r1/ppp2ppp/3p4/4Pp3/3P4/5N2/PPP2PPP/RNBQKB1R w KQkq - 0 4".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Removes knight, damages White's pawn structure".to_string(),
                tactical_analysis: "Captures knight, forces White to recapture with queen, damages pawn structure".to_string(),
            },
            OperaGameMove {
                move_number: 5,
                player: Player::White,
                move_notation: "d1f3".to_string(),
                annotation: "Queen recaptures - Centralized queen".to_string(),
                fen: "rnbqk1r1/ppp2ppp/3p4/4Pp3/3P4/3Q4/PPP2PPP/RNBQKB1R b KQkq - 1 4".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Centralized queen, prepares for attack".to_string(),
                tactical_analysis: "Queen centralized, eyes weak squares, prepares attacking ideas".to_string(),
            },
            OperaGameMove {
                move_number: 5,
                player: Player::Black,
                move_notation: "d6e5".to_string(),
                annotation: "Recaptures pawn - Opens d-file".to_string(),
                fen: "rnbqk1r1/ppp2ppp/3p4/4P3/3P4/3Q4/PPP2PPP/RNBQKB1R w KQkq - 0 5".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Recaptures, opens d-file for rook".to_string(),
                tactical_analysis: "Recaptures pawn, opens d-file, prepares to challenge White's queen".to_string(),
            },
            OperaGameMove {
                move_number: 6,
                player: Player::White,
                move_notation: "f1c4".to_string(),
                annotation: "Bishop to c4 - Targets f7 weakness".to_string(),
                fen: "rnbqk1r1/ppp2ppp/3p4/4P3/2BP4/3Q4/PPP2PPP/RNBQKB1R b KQkq - 1 5".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Italian Game setup, targets f7 weakness".to_string(),
                tactical_analysis: "Develops bishop to active square, eyes f7, prepares attacking ideas".to_string(),
            },
            OperaGameMove {
                move_number: 6,
                player: Player::Black,
                move_notation: "g8f6".to_string(),
                annotation: "Knight develops - Defends and attacks".to_string(),
                fen: "rnbqk2r/ppp2ppp/3p4/4P3/2BP4/3Q4/PPP2PPP/RNBQKB1R w KQkq - 2 5".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Develops knight, defends e5, prepares castling".to_string(),
                tactical_analysis: "Optimal knight development, defends e5, prepares castling, controls center".to_string(),
            },
            OperaGameMove {
                move_number: 7,
                player: Player::White,
                move_notation: "f3b3".to_string(),
                annotation: "Queen to b3 - Double attack on b7 and f7".to_string(),
                fen: "rnbqk2r/ppp2ppp/3p4/4P3/2BP4/1Q6/PPP2PPP/RNBQKB1R b KQkq - 3 5".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Creates double attack, forces Black's response".to_string(),
                tactical_analysis: "Double attack on b7 and f7, forces Black to defend multiple threats".to_string(),
            },
            OperaGameMove {
                move_number: 7,
                player: Player::Black,
                move_notation: "d8e7".to_string(),
                annotation: "Queen guards f7 and e-file".to_string(),
                fen: "rnbqk2r/ppp1qppp/3p4/4P3/2BP4/1Q6/PPP2PPP/RNBQKB1R w KQkq - 4 6".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Defends against double attack, prepares development".to_string(),
                tactical_analysis: "Defends f7 and e-file, prepares to connect rooks, solidifies position".to_string(),
            },
            OperaGameMove {
                move_number: 8,
                player: Player::White,
                move_notation: "b1c3".to_string(),
                annotation: "Knight to c3 - Completes development".to_string(),
                fen: "rnbqk2r/ppp1qppp/3p4/4P3/2BP4/1Q6/PPP2PPP/RNBQKB1R b KQkq - 4 6".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Completes development, prepares castling".to_string(),
                tactical_analysis: "Final piece development, prepares castling, controls center".to_string(),
            },
            OperaGameMove {
                move_number: 8,
                player: Player::Black,
                move_notation: "c7c6".to_string(),
                annotation: "Solidifies center - Prepares d5".to_string(),
                fen: "rnbqk2r/ppq1qppp/3p4/4P3/2BP4/1Q6/PPP2PPP/RNBQKB1R w KQkq - 5 7".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Solidifies center, prepares counterplay".to_string(),
                tactical_analysis: "Supports d5 push, solidifies center, prepares queenside play".to_string(),
            },
            OperaGameMove {
                move_number: 9,
                player: Player::White,
                move_notation: "c1g5".to_string(),
                annotation: "Bishop pins knight to queen - Increasing pressure".to_string(),
                fen: "rnbqk2r/ppq1qppp/3p4/4P3/2B1P3/1Q6/PPP2PPP/RNBQKB1R b KQkq - 5 7".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Increases pressure, creates tactical problems".to_string(),
                tactical_analysis: "Pins knight to queen, increases pressure, creates tactical complications".to_string(),
            },
            OperaGameMove {
                move_number: 9,
                player: Player::Black,
                move_notation: "b7b5".to_string(),
                annotation: "b5 thrust - Counterplay on queenside".to_string(),
                fen: "rnbqk2r/pp1qppp1/3p4/1p1P3/2B1P3/1Q6/PPP2PPP/RNBQKB1R w KQkq - 0 8".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Creates counterplay, challenges White's bishop".to_string(),
                tactical_analysis: "Creates counterplay, challenges White's bishop, opens lines".to_string(),
            },
            OperaGameMove {
                move_number: 10,
                player: Player::White,
                move_notation: "c3b5".to_string(),
                annotation: "Knight takes b5 - Tactical blow".to_string(),
                fen: "rnbqk2r/pp1qppp1/3p4/1p1P3/2B1P3/1QN5/PPP2PPP/RNBQKB1R b KQkq - 1 8".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Tactical blow, creates immediate threats".to_string(),
                tactical_analysis: "Tactical blow, creates immediate threats, forces Black's response".to_string(),
            },
            OperaGameMove {
                move_number: 10,
                player: Player::Black,
                move_notation: "c6b5".to_string(),
                annotation: "Recaptures knight - Opens c-file".to_string(),
                fen: "rnbqk2r/pp1qppp1/3p4/1p1P3/2B1P3/1QN5/PPP2PPP/RNBQKB1R w KQkq - 0 9".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Recaptures, opens c-file, maintains balance".to_string(),
                tactical_analysis: "Recaptures knight, opens c-file, maintains material balance".to_string(),
            },
            OperaGameMove {
                move_number: 11,
                player: Player::White,
                move_notation: "c4b5".to_string(),
                annotation: "Bishop takes b5 check! - Forcing sequence begins".to_string(),
                fen: "rnbqk2r/pp1qppp1/3p4/1p1P3/4P3/2QN5/PPP2PPP/RNBQKB1R b KQkq - 0 9".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Starts forcing sequence, creates tactical complications".to_string(),
                tactical_analysis: "Starts forcing sequence, creates tactical complications, forces Black's response".to_string(),
            },
            OperaGameMove {
                move_number: 11,
                player: Player::Black,
                move_notation: "b8d7".to_string(),
                annotation: "Knight blocks check - Only reasonable move".to_string(),
                fen: "rnb1k2r/pp1qppp1/3p4/1p1P3/4P3/2QN5/PPP2PPP/RNBQKB1R w KQkq - 1 10".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Forced move, only reasonable response to check".to_string(),
                tactical_analysis: "Forced move, only reasonable response to check, maintains king safety".to_string(),
            },
            OperaGameMove {
                move_number: 12,
                player: Player::White,
                move_notation: "e1c1".to_string(),
                annotation: "Queenside castling - Rook enters d-file with tempo".to_string(),
                fen: "rnb1k2r/pp1qppp1/3p4/1p1P3/4P3/2QN5/PPP2PPP/RNBQKB1R b KQkq - 1 10".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Queenside castling, rook enters d-file with tempo".to_string(),
                tactical_analysis: "Queenside castling, rook enters d-file with tempo, increases pressure".to_string(),
            },
            OperaGameMove {
                move_number: 12,
                player: Player::Black,
                move_notation: "a8d8".to_string(),
                annotation: "Rook to d8 - Defends against discovered attack".to_string(),
                fen: "r2b1k2r/pp1qppp1/3p4/1p1P3/4P3/2QN5/PPP2PPP/RNBQKB1R w KQkq - 2 11".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Defends against discovered attack, prepares counterplay".to_string(),
                tactical_analysis: "Defends against discovered attack, prepares counterplay, connects rooks".to_string(),
            },
            OperaGameMove {
                move_number: 13,
                player: Player::White,
                move_notation: "d1d7".to_string(),
                annotation: "Rook sacrifice! Rxd7 - Morphy's brilliance begins".to_string(),
                fen: "r2b1k2r/pp1qppp1/3p4/1p1P3/4P3/2QN5/PPP2PPP/RNBQKB1R b KQkq - 2 11".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Brilliant rook sacrifice, starts combination".to_string(),
                tactical_analysis: "Brilliant rook sacrifice, starts combination, forces Black's response".to_string(),
            },
            OperaGameMove {
                move_number: 13,
                player: Player::Black,
                move_notation: "d8d7".to_string(),
                annotation: "Forced recapture - Removes the rook".to_string(),
                fen: "r2b1k2r/pp1qppp1/3p4/1p1P3/4P3/2Q5/PPP2PPP/RNBQKB1R w KQkq - 0 12".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Forced recapture, removes the rook".to_string(),
                tactical_analysis: "Forced recapture, removes the rook, maintains material balance".to_string(),
            },
            OperaGameMove {
                move_number: 14,
                player: Player::White,
                move_notation: "h1d1".to_string(),
                annotation: "Rook to d1 - Pins the defender to the king".to_string(),
                fen: "r2b1k2r/pp1qppp1/3p4/1p1P3/4P3/2Q5/PPP2PPPP/RNBQKB1R b KQkq - 0 12".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Pins defender to king, increases pressure".to_string(),
                tactical_analysis: "Pins defender to king, increases pressure, creates tactical problems".to_string(),
            },
            OperaGameMove {
                move_number: 14,
                player: Player::Black,
                move_notation: "e7e6".to_string(),
                annotation: "Queen to e6 - Desperate attempt to block".to_string(),
                fen: "r2b1k2r/pp1qpp1/3p4/1p1P3/4P3/2Q5/PPP2PPPP/RNBQKB1R w KQkq - 1 13".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Desperate attempt to block, prepares defense".to_string(),
                tactical_analysis: "Desperate attempt to block, prepares defense, tries to survive".to_string(),
            },
            OperaGameMove {
                move_number: 15,
                player: Player::White,
                move_notation: "b5d7".to_string(),
                annotation: "Bishop takes d7 check! - Removes last defender".to_string(),
                fen: "r2b1k2r/pp1qpp1/3p4/1p1P3/4P3/8/PPP2PPPP/RNBQKB1R b KQkq - 0 13".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Removes last defender, prepares final attack".to_string(),
                tactical_analysis: "Removes last defender, prepares final attack, forces Black's response".to_string(),
            },
            OperaGameMove {
                move_number: 15,
                player: Player::Black,
                move_notation: "f6d7".to_string(),
                annotation: "Knight recaptures - Forced".to_string(),
                fen: "r2b1k2r/pp1qpp1/3p4/1p1P3/4P3/8/PPP2PPPP/RNBQKB1R w KQkq - 0 14".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Forced recapture, tries to survive".to_string(),
                tactical_analysis: "Forced recapture, tries to survive, maintains material balance".to_string(),
            },
            OperaGameMove {
                move_number: 16,
                player: Player::White,
                move_notation: "b3b8".to_string(),
                annotation: "Queen sacrifice! Qb8+!! - The immortal offer".to_string(),
                fen: "r2k1b1r/pp1qpp1/3p4/1p1P3/4P3/8/PPP2PPPP/RNBQKB1R b KQkq - 1 14".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Immortal queen sacrifice, brilliancy at its finest".to_string(),
                tactical_analysis: "Immortal queen sacrifice, brilliancy at its finest, forces checkmate".to_string(),
            },
            OperaGameMove {
                move_number: 16,
                player: Player::Black,
                move_notation: "d7b8".to_string(),
                annotation: "Knight forced to take queen".to_string(),
                fen: "r2k1b1r/pp1qpp1/3p4/1p1P3/4P3/8/PPP2PPPP/RNBQKB1R w KQkq - 0 15".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Forced to take queen, sealing fate".to_string(),
                tactical_analysis: "Forced to take queen, sealing fate, leads to checkmate".to_string(),
            },
            OperaGameMove {
                move_number: 17,
                player: Player::White,
                move_notation: "d1d8".to_string(),
                annotation: "ROOK TO D8# - CHECKMATE!! The Opera Game concludes!".to_string(),
                fen: "r2k1b1r/pp1qpp1/3p4/1p1P3/4P3/8/PPP2PPPP/RNBQK2R b KQkq - 0 15".to_string(),
                timestamp: timestamp.clone(),
                historical_significance: "Brilliant checkmate, Opera Game concludes".to_string(),
                tactical_analysis: "Brilliant checkmate, Opera Game concludes, chess immortality".to_string(),
            },
        ]
    }
    
    pub fn generate_explorer_links(&self) -> Vec<String> {
        let mut links = Vec::new();
        
        // Game creation link
        links.push("https://explorer.solana.com/tx/CREATE_GAME_TX?cluster=devnet".to_string());
        
        // Black joins link
        links.push("https://explorer.solana.com/tx/JOIN_GAME_TX?cluster=devnet".to_string());
        
        // Move links
        for i in 1..=self.moves.len() {
            links.push(format!("https://explorer.solana.com/tx/MOVE_TX_{}?cluster=devnet", i));
        }
        
        // Finalization link
        links.push("https://explorer.solana.com/tx/FINALIZE_GAME_TX?cluster=devnet".to_string());
        
        links
    }
    
    pub fn get_move_annotation(&self, move_number: usize) -> Option<&OperaGameMove> {
        self.moves.iter().find(|m| m.move_number == move_number)
    }
    
    pub fn get_player_moves(&self, player: Player) -> Vec<&OperaGameMove> {
        self.moves.iter().filter(|m| m.player == player).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_opera_game_metadata_creation() {
        let metadata = OperaGameMetadata::new();
        assert_eq!(metadata.moves.len(), 33);
        assert_eq!(metadata.game_info.title, "The Opera Game");
        assert_eq!(metadata.game_info.total_moves, 33);
    }
    
    #[test]
    fn test_player_names() {
        assert_eq!(Player::White.name(), "Paul Morphy");
        assert_eq!(Player::Black.name(), "Duke of Brunswick & Count Isouard");
    }
    
    #[test]
    fn test_explorer_links_generation() {
        let metadata = OperaGameMetadata::new();
        let links = metadata.generate_explorer_links();
        assert_eq!(links.len(), 36); // 1 create + 1 join + 33 moves + 1 finalization
    }
}
