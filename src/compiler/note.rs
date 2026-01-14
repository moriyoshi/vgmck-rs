//! Note and frequency calculations

/// Calculated note values for a chip
#[derive(Debug, Clone)]
pub struct NoteTable {
    /// Note values (frequency or period depending on chip)
    pub values: [i64; 32],
}

impl NoteTable {
    pub fn new() -> Self {
        Self { values: [0; 32] }
    }

    /// Calculate note values for a chip
    ///
    /// - `clock_div`: Clock divisor (negative for period-based, positive for frequency-based)
    /// - `note_bits`: Number of bits for note value (negative to not shift by octave)
    /// - `basic_octave`: Base octave number
    /// - `note_freq`: Note frequencies for current scale
    /// - `base_freq`: Base frequency (Hz)
    pub fn calculate(
        clock_div: i32,
        note_bits: i32,
        note_freq: &[f64; 32],
        base_freq: f64,
    ) -> Self {
        let mut table = Self::new();

        if clock_div == 0 {
            return table;
        }

        let bits = note_bits.abs();
        let is_period = clock_div < 0;
        let q = clock_div.abs() as u64;
        let mask = (!0u64) << bits;

        let mut u = [0u64; 32];
        let mut w = 0u64;

        for i in 0..32 {
            let freq = note_freq[i] * base_freq + 0.000001;
            let v = if is_period {
                ((q as u64) << 24) / (freq as u64)
            } else {
                (freq as u64) * ((q as u64) << 22)
            };
            u[i] = v;
            w |= v;
        }

        // Normalize to fit in note_bits
        while (w & mask) != 0 {
            w >>= 1;
            for v in &mut u {
                *v >>= 1;
            }
        }

        for i in 0..32 {
            table.values[i] = u[i] as i64;
        }

        table
    }

    /// Get note value for a given note and octave
    pub fn get(&self, note: i32, octave: i32, basic_octave: i32, clock_div: i32, note_bits: i32) -> i64 {
        if note < 0 || note >= 32 {
            return 0;
        }

        let value = self.values[note as usize];

        if note_bits < 0 {
            // Don't shift by octave
            value
        } else if clock_div < 0 {
            // Period-based: higher octave = shorter period
            value >> (octave - basic_octave)
        } else {
            // Frequency-based: higher octave = higher frequency
            value >> (basic_octave - octave)
        }
    }
}

impl Default for NoteTable {
    fn default() -> Self {
        Self::new()
    }
}
