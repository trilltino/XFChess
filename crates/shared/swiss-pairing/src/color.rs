use crate::{Color, SwissPlayer};

/// Calculate color balance for a player
/// Returns: positive = needs white, negative = needs black, 0 = balanced
pub fn calculate_balance(history: &[Color]) -> i8 {
    let whites = history.iter().filter(|c| **c == Color::White).count() as i8;
    let blacks = history.iter().filter(|c| **c == Color::Black).count() as i8;
    blacks - whites
}

/// Check if a player had the same color twice in a row
pub fn had_same_color_twice(history: &[Color]) -> bool {
    if history.len() < 2 {
        return false;
    }
    let len = history.len();
    history[len - 1] == history[len - 2]
}

/// Check if assigning a color would create three in a row
pub fn would_violate_three_in_row(history: &[Color], color: Color) -> bool {
    if history.len() < 2 {
        return false;
    }
    let len = history.len();
    history[len - 1] == color && history[len - 2] == color
}

/// Determine if colors should be swapped based on balance and history
pub fn should_swap_colors(
    white_balance: i8,
    black_balance: i8,
    white_history: &[Color],
    black_history: &[Color],
) -> bool {
    // If one player needs white significantly more
    if white_balance > black_balance + 1 {
        return false; // Keep current assignment
    }
    if black_balance > white_balance + 1 {
        return true; // Swap to give black player white
    }

    // Check for 3-in-a-row violations
    let white_would_violate = would_violate_three_in_row(white_history, Color::White);
    let black_would_violate = would_violate_three_in_row(black_history, Color::White);

    if white_would_violate && !black_would_violate {
        return true; // Swap to avoid violation
    }

    // Check for double-same-color (prefer alternating)
    let white_had_double = had_same_color_twice(white_history);
    let black_had_double = had_same_color_twice(black_history);

    if white_had_double && !black_had_double {
        return true;
    }

    false
}

/// Validate that a color assignment doesn't violate tournament rules
pub fn validate_color_assignment(players: &[SwissPlayer]) -> bool {
    for player in players {
        // Check no 3 in a row
        let history = &player.color_history;
        if history.len() >= 3 {
            for window in history.windows(3) {
                if window[0] == window[1] && window[1] == window[2] {
                    return false; // Three in a row detected
                }
            }
        }

        // Check overall balance (should be within 1)
        let balance = calculate_balance(history).abs();
        if balance > 1 {
            return false;
        }
    }

    true
}

/// Get preferred color for a player (the one they need more)
pub fn preferred_color(player: &SwissPlayer) -> Option<Color> {
    let balance = calculate_balance(&player.color_history);

    if balance > 0 {
        Some(Color::White) // Needs white
    } else if balance < 0 {
        Some(Color::Black) // Needs black
    } else {
        None // Balanced - no preference
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_balance() {
        let history = vec![Color::White, Color::Black, Color::White];
        assert_eq!(calculate_balance(&history), -1); // Needs black

        let history = vec![Color::Black, Color::Black, Color::White];
        assert_eq!(calculate_balance(&history), 1); // Needs white

        let history = vec![Color::White, Color::Black];
        assert_eq!(calculate_balance(&history), 0); // Balanced
    }

    #[test]
    fn test_would_violate_three_in_row() {
        let history = vec![Color::White, Color::White];
        assert!(would_violate_three_in_row(&history, Color::White));
        assert!(!would_violate_three_in_row(&history, Color::Black));

        let history = vec![Color::White, Color::Black];
        assert!(!would_violate_three_in_row(&history, Color::White));
    }

    #[test]
    fn test_should_swap_colors() {
        // White needs white more
        assert!(!should_swap_colors(2, 0, &[], &[]));

        // Black needs white more
        assert!(should_swap_colors(0, 2, &[], &[]));

        // White would violate
        let white_hist = vec![Color::White, Color::White];
        assert!(should_swap_colors(0, 0, &white_hist, &[]));
    }

    #[test]
    fn test_validate_color_assignment() {
        // Valid assignment
        let players = vec![SwissPlayer {
            id: "p1".to_string(),
            rating: 2000,
            score: 0.0,
            color_history: vec![Color::White, Color::Black, Color::White],
            opponents: vec![],
            bye_rounds: Vec::new(),
            float_history: Vec::new(),
            absent: false,
            withdrawn: false,
            forfeit_round: None,
        }];
        assert!(validate_color_assignment(&players));

        // Invalid - three in a row
        let players = vec![SwissPlayer {
            id: "p2".to_string(),
            rating: 2000,
            score: 0.0,
            color_history: vec![Color::White, Color::White, Color::White],
            opponents: vec![],
            bye_rounds: Vec::new(),
            float_history: Vec::new(),
            absent: false,
            withdrawn: false,
            forfeit_round: None,
        }];
        assert!(!validate_color_assignment(&players));
    }
}
