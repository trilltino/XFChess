use solana_sdk::pubkey::Pubkey;

pub const STATUS_PENDING: u8 = 0;
pub const STATUS_WAITING_FOR_OPPONENT: u8 = 1;
pub const STATUS_ACTIVE: u8 = 2;
pub const STATUS_INACTIVE: u8 = 3;
pub const STATUS_DISPUTED: u8 = 4;
pub const STATUS_FINISHED: u8 = 5;
pub const STATUS_SETTLED: u8 = 6;
pub const STATUS_EXPIRED: u8 = 7;
pub const STATUS_CANCELLED: u8 = 8;

pub const RESULT_NONE: u8 = 0;
pub const RESULT_WINNER: u8 = 1;
pub const RESULT_DRAW: u8 = 2;

#[derive(Debug, Clone)]
pub struct GameAccount {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub status: u8,
    pub last_move_timestamp: i64,
    pub fees_advanced: u64,
    pub fee_payer: Pubkey,
    pub result_tag: u8,
    pub winner: Option<Pubkey>,
    pub move_count: u16,
    pub halfmove_clock: u16,
    pub turn: u16,
    pub created_at: i64,
    pub updated_at: i64,
    pub wager_amount: u64,
    pub country_fee: u64,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
    pub is_delegated: bool,
    pub tournament_id: Option<u64>,
    pub nonce: u64,
}

