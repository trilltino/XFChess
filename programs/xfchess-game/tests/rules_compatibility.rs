//! Host rules may change; contract move bytes and validation must remain compatible.
use anyhow::Context as _;
use nimzovich_engine::{validate_and_apply, CompactBoard};
use xfchess_rules::Game;

const POSITIONS: &[&str] = &[
    xfchess_rules::STARTING_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
    "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
    "7k/P7/8/8/8/8/8/7K w - - 0 1",
    "7k/8/8/8/8/8/p7/7K b - - 0 1",
    "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3",
];

fn apply_contract(fen: &str, uci: &str) -> Result<[u8; 68], String> {
    let mut position = CompactBoard::from_fen(fen).to_on_chain_game();
    let mut bytes = [b' '; 5];
    bytes[..uci.len()].copy_from_slice(uci.as_bytes());
    validate_and_apply(&mut position, &bytes).map_err(|e| format!("{e:?}"))?;
    Ok(position.to_compact_board().to_bytes())
}

fn compare(game: &Game) -> anyhow::Result<()> {
    let fen = game.fen();
    assert_eq!(
        game.compact_board().to_bytes(),
        CompactBoard::from_fen(&fen).to_bytes(),
        "{fen}"
    );
    assert_eq!(
        game.compact_board().to_fen().map_err(anyhow::Error::msg)?,
        CompactBoard::from_fen(&fen).to_fen()
    );
    for uci in game.legal_moves() {
        let mut next = game.clone();
        next.play_uci(&uci)
            .map_err(|error| anyhow::anyhow!("{error:?}"))
            .context("required operation value")?;
        assert_eq!(
            apply_contract(&fen, &uci)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("Contract rejects {uci} at {fen}"))?,
            next.compact_board().to_bytes(),
            "{fen} {uci}"
        );
    }
    // Compare ordinary illegal transitions as well as all legal promotions.
    for from in 0..64 {
        for to in 0..64 {
            let uci = xfchess_rules::move_uci(from, to, 0)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("required operation value")?;
            let promotion = game.board[from as usize].abs() == 1 && (to / 8 == 0 || to / 8 == 7);
            if !promotion {
                assert_eq!(
                    apply_contract(&fen, &uci).is_ok(),
                    game.is_legal_uci(&uci),
                    "{fen} {uci}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn contract_and_host_agree_on_special_positions() -> anyhow::Result<()> {
    for fen in POSITIONS {
        compare(
            &Game::from_fen(fen)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("required operation value")?,
        )?;
    }
    Ok(())
}

#[test]
fn contract_and_host_agree_on_generated_games() -> anyhow::Result<()> {
    let mut seed = 42u64;
    for _ in 0..4 {
        let mut game = Game::default();
        for ply in 0..100 {
            let legal = game.legal_moves();
            if legal.is_empty() {
                break;
            }
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            if ply % 20 == 0 {
                compare(&game)?;
            }
            let uci = &legal[seed as usize % legal.len()];
            let bytes = apply_contract(&game.fen(), uci)
                .map_err(anyhow::Error::msg)
                .context("required operation value")?;
            game.play_uci(uci)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("required operation value")?;
            assert_eq!(bytes, game.compact_board().to_bytes());
        }
    }
    Ok(())
}

#[test]
fn contract_and_host_agree_on_insufficient_material() -> anyhow::Result<()> {
    use nimzovich_engine::on_chain_moves::is_insufficient_material;
    for fen in [
        "8/8/4k3/8/8/4K3/8/8 w - - 0 1",
        "8/8/4k3/8/8/4K3/8/5N2 w - - 0 1",
        "8/8/4k3/8/8/4K3/8/6n1 b - - 0 1",
        "5b2/8/4k3/8/8/4K3/8/2B5 w - - 0 1",
        "2b5/8/4k3/8/8/4K3/8/2B5 w - - 0 1",
        "8/8/4k3/8/8/4K3/8/1N3N2 w - - 0 1",
        "8/8/4k3/8/8/4K3/8/2B2B2 w - - 0 1",
        "8/8/4k3/8/8/4K3/8/1N3B2 w - - 0 1",
        "8/8/4k3/8/8/4K3/4P3/8 w - - 0 1",
        "8/8/4k3/8/8/4K3/8/R7 w - - 0 1",
        xfchess_rules::STARTING_FEN,
    ] {
        let contract = is_insufficient_material(&CompactBoard::from_fen(fen).to_on_chain_game());
        assert_eq!(
            Game::from_fen(fen)
                .map_err(|error| anyhow::anyhow!("{error:?}"))
                .context("required operation value")?
                .is_insufficient_material(),
            contract,
            "{fen}"
        );
    }
    Ok(())
}
