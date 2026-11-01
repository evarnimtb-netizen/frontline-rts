//! Small random sources for the network layer: a seeded generator for tests
//! and link simulation, and unpredictable values for tokens and nonces.
//! Nothing here feeds the sim.

use std::hash::{BuildHasher, RandomState};

/// SplitMix64: seeded, fast, good enough for jitter, drops and scripts.
/// Its output reveals its state, so it never makes tokens outside tests.
#[derive(Clone, Debug)]
pub struct SplitMix(u64);

impl SplitMix {
    pub fn new(seed: u64) -> SplitMix {
        SplitMix(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n > 0`; a tiny modulo bias is fine here).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
}

/// Unpredictable 64-bit values: SipHash-1-3 of a counter under the
/// process's random `RandomState` keys (seeded from the OS). Not a CSPRNG
/// by contract, but knowing one output does not give the next, which is
/// what tokens need (S2 "Security": a player who knows its own token must
/// not be able to compute the other's).
pub struct OsRandom {
    keys: RandomState,
    counter: u64,
}

impl OsRandom {
    pub fn new() -> OsRandom {
        OsRandom {
            keys: RandomState::new(),
            counter: 0,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.counter += 1;
        self.keys.hash_one(self.counter)
    }
}

impl Default for OsRandom {
    fn default() -> OsRandom {
        OsRandom::new()
    }
}

/// Token source of a relay: unpredictable in real use, seeded in tests so
/// a run repeats exactly.
pub enum TokenSource {
    Os(OsRandom),
    Seeded(SplitMix),
}

impl TokenSource {
    pub fn os() -> TokenSource {
        TokenSource::Os(OsRandom::new())
    }

    pub fn seeded(seed: u64) -> TokenSource {
        TokenSource::Seeded(SplitMix::new(seed))
    }

    /// A non-zero value (0 means "no token" on the wire).
    pub fn next_token(&mut self) -> u64 {
        loop {
            let v = match self {
                TokenSource::Os(r) => r.next_u64(),
                TokenSource::Seeded(r) => r.next_u64(),
            };
            if v != 0 {
                return v;
            }
        }
    }
}
