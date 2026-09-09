use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome<T> {
    Duplicate,
    Ready(Vec<T>),
    Overflow { resync_from: u64 },
}

pub struct NonceSequencer<T> {
    expected: u64,
    buffered: BTreeMap<u64, T>,
    max_buffered: usize,
}

impl<T> NonceSequencer<T> {
    pub fn new(max_buffered: usize) -> Self {
        Self {
            expected: 1,
            buffered: BTreeMap::new(),
            max_buffered,
        }
    }

    pub fn expected(&self) -> u64 {
        self.expected
    }

    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }

    pub fn ingest(&mut self, nonce: u64, payload: T) -> IngestOutcome<T> {
        if nonce < self.expected {
            return IngestOutcome::Duplicate;
        }
        if nonce > self.expected {
            self.buffered.insert(nonce, payload);
            if self.buffered.len() > self.max_buffered {
                return self.expire();
            }
            return IngestOutcome::Ready(Vec::new());
        }

        let mut ready = vec![payload];
        self.expected += 1;
        while let Some(next) = self.buffered.remove(&self.expected) {
            ready.push(next);
            self.expected += 1;
        }
        IngestOutcome::Ready(ready)
    }

    pub fn expire(&mut self) -> IngestOutcome<T> {
        let resync_from = self.expected;
        if let Some((&max_seen, _)) = self.buffered.iter().next_back() {
            self.expected = max_seen + 1;
        }
        self.buffered.clear();
        IngestOutcome::Overflow { resync_from }
    }

    pub fn has_buffered(&self) -> bool {
        !self.buffered.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_delivery_applies_immediately() {
        let mut seq = NonceSequencer::new(8);
        assert_eq!(seq.ingest(1, "a"), IngestOutcome::Ready(vec!["a"]));
        assert_eq!(seq.ingest(2, "b"), IngestOutcome::Ready(vec!["b"]));
        assert_eq!(seq.ingest(3, "c"), IngestOutcome::Ready(vec!["c"]));
    }

    #[test]
    fn duplicate_is_rejected_not_reapplied() {
        let mut seq = NonceSequencer::new(8);
        assert_eq!(seq.ingest(1, "a"), IngestOutcome::Ready(vec!["a"]));
        assert_eq!(seq.ingest(1, "a-dup"), IngestOutcome::Duplicate);
        assert_eq!(seq.ingest(1, "a-dup-2"), IngestOutcome::Duplicate);
    }

    #[test]
    fn single_reorder_buffers_then_drains_in_order() {
        // move N+1 arrives (fast relay) before move N (slow gossip) — this is
        // exactly the seam the user flagged.
        let mut seq = NonceSequencer::new(8);
        assert_eq!(seq.ingest(2, "b"), IngestOutcome::Ready(vec![]));
        // N later arrives: both N and the buffered N+1 release, in order.
        assert_eq!(seq.ingest(1, "a"), IngestOutcome::Ready(vec!["a", "b"]));
        // and the duplicate copy of N+1 that eventually shows up via the
        // other transport is correctly recognized as a replay, not reapplied.
        assert_eq!(seq.ingest(2, "b-dup"), IngestOutcome::Duplicate);
    }

    #[test]
    fn deep_reorder_drains_full_run_once_gap_fills() {
        let mut seq = NonceSequencer::new(8);
        assert_eq!(seq.ingest(4, "d"), IngestOutcome::Ready(vec![]));
        assert_eq!(seq.ingest(2, "b"), IngestOutcome::Ready(vec![]));
        assert_eq!(seq.ingest(3, "c"), IngestOutcome::Ready(vec![]));
        assert_eq!(
            seq.ingest(1, "a"),
            IngestOutcome::Ready(vec!["a", "b", "c", "d"])
        );
    }

    #[test]
    fn permanently_dropped_message_triggers_overflow_not_infinite_buffering() {
        let mut seq = NonceSequencer::new(2);
        assert_eq!(seq.ingest(2, "b"), IngestOutcome::Ready(vec![]));
        assert_eq!(seq.ingest(3, "c"), IngestOutcome::Ready(vec![]));
        // third out-of-order arrival with nonce 1 still missing exceeds the bound
        match seq.ingest(4, "d") {
            IngestOutcome::Overflow { resync_from } => assert_eq!(resync_from, 1),
            other => panic!("expected Overflow, got {other:?}"),
        }
        assert!(!seq.has_buffered());
        // sequencer resynced past the lost gap — new in-order traffic proceeds normally
        assert_eq!(seq.ingest(5, "e"), IngestOutcome::Ready(vec!["e"]));
    }

    #[test]
    fn sparse_gap_with_no_further_traffic_is_reaped_by_explicit_expire() {
        // Models the realistic "genuinely dropped, not just reordered" case:
        // the opponent whose turn it is can't produce move 2 until they see
        // move 1, so the buffer never grows past one entry and the count
        // bound alone never fires. The ECS wrapper's wall-clock sweep calls
        // `expire()` directly once the oldest buffered entry has waited too
        // long — this test exercises that path.
        let mut seq = NonceSequencer::new(8);
        assert_eq!(seq.ingest(2, "b"), IngestOutcome::Ready(vec![]));
        assert!(seq.has_buffered());
        match seq.expire() {
            IngestOutcome::Overflow { resync_from } => assert_eq!(resync_from, 1),
            other => panic!("expected Overflow, got {other:?}"),
        }
        assert!(!seq.has_buffered());
    }

    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn range(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    #[test]
    fn dual_transport_interleave_duplicate_drop_fuzz() {
        const MOVES: u64 = 40;
        const TRIALS: usize = 500;

        for trial in 0..TRIALS {
            let mut rng = XorShift(0x9E3779B97F4A7C15u64.wrapping_add(trial as u64 * 2654435761));

            // Build the "wire": every move appears twice (gossip copy + relay
            // copy, both carrying the same nonce/payload) — except ~15% of
            // nonces, which lose exactly one of their two copies, modeling a
            // single transport dropping that particular move. The other copy
            // always survives (matching the relay's at-least-once
            // persistence within its TTL window plus gossip's independent
            // path), so every nonce remains deliverable — this models
            // reordering/duplication/single-transport-loss, not the
            // "genuinely unrecoverable" case (covered separately by
            // `permanently_dropped_message_triggers_overflow_not_infinite_buffering`
            // and `sparse_gap_with_no_further_traffic_is_reaped_by_explicit_expire`).
            let mut wire: Vec<(u64, u64)> = Vec::new();
            for nonce in 1..=MOVES {
                let drop_one_copy = rng.range(100) < 15;
                if !drop_one_copy {
                    wire.push((nonce, nonce)); // "gossip" copy
                }
                wire.push((nonce, nonce)); // "relay" copy (always present)
            }
            // Fisher-Yates shuffle — models the two transports' independent,
            // uncorrelated latency reordering arrivals relative to each other.
            for i in (1..wire.len()).rev() {
                let j = rng.range(i + 1);
                wire.swap(i, j);
            }

            let mut seq = NonceSequencer::new(8);
            let mut applied: Vec<u64> = Vec::new();
            let mut resynced = false;

            for (nonce, payload) in wire {
                match seq.ingest(nonce, payload) {
                    IngestOutcome::Duplicate => {}
                    IngestOutcome::Ready(batch) => {
                        for item in batch {
                            // The single-correct-lineage property: every
                            // nonce this sequencer ever hands back as ready
                            // to apply must be strictly greater than the
                            // last one it handed back. Violating this would
                            // mean a fork (re-applying an already-applied
                            // nonce) or a regression (applying an earlier
                            // nonce after a later one) — exactly the
                            // divergence between "what the P2P layer thinks
                            // happened and what gets recorded" that matters
                            // for a wagered game. This must hold on every
                            // step, resync or not — a resync only excuses
                            // *gaps*, never forks/regressions.
                            if let Some(&last) = applied.last() {
                                assert!(
                                    item > last,
                                    "trial {trial}: nonce {item} applied after {last} — fork or regression, not a single lineage"
                                );
                            }
                            applied.push(item);
                        }
                    }
                    IngestOutcome::Overflow { .. } => {
                        // The buffer gave up on a gap — in production this
                        // fires a ResyncRequest, and the receiver's board
                        // gets snapped to the authoritative FEN wholesale
                        // (not gap-filled). So a resync legitimately excuses
                        // *missing* nonces from here on — but the
                        // strictly-increasing check above still applies to
                        // whatever the sequencer does go on to apply.
                        resynced = true;
                    }
                }
            }

            if !resynced {
                // No gap was ever tolerated: reordering and duplication
                // alone (no permanent loss) must still deliver the complete,
                // exact 1..=MOVES run with nothing skipped.
                let expected: Vec<u64> = (1..=MOVES).collect();
                assert_eq!(
                    applied, expected,
                    "trial {trial}: no overflow occurred but applied sequence isn't the full exact run"
                );
            }
        }
    }
}
