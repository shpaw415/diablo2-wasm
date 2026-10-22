//! SplitMix64 — même PRNG que `d2rules` (30 lignes, zéro dépendance).
//! État sérialisé dans la sauvegarde : déterminisme bit-exact après reload.

use serde::{Deserialize, Serialize};

/// Générateur pseudo-aléatoire déterministe (SplitMix64).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    /// État interne (64 bits, public pour la sérialisation de sauvegarde).
    pub s: u64,
}

impl Rng {
    /// Nouveau générateur depuis une graine quelconque (0 inclus).
    pub fn new(seed: u64) -> Rng {
        Rng { s: seed }
    }

    /// Prochain u64 brut (SplitMix64). `next_u64` (et non `next`) pour ne pas
    /// être confondu avec `Iterator::next`.
    pub fn next_u64(&mut self) -> u64 {
        self.s = self.s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Entier uniforme dans `[0, bound)` (0 si `bound == 0`).
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            0
        } else {
            // Rejet d'échantillonnage pour éviter le biais modulo.
            let bound64 = bound as u64;
            let limit = u64::MAX - (u64::MAX % bound64) - 1;
            loop {
                let v = self.next_u64();
                if v <= limit {
                    return (v % bound64) as u32;
                }
            }
        }
    }

    /// Entier uniforme dans `[a, b]` inclusif (clampé si `a > b`).
    pub fn range_u32(&mut self, a: u32, b: u32) -> u32 {
        if a >= b {
            a
        } else {
            a + self.below(b - a + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_per_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn rng_range_is_inclusive_and_bounded() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range_u32(3, 8);
            assert!((3..=8).contains(&v));
        }
        assert_eq!(r.range_u32(5, 5), 5);
        assert_eq!(r.range_u32(9, 2), 9); // clampé
        assert_eq!(r.below(0), 0);
    }
}
