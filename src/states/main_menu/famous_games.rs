//! Famous historical games replayed on the main-menu ambient board.
//!
//! Pure data — no playback logic here (that lives in `board_animation.rs`).
//! Ordered chronologically; reorder freely, it's just an array.

pub(super) struct FamousGame {
    pub caption: &'static str,
    pub pgn: &'static str,
}

pub(super) static FAMOUS_GAMES: &[FamousGame] = &[
    FamousGame {
        caption: "Anderssen vs. Kieseritzky, London 1851 — The Immortal Game",
        pgn: ANDERSSEN_KIESERITZKY,
    },
    FamousGame {
        caption: "Sämisch vs. Nimzowitsch, Copenhagen 1923",
        pgn: ZUGZWANG_PGN,
    },
    FamousGame {
        caption: "Menchik vs. Euwe, Hastings 1931/32",
        pgn: MENCHIK_EUWE,
    },
    FamousGame {
        caption: "Byrne vs. Fischer, New York 1956 — The Game of the Century",
        pgn: BYRNE_FISCHER,
    },
    FamousGame {
        caption: "Polgár vs. Kasparov, Moscow 2002",
        pgn: POLGAR_KASPAROV,
    },
    FamousGame {
        caption: "Hou Yifan vs. Caruana, Karlsruhe 2017",
        pgn: HOU_CARUANA,
    },
];

// Anderssen vs. Kieseritzky, London 1851 — King's Gambit Accepted, "The Immortal Game".
const ANDERSSEN_KIESERITZKY: &str = "
1. e4 e5 2. f4 exf4 3. Bc4 Qh4+ 4. Kf1 b5 5. Bxb5 Nf6 6. Nf3 Qh6 7. d3 Nh5
8. Nh4 Qg5 9. Nf5 c6 10. g4 Nf6 11. Rg1 cxb5 12. h4 Qg6 13. h5 Qg5 14. Qf3 Ng8
15. Bxf4 Qf6 16. Nc3 Bc5 17. Nd5 Qxb2 18. Bd6 Bxg1 19. e5 Qxa1+ 20. Ke2 Na6
21. Nxg7+ Kd8 22. Qf6+ Nxf6 23. Be7#
1-0
";

// Sämisch vs Nimzowitsch, Copenhagen 1923 — the Immortal Zugzwang Game.
const ZUGZWANG_PGN: &str = "
1. d4 Nf6 2. c4 e6 3. Nf3 b6 4. g3 Bb7 5. Bg2 Be7
6. Nc3 O-O 7. O-O d5 8. Ne5 c6
9. cxd5 cxd5 10. Bf4 a6
11. Rc1 b5 12. Qb3 Nc6
13. Nxc6 Bxc6 14. h3 Qd7 15. Kh2 Nh5
16. Bd2 f5 17. Qd1 b4 18. Nb1 Bb5 19. Rg1 Bd6 20. e4 fxe4
21. Qxh5 Rxf2 22. Qg5 Raf8 23. Kh1 R8f5 24. Qe3 Bd3 25. Rce1 h6
0-1
";

// Menchik vs. Euwe, Hastings 1931/32 — Slav Defense.
const MENCHIK_EUWE: &str = "
1. d4 d5 2. c4 c6 3. Nf3 Nf6 4. Nc3 dxc4 5. a4 Bf5 6. e3 Na6 7. Bxc4 Nb4
8. O-O e6 9. Ne5 Bd6 10. Qe2 c5 11. Bb5+ Ke7 12. e4 Bg6 13. Nxg6+ hxg6
14. e5 cxd4 15. Rd1 Bc7 16. exf6+ gxf6 17. g3 a6 18. Be3 Bb6 19. Bc4 Kf8
20. Ne4 Kg7 21. Rac1 Rh5 22. Bf4 e5 23. g4 Rh8 24. Bg3 Qe7 25. Nd2 Rhe8
26. Qe4 Qd7 27. Nf3 Qc6 28. Qxc6 Nxc6 29. Bd5 Rac8 30. Be4 Rc7 31. Ne1 Rec8
32. Nd3 Ne7 33. Rxc7 Rxc7 34. Kf1 Rc4 35. Bxb7 Rxa4 36. Rc1 g5 37. f3 Ra2
38. Be1 a5 39. Bd2 f5 40. gxf5 a4 41. Ke1 a3 42. b4 Kf6 43. Ba6 g4 44. Bc4 Rxd2
45. Kxd2 gxf3 46. Nc5 Kxf5 47. Bxf7 Bd8 48. Be6+ Kf6 49. Bg4 Nd5 50. Bxf3 Nxb4
51. Be4 Be7 52. Nd3 Na2 53. Rc6+ Kg5 54. Rg6+ Kh4 55. Nxe5 Nc3 56. Kd3
1-0
";

