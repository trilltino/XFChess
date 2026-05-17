//! Named time control presets for chess games.
//!
//! Covers the full range from ultra-bullet to correspondence,
//! plus a custom variant for arbitrary base + increment values.
//!
//! Reference: https://www.lichess.org/variant/timeControl

/// A chess time control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeControl {
    /// 15 + 0 (ultra-fast bullets)
    UltraBullet,
    /// 60 + 0
    Bullet,
    /// 60 + 1
    BulletIncrement,
    /// 180 + 0
    BlitzThree,
    /// 180 + 2
    BlitzThreePlus,
    /// 300 + 0
    #[default]
    Blitz,
    /// 300 + 3
    BlitzPlus,
    /// 600 + 0
    Rapid,
    /// 900 + 10
    RapidPlus,
    /// 1800 + 0
    Classical,
    /// 0 — no clock (correspondence / AI casual)
    Unlimited,
    /// Arbitrary base + increment (seconds)
    Custom {
        base_seconds: u32,
        increment_seconds: u16,
    },
}

impl TimeControl {
    /// Total base time per player in seconds (0 = no clock).
    pub fn base_seconds(self) -> u32 {
        match self {
            TimeControl::UltraBullet => 15,
            TimeControl::Bullet | TimeControl::BulletIncrement => 60,
            TimeControl::BlitzThree | TimeControl::BlitzThreePlus => 180,
            TimeControl::Blitz | TimeControl::BlitzPlus => 300,
            TimeControl::Rapid => 600,
            TimeControl::RapidPlus => 900,
            TimeControl::Classical => 1800,
            TimeControl::Unlimited => 0,
            TimeControl::Custom { base_seconds, .. } => base_seconds,
        }
    }

    /// Fischer increment added after each move in seconds.
    pub fn increment_seconds(self) -> u16 {
        match self {
            TimeControl::BulletIncrement => 1,
            TimeControl::BlitzThreePlus => 2,
            TimeControl::BlitzPlus => 3,
            TimeControl::RapidPlus => 10,
            TimeControl::Custom { increment_seconds, .. } => increment_seconds,
            _ => 0,
        }
    }

    /// Short display label, e.g. "5+0" or "3+2".
    pub fn short_label(self) -> String {
        let base_min = self.base_seconds() / 60;
        let base_sec = self.base_seconds() % 60;
        let inc = self.increment_seconds();
        if base_min > 0 && base_sec == 0 {
            format!("{}+{}", base_min, inc)
        } else if base_min == 0 {
            format!("{}s+{}", base_sec, inc)
        } else {
            format!("{}:{:02}+{}", base_min, base_sec, inc)
        }
    }

    /// Human-readable category + label, e.g. " Blitz 5+0".
    pub fn display_name(self) -> String {
        let label = self.short_label();
        match self.category() {
            TimeCategory::UltraBullet => format!(" UltraBullet {}", label),
            TimeCategory::Bullet => format!(" Bullet {}", label),
            TimeCategory::Blitz => format!(" Blitz {}", label),
            TimeCategory::Rapid => format!(" Rapid {}", label),
            TimeCategory::Classical => format!(" Classical {}", label),
            TimeCategory::Unlimited => "8 Unlimited".to_string(),
        }
    }

    /// Category for grouping in the UI.
    pub fn category(self) -> TimeCategory {
        match self.base_seconds() {
            0 => TimeCategory::Unlimited,
            1..=59 => TimeCategory::UltraBullet,
            60..=179 => TimeCategory::Bullet,
            180..=599 => TimeCategory::Blitz,
            600..=1499 => TimeCategory::Rapid,
            _ => TimeCategory::Classical,
        }
    }

    /// All presets in display order.
    pub fn presets() -> &'static [TimeControl] {
        &[
            TimeControl::UltraBullet,
            TimeControl::Bullet,
            TimeControl::BulletIncrement,
            TimeControl::BlitzThree,
            TimeControl::BlitzThreePlus,
            TimeControl::Blitz,
            TimeControl::BlitzPlus,
            TimeControl::Rapid,
            TimeControl::RapidPlus,
            TimeControl::Classical,
            TimeControl::Unlimited,
        ]
    }
}

/// Broad category for a time control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeCategory {
    UltraBullet,
    Bullet,
    Blitz,
    Rapid,
    Classical,
    Unlimited,
}

impl TimeCategory {
    pub fn label(self) -> &'static str {
        match self {
            TimeCategory::UltraBullet => " UltraBullet",
            TimeCategory::Bullet => " Bullet",
            TimeCategory::Blitz => " Blitz",
            TimeCategory::Rapid => " Rapid",
            TimeCategory::Classical => " Classical",
            TimeCategory::Unlimited => "8 Unlimited",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blitz_base_and_increment() {
        assert_eq!(TimeControl::Blitz.base_seconds(), 300);
        assert_eq!(TimeControl::Blitz.increment_seconds(), 0);
        assert_eq!(TimeControl::BlitzPlus.increment_seconds(), 3);
    }

    #[test]
    fn short_label_formats() {
        assert_eq!(TimeControl::Blitz.short_label(), "5+0");
        assert_eq!(TimeControl::BlitzPlus.short_label(), "5+3");
        assert_eq!(TimeControl::UltraBullet.short_label(), "15s+0");
        assert_eq!(TimeControl::Unlimited.short_label(), "0+0");
    }

    #[test]
    fn custom_preset() {
        let tc = TimeControl::Custom { base_seconds: 120, increment_seconds: 5 };
        assert_eq!(tc.base_seconds(), 120);
        assert_eq!(tc.increment_seconds(), 5);
        assert_eq!(tc.short_label(), "2+5");
    }

    #[test]
    fn categories_are_correct() {
        assert_eq!(TimeControl::UltraBullet.category(), TimeCategory::UltraBullet);
        assert_eq!(TimeControl::Blitz.category(), TimeCategory::Blitz);
        assert_eq!(TimeControl::Classical.category(), TimeCategory::Classical);
        assert_eq!(TimeControl::Unlimited.category(), TimeCategory::Unlimited);
    }
}

