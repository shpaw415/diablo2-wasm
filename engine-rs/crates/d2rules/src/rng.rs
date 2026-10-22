//! PRNG déterministe (SplitMix64) — même graine ⇒ même partie.

/// Générateur SplitMix64 (état 64 bits, sérialisé tel quel dans la sauvegarde).
pub(crate) struct Rng {
    /// État brut.
    pub s: u64,
}

impl Rng {
    /// Nouveau générateur depuis une graine arbitraire.
    pub fn new(seed: u64) -> Rng {
        Rng { s: seed }
    }

    /// Prochain u64 (SplitMix64, Steele et al. 2019).
    pub fn next_u64(&mut self) -> u64 {
        self.s = self.s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Entier uniforme dans [0, n) ; n = 0 ⇒ 0.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            ((self.next_u64() >> 32) as u32) % n
        }
    }

    /// Entier uniforme inclusif [lo, hi] ; hi <= lo ⇒ lo.
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            lo
        } else {
            lo + self.below(hi - lo + 1)
        }
    }

    /// Entier uniforme inclusif [lo, hi] signé.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            lo
        } else {
            lo + self.below((hi - lo + 1) as u32) as i32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
            assert_eq!(a.range_u32(0, 99), b.range_u32(0, 99));
            assert_eq!(a.range_i32(-7, 7), b.range_i32(-7, 7));
        }
    }

    #[test]
    fn ranges_are_bounded() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.range_u32(3, 9);
            assert!((3..=9).contains(&v));
            let w = r.range_i32(-5, -1);
            assert!((-5..=-1).contains(&w));
        }
    }
}
