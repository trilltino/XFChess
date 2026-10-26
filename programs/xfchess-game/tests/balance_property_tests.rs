//! P2: seeded property tests for the pure money arithmetic. Edge values
//! (0, 1, rent floor, u64::MAX, 10,000-bps boundaries) are always included;
//! the rest comes from a deterministic generator. Reproduce a failure with
//!     BALANCE_PROP_SEED=<n> cargo test -p xfchess-game --test balance_property_tests
use xfchess_game::account_ix::session_guards::checked_session_total;
use xfchess_game::common::escrow::pot;
use xfchess_game::state::GlobalSessionDelegation;
use xfchess_game::tournament_ix::prizes::ledger::{place_bit, prize_amount, MAX_PRIZE_PLACES};

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn seed() -> u64 {
    std::env::var("BALANCE_PROP_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0xBA1A_0CE5)
}

const RENT_FLOOR: u64 = 890_880;

fn pools(rng: &mut Rng) -> Vec<u64> {
    let mut pools = vec![
        0,
        1,
        9_999,
        10_000,
        10_001,
        RENT_FLOOR,
        RENT_FLOOR + 1,
        u64::MAX / 2,
        u64::MAX - 1,
        u64::MAX,
    ];
    for _ in 0..2_000 {
        let shift = rng.next() % 64;
        pools.push(rng.next() >> shift);
    }
    pools
}

/// Ten shares that sum to at most 10,000, as `initialize_tournament` enforces.
fn shares(rng: &mut Rng) -> [u16; MAX_PRIZE_PLACES] {
    let mut shares = [0u16; MAX_PRIZE_PLACES];
    let mut left = 10_000u16;
    for share in shares.iter_mut() {
        if rng.next().is_multiple_of(3) {
            continue;
        }
        let take = (rng.next() % (u64::from(left) + 1)) as u16;
        *share = take;
        left -= take;
    }
    shares
}

#[test]
fn prize_shares_never_pay_out_more_than_the_pool() -> anyhow::Result<()> {
    let mut rng = Rng::new(seed());
    let fixed: [[u16; MAX_PRIZE_PLACES]; 4] = [
        [10_000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [1; MAX_PRIZE_PLACES],
        [1_000; MAX_PRIZE_PLACES],
        [7_000, 3_000, 0, 0, 0, 0, 0, 0, 0, 0],
    ];
    for pool in pools(&mut rng) {
        let mut cases = fixed.to_vec();
        cases.extend((0..8).map(|_| shares(&mut rng)));
        for case in cases {
            let mut paid: u128 = 0;
            for share in case {
                let amount = prize_amount(pool, share)?;
                assert!(amount <= pool, "pool {pool} share {share}");
                paid += u128::from(amount);
            }
            assert!(
                paid <= u128::from(pool),
                "pool {pool} shares {case:?}: paid {paid}"
            );
            // Flooring loses less than one lamport per place.
            let total_bps: u128 = case.iter().map(|share| u128::from(*share)).sum();
            let exact = u128::from(pool) * total_bps / 10_000;
            assert!(
                exact - paid <= MAX_PRIZE_PLACES as u128,
                "pool {pool} shares {case:?}: rounding lost {}",
                exact - paid
            );
        }
        assert_eq!(prize_amount(pool, 0)?, 0);
        assert_eq!(prize_amount(pool, 10_000)?, pool);
    }
    Ok(())
}

#[test]
fn prize_amount_is_monotonic_in_the_share() -> anyhow::Result<()> {
    let mut rng = Rng::new(seed() ^ 1);
    for pool in pools(&mut rng) {
        let mut previous = 0u64;
        for share in (0..=10_000u16).step_by(97).chain([9_999, 10_000]) {
            let amount = prize_amount(pool, share)?;
            assert!(amount >= previous, "pool {pool} share {share}");
            previous = amount;
        }
    }
    Ok(())
}

#[test]
fn place_bits_are_distinct_and_bounded() -> anyhow::Result<()> {
    let mut seen = 0u16;
    for index in 0..MAX_PRIZE_PLACES {
        let bit = place_bit(index)?;
        assert_eq!(bit.count_ones(), 1);
        assert_eq!(seen & bit, 0, "place {index} reuses a bit");
        seen |= bit;
    }
    assert_eq!(seen, 0b11_1111_1111);
    for index in [MAX_PRIZE_PLACES, 16, usize::MAX] {
        assert!(place_bit(index).is_err(), "place {index}");
    }
    Ok(())
}

#[test]
fn pot_and_session_totals_reject_overflow_instead_of_wrapping() {
    assert_eq!(pot(0).ok(), Some(0));
    assert_eq!(pot(u64::MAX / 2).ok(), Some(u64::MAX - 1));
    assert!(pot(u64::MAX / 2 + 1).is_err());
    assert!(pot(u64::MAX).is_err());

    assert_eq!(checked_session_total(u64::MAX - 1, 1).ok(), Some(u64::MAX));
    assert!(checked_session_total(u64::MAX, 1).is_err());
    assert!(checked_session_total(1, u64::MAX).is_err());
}

#[test]
fn session_budget_never_exceeds_either_cap() {
    let mut rng = Rng::new(seed() ^ 2);
    let edge = [0, 1, RENT_FLOOR, u64::MAX - 1, u64::MAX];
    let mut values: Vec<u64> = edge.to_vec();
    values.extend((0..60).map(|_| rng.next() >> (rng.next() % 64)));
    for &limit in &values {
        for &spent in &values {
            for &max_wager in &values {
                let session = GlobalSessionDelegation {
                    player: Default::default(),
                    session_key: Default::default(),
                    expires_at: i64::MAX,
                    spending_limit: limit,
                    total_spent: spent,
                    max_wager,
                    games_remaining: 1,
                    enabled: true,
                    bump: 255,
                };
                for &amount in &edge {
                    if session.has_budget(amount) {
                        assert!(amount <= max_wager, "{amount} > max wager {max_wager}");
                        let total = u128::from(spent) + u128::from(amount);
                        assert!(
                            total <= u128::from(limit),
                            "limit {limit} spent {spent} amount {amount}"
                        );
                    }
                }
            }
        }
    }
}
