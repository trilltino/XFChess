//! S+ñmisch vs Nimzowitsch, Copenhagen 1923 ÔÇö "Immortal Zugzwang Game".
//!
//! Coordinates: `a1 = (0, 0)`, files 0..7 = a..h, ranks 0..7 = 1..8.

use super::super::sequence::{MoveKind, MoveStep};

pub const IMMORTAL_ZUGZWANG: &[MoveStep] = &[
    // 1. d4 Nf6
    MoveStep {
        from: (3, 1),
        to: (3, 3),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (6, 7),
        to: (5, 5),
        kind: MoveKind::Normal,
    },
    // 2. c4 e6
    MoveStep {
        from: (2, 1),
        to: (2, 3),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (4, 6),
        to: (4, 5),
        kind: MoveKind::Normal,
    },
    // 3. Nf3 b6
    MoveStep {
        from: (6, 0),
        to: (5, 2),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (1, 6),
        to: (1, 5),
        kind: MoveKind::Normal,
    },
    // 4. g3 Bb7
    MoveStep {
        from: (6, 1),
        to: (6, 2),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (2, 7),
        to: (1, 6),
        kind: MoveKind::Normal,
    },
    // 5. Bg2 Be7
    MoveStep {
        from: (5, 0),
        to: (6, 1),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (5, 7),
        to: (4, 6),
        kind: MoveKind::Normal,
    },
    // 6. Nc3 O-O
    MoveStep {
        from: (1, 0),
        to: (2, 2),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (4, 7),
        to: (6, 7),
        kind: MoveKind::CastleKingside,
    },
    // 7. O-O d5
    MoveStep {
        from: (4, 0),
        to: (6, 0),
        kind: MoveKind::CastleKingside,
    },
    MoveStep {
        from: (3, 6),
        to: (3, 4),
        kind: MoveKind::Normal,
    },
    // 8. Ne5 c6
    MoveStep {
        from: (5, 2),
        to: (4, 4),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (2, 6),
        to: (2, 5),
        kind: MoveKind::Normal,
    },
    // 9. cxd5 cxd5
    MoveStep {
        from: (2, 3),
        to: (3, 4),
        kind: MoveKind::Capture,
    },
    MoveStep {
        from: (2, 5),
        to: (3, 4),
        kind: MoveKind::Capture,
    },
    // 10. Bf4 a6
    MoveStep {
        from: (2, 0),
        to: (5, 3),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (0, 6),
        to: (0, 5),
        kind: MoveKind::Normal,
    },
    // 11. Rc1 b5
    MoveStep {
        from: (0, 0),
        to: (2, 0),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (1, 5),
        to: (1, 4),
        kind: MoveKind::Normal,
    },
    // 12. Qb3 Nc6
    MoveStep {
        from: (3, 0),
        to: (1, 2),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (1, 7),
        to: (2, 5),
        kind: MoveKind::Normal,
    },
    // 13. Nxc6 Bxc6
    MoveStep {
        from: (4, 4),
        to: (2, 5),
        kind: MoveKind::Capture,
    },
    MoveStep {
        from: (1, 6),
        to: (2, 5),
        kind: MoveKind::Capture,
    },
    // 14. h3 Qd7
    MoveStep {
        from: (7, 1),
        to: (7, 2),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (3, 7),
        to: (3, 6),
        kind: MoveKind::Normal,
    },
    // 15. Kh2 Nh5
    MoveStep {
        from: (6, 0),
        to: (7, 1),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (5, 5),
        to: (7, 4),
        kind: MoveKind::Normal,
    },
    // 16. Bd2 f5
    MoveStep {
        from: (5, 3),
        to: (3, 1),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (5, 6),
        to: (5, 4),
        kind: MoveKind::Normal,
    },
    // 17. Qd1 b4
    MoveStep {
        from: (1, 2),
        to: (3, 0),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (1, 4),
        to: (1, 3),
        kind: MoveKind::Normal,
    },
    // 18. Nb1 Bb5
    MoveStep {
        from: (2, 2),
        to: (1, 0),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (2, 5),
        to: (1, 4),
        kind: MoveKind::Normal,
    },
    // 19. Rg1 Bd6
    MoveStep {
        from: (5, 0),
        to: (6, 0),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (4, 6),
        to: (3, 5),
        kind: MoveKind::Normal,
    },
    // 20. e4 fxe4
    MoveStep {
        from: (4, 1),
        to: (4, 3),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (5, 4),
        to: (4, 3),
        kind: MoveKind::Capture,
    },
    // 21. Qxh5 Rxf2
    MoveStep {
        from: (3, 0),
        to: (7, 4),
        kind: MoveKind::Capture,
    },
    MoveStep {
        from: (5, 7),
        to: (5, 1),
        kind: MoveKind::Capture,
    },
    // 22. Qg5 Raf8
    MoveStep {
        from: (7, 4),
        to: (6, 4),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (0, 7),
        to: (5, 7),
        kind: MoveKind::Normal,
    },
    // 23. Kh1 R8f5
    MoveStep {
        from: (7, 1),
        to: (7, 0),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (5, 7),
        to: (5, 4),
        kind: MoveKind::Normal,
    },
    // 24. Qe3 Bd3
    MoveStep {
        from: (6, 4),
        to: (4, 2),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (1, 4),
        to: (3, 2),
        kind: MoveKind::Normal,
    },
    // 25. Rce1 h6!  (zugzwang ÔÇö White resigned)
    MoveStep {
        from: (2, 0),
        to: (4, 0),
        kind: MoveKind::Normal,
    },
    MoveStep {
        from: (7, 6),
        to: (7, 5),
        kind: MoveKind::Normal,
    },
];