// Byrne vs. Fischer, New York 1956 — Grünfeld Defense, "The Game of the Century".
const BYRNE_FISCHER: &str = "
1. Nf3 Nf6 2. c4 g6 3. Nc3 Bg7 4. d4 O-O 5. Bf4 d5 6. Qb3 dxc4 7. Qxc4 c6
8. e4 Nbd7 9. Rd1 Nb6 10. Qc5 Bg4 11. Bg5 Na4 12. Qa3 Nxc3 13. bxc3 Nxe4
14. Bxe7 Qb6 15. Bc4 Nxc3 16. Bc5 Rfe8+ 17. Kf1 Be6 18. Bxb6 Bxc4+ 19. Kg1 Ne2+
20. Kf1 Nxd4+ 21. Kg1 Ne2+ 22. Kf1 Nc3+ 23. Kg1 axb6 24. Qb4 Ra4 25. Qxb6 Nxd1
26. h3 Rxa2 27. Kh2 Nxf2 28. Re1 Rxe1 29. Qd8+ Bf8 30. Nxe1 Bd5 31. Nf3 Ne4
32. Qb8 b5 33. h4 h5 34. Ne5 Kg7 35. Kg1 Bc5+ 36. Kf1 Ng3+ 37. Ke1 Bb4+
38. Kd1 Bb3+ 39. Kc1 Ne2+ 40. Kb1 Nc3+ 41. Kc1 Rc2#
0-1
";

// Polgár vs. Kasparov, Moscow 2002 — Ruy Lopez, Berlin Defense (Russia vs. Rest of the World).
const POLGAR_KASPAROV: &str = "
1. e4 e5 2. Nf3 Nc6 3. Bb5 Nf6 4. O-O Nxe4 5. d4 Nd6 6. Bxc6 dxc6 7. dxe5 Nf5
8. Qxd8+ Kxd8 9. Nc3 h6 10. Rd1+ Ke8 11. h3 Be7 12. Ne2 Nh4 13. Nxh4 Bxh4
14. Be3 Bf5 15. Nd4 Bh7 16. g4 Be7 17. Kg2 h5 18. Nf5 Bf8 19. Kf3 Bg6
20. Rd2 hxg4+ 21. hxg4 Rh3+ 22. Kg2 Rh7 23. Kg3 f6 24. Bf4 Bxf5 25. gxf5 fxe5
26. Re1 Bd6 27. Bxe5 Kd7 28. c4 c5 29. Bxd6 cxd6 30. Re6 Rah8 31. Rexd6+ Kc8
32. R2d5 Rh3+ 33. Kg2 Rh2+ 34. Kf3 R2h3+ 35. Ke4 b6 36. Rc6+ Kb8 37. Rd7 Rh2
38. Ke3 Rf8 39. Rcc7 Rxf5 40. Rb7+ Kc8 41. Rdc7+ Kd8 42. Rxg7 Kc8
1-0
";

// Hou Yifan vs. Caruana, Karlsruhe 2017 (GRENKE Chess Classic) — Ruy Lopez, Berlin Defense,
// Rio Gambit Accepted.
const HOU_CARUANA: &str = "
1. e4 e5 2. Nf3 Nc6 3. Bb5 Nf6 4. O-O Nxe4 5. Re1 Nd6 6. Nxe5 Be7 7. Bf1 O-O
8. d4 Nf5 9. Nf3 d5 10. c3 Bd6 11. Nbd2 Nce7 12. Qc2 c6 13. Bd3 g6 14. Nf1 f6
15. h3 Rf7 16. Bd2 Bd7 17. Re2 c5 18. dxc5 Bxc5 19. Bf4 Rc8 20. Rae1 g5
21. Ng3 Nxg3 22. Bxg3 a5 23. Qd2 a4 24. b4 axb3 25. axb3 Ng6 26. h4 gxh4
27. Nxh4 Nxh4 28. Bxh4 Qf8 29. Qf4 Bd6 30. Qd4 Rd8 31. Re3 Bc8 32. b4 Kg7
33. Bb5 Bc7 34. Re8 Qd6 35. Bg3 Qb6 36. Qd3 Bd7 37. Bxd7 Rdxd7 38. Qf5 Bxg3
39. Qg4+ Kh6 40. Qh3+
1-0
";
