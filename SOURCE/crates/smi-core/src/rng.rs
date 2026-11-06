//! Générateur pseudo-aléatoire reproductible : xoshiro256++ amorcé par SplitMix64.
//!
//! Une simulation est découpée en lots de taille fixe ; le lot `i` d'une graine `g` utilise toujours le
//! flux [`Xoshiro256pp::stream`]`(g, i)`. Le résultat ne dépend donc ni du nombre de threads ni de
//! l'ordre dans lequel les lots sont traités.

#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[derive(Clone, Debug)]
pub struct Xoshiro256pp {
    s: [u64; 4],
}

impl Xoshiro256pp {
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut sm = seed;
        let mut s = [0u64; 4];
        for v in &mut s {
            *v = splitmix64(&mut sm);
        }
        if s == [0; 4] {
            s[0] = 1;
        }
        Self { s }
    }

    /// Flux indépendant numéro `index` d'une simulation de graine `seed`.
    pub fn stream(seed: u64, index: u64) -> Self {
        let mut sm = seed;
        let a = splitmix64(&mut sm);
        let mut sm2 = a ^ index.wrapping_mul(0xD1B5_4A32_D192_ED03).rotate_left(17);
        Self::seed_from_u64(splitmix64(&mut sm2) ^ index)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Réel uniforme dans [0, 1) sur 53 bits.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Entier uniforme dans [0, n), sans biais (méthode de Lemire).
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let mut x = self.next_u64();
        let mut m = (x as u128) * (n as u128);
        let mut l = m as u64;
        if l < n {
            let t = n.wrapping_neg() % n;
            while l < t {
                x = self.next_u64();
                m = (x as u128) * (n as u128);
                l = m as u64;
            }
        }
        (m >> 64) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_reproducible_and_distinct() {
        let a: Vec<u64> = (0..4).map(|_| 0).scan(Xoshiro256pp::stream(7, 3), |r, _| Some(r.next_u64())).collect();
        let b: Vec<u64> = (0..4).map(|_| 0).scan(Xoshiro256pp::stream(7, 3), |r, _| Some(r.next_u64())).collect();
        let c: Vec<u64> = (0..4).map(|_| 0).scan(Xoshiro256pp::stream(7, 4), |r, _| Some(r.next_u64())).collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn below_is_uniform() {
        let mut r = Xoshiro256pp::seed_from_u64(42);
        let mut counts = [0u32; 6];
        for _ in 0..600_000 {
            counts[r.below(6) as usize] += 1;
        }
        for c in counts {
            assert!((c as i64 - 100_000).abs() < 1_500, "{c}");
        }
    }
}