impl GameAccount {
    pub fn is_finished(&self) -> bool {
        self.status == STATUS_FINISHED
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.status, STATUS_SETTLED | STATUS_EXPIRED)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let slice = self.data.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn bool(&mut self) -> Option<bool> {
        self.u8().map(|b| b != 0)
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn pubkey(&mut self) -> Option<Pubkey> {
        Pubkey::try_from(self.take(32)?).ok()
    }
}

pub fn parse(data: &[u8]) -> Option<GameAccount> {
    let mut r = Reader::new(data);

    r.skip(8)?; // Anchor discriminator
    let game_id = r.u64()?;
    let white = r.pubkey()?;
    let black = r.pubkey()?;
    let status = r.u8()?;
    let last_move_timestamp = r.i64()?;
    let fees_advanced = r.u64()?;
    let fee_payer = r.pubkey()?;

    let result_tag = r.u8()?;
    let winner = match result_tag {
        RESULT_WINNER => Some(r.pubkey()?),
        RESULT_NONE | RESULT_DRAW => None,
        // An unrecognised tag means the layout assumption is broken; every
        // subsequent offset would be guesswork.
        _ => return None,
    };

    r.skip(68)?; // board_state
    let move_count = r.u16()?;
    // The field the old parsers dropped. Its absence shifted everything below
    // by two bytes.
    let halfmove_clock = r.u16()?;
    let turn = r.u16()?;
    let created_at = r.i64()?;
    let updated_at = r.i64()?;
    let wager_amount = r.u64()?;

    match r.u8()? {
        // wager_token: Option<Pubkey>
        0 => {}
        1 => r.skip(32)?,
        _ => return None,
    }

    r.skip(1)?; // game_type
    r.skip(1)?; // match_type
    let country_fee = r.u64()?;
    let base_time_seconds = r.u64()?;
    let increment_seconds = r.u16()?;
    r.skip(1)?; // bump
    let is_delegated = r.bool()?;

    let tournament_id = match r.u8()? {
        0 => None,
        1 => Some(r.u64()?),
        _ => return None,
    };

    let nonce = r.u64()?;

    // `draw_offered_by: Option<Pubkey>` is the final field. Nothing here reads
    // it, but it is consumed so that an account truncated after `nonce` fails
    // to decode instead of passing as complete — the record is only whole once
    // this tag is present.
    match r.u8()? {
        0 => {}
        1 => r.skip(32)?,
        _ => return None,
    }

    Some(GameAccount {
        game_id,
        white,
        black,
        status,
        last_move_timestamp,
        fees_advanced,
        fee_payer,
        result_tag,
        winner,
        move_count,
        halfmove_clock,
        turn,
        created_at,
        updated_at,
        wager_amount,
        country_fee,
        base_time_seconds,
        increment_seconds,
        is_delegated,
        tournament_id,
        nonce,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Builder {
        out: Vec<u8>,
    }

    impl Builder {
        fn new() -> Self {
            Self {
                out: vec![0u8; 8], // discriminator
            }
        }
        fn u8(mut self, v: u8) -> Self {
            self.out.push(v);
            self
        }
        fn u16(mut self, v: u16) -> Self {
            self.out.extend_from_slice(&v.to_le_bytes());
            self
        }
        fn u64(mut self, v: u64) -> Self {
            self.out.extend_from_slice(&v.to_le_bytes());
            self
        }
        fn i64(mut self, v: i64) -> Self {
            self.out.extend_from_slice(&v.to_le_bytes());
            self
        }
        fn key(mut self, k: &Pubkey) -> Self {
            self.out.extend_from_slice(k.as_ref());
            self
        }
        fn zeros(mut self, n: usize) -> Self {
            self.out.extend(std::iter::repeat_n(0u8, n));
            self
        }
    }

    fn sample() -> (Vec<u8>, Pubkey, Pubkey, Pubkey) {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let payer = Pubkey::new_unique();
        let bytes = Builder::new()
            .u64(42) // game_id
            .key(&white)
            .key(&black)
            .u8(STATUS_ACTIVE)
            .i64(1_700_000_000) // last_move_timestamp
            .u64(123_456) // fees_advanced
            .key(&payer)
            .u8(RESULT_NONE)
            .zeros(68) // board_state
            .u16(9) // move_count
            .u16(4) // halfmove_clock
            .u16(10) // turn
            .i64(1_699_000_000) // created_at
            .i64(1_700_000_001) // updated_at
            .u64(50_000_000) // wager_amount
            .u8(0) // wager_token = None
            .u8(0) // game_type
            .u8(1) // match_type
            .u64(7_777) // country_fee
            .u64(300) // base_time_seconds
            .u16(2) // increment_seconds
            .u8(255) // bump
            .u8(1) // is_delegated = true
            .u8(0) // tournament_id = None
            .u64(9) // nonce
            .u8(0) // draw_offered_by = None
            .out;
        (bytes, white, black, payer)
    }

    #[test]
    fn decodes_every_field() {
        let (bytes, white, black, payer) = sample();
        let g = parse(&bytes).expect("a well-formed account must decode");

        assert_eq!(g.game_id, 42);
        assert_eq!(g.white, white);
        assert_eq!(g.black, black);
        assert_eq!(g.fee_payer, payer);
        assert_eq!(g.status, STATUS_ACTIVE);
        assert_eq!(g.fees_advanced, 123_456);
        assert_eq!(g.move_count, 9);
        assert_eq!(g.halfmove_clock, 4);
        assert_eq!(g.turn, 10);
        assert_eq!(g.created_at, 1_699_000_000);
        assert_eq!(g.updated_at, 1_700_000_001);
        assert_eq!(g.wager_amount, 50_000_000);
        assert_eq!(g.country_fee, 7_777);
        assert_eq!(g.base_time_seconds, 300);
        assert_eq!(g.increment_seconds, 2);
        assert!(g.is_delegated);
        assert_eq!(g.tournament_id, None);
        assert_eq!(g.nonce, 9);
    }

    #[test]
    fn offsets_match_the_on_chain_layout() {
        const WAGER_ABS_OFFSET: usize = 8 + 212;

        let (mut bytes, ..) = sample();
        let marker: u64 = 0x1122_3344_5566_7788;
        bytes[WAGER_ABS_OFFSET..WAGER_ABS_OFFSET + 8].copy_from_slice(&marker.to_le_bytes());

        let g = parse(&bytes).expect("decode");
        assert_eq!(
            g.wager_amount, marker,
            "wager_amount must sit at 8 + 212, matching the program's pinned test and the \
             client's WAGER_OFFSET in src/multiplayer/solana/lobby.rs"
        );
    }

    #[test]
    fn decodes_a_winner_and_a_tournament_id() {
        let winner = Pubkey::new_unique();
        let white = Pubkey::new_unique();
        let bytes = Builder::new()
            .u64(7)
            .key(&white)
            .key(&Pubkey::new_unique())
            .u8(STATUS_FINISHED)
            .i64(0)
            .u64(0)
            .key(&Pubkey::new_unique())
            .u8(RESULT_WINNER)
            .key(&winner)
            .zeros(68)
            .u16(30)
            .u16(0)
            .u16(31)
            .i64(0)
            .i64(0)
            .u64(1_000)
            .u8(0)
            .u8(0)
            .u8(1)
            .u64(10)
            .u64(600)
            .u16(5)
            .u8(254)
            .u8(0)
            .u8(1) // tournament_id = Some
            .u64(88)
            .u64(30) // nonce
            .u8(0)
            .out;

        let g = parse(&bytes).expect("decode");
        assert_eq!(g.winner, Some(winner));
        assert_eq!(g.tournament_id, Some(88));
        assert_eq!(g.nonce, 30);
        assert!(g.is_finished());
    }

    #[test]
    fn refuses_truncated_and_malformed_accounts() {
        let (bytes, ..) = sample();
        for cut in [0, 8, 100, bytes.len() - 1] {
            assert!(
                parse(&bytes[..cut]).is_none(),
                "a {cut}-byte account must not decode"
            );
        }

        let mut bad_tag = bytes.clone();
        bad_tag[129] = 9; // impossible GameResult discriminant
        assert!(
            parse(&bad_tag).is_none(),
            "an unrecognised enum tag must fail rather than guess subsequent offsets"
        );
    }
}
