use rp2040_hal::pac;

/// Build a 32-bit seed from the RP2040's ring oscillator.
///
/// Call this early in `main()`, e.g. `let seed = rosc_seed(&pac.ROSC);`.
/// The result is never zero, so it is safe to use with xorshift32.
pub fn rosc_seed(rosc: &pac::ROSC) -> u32 {
    let mut seed: u32 = 0;
    for _ in 0..32 {
        let bit = rosc.randombit().read().randombit().bit() as u32;
        seed = (seed << 1) | bit;
        // small gap so successive samples are less correlated
        for _ in 0..100 {
            core::hint::spin_loop();
        }
    }

    // Scramble (murmur3 finalizer) so weak/biased bits get spread around
    seed ^= seed >> 16;
    seed = seed.wrapping_mul(0x85eb_ca6b);
    seed ^= seed >> 13;
    seed = seed.wrapping_mul(0xc2b2_ae35);
    seed ^= seed >> 16;

    // xorshift32 breaks on a zero state
    if seed == 0 {
        0xDEAD_BEEF
    } else {
        seed
    }
}

pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // xorshift32 never leaves a zero state, so avoid it
        Self {
            state: if seed == 0 { 0xDEAD_BEEF } else { seed },
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        // xorshift32
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    pub fn range_u64(&mut self, min: u64, max: u64) -> u64 {
        min + (self.next_u32() as u64 * (max - min)) / u32::MAX as u64
    }
}
