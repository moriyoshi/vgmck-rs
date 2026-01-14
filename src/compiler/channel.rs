//! Channel state management

/// Channel definition and state
#[derive(Debug, Clone)]
pub struct Channel {
    /// Associated chip name
    pub chip_name: String,
    /// Chip sub-index (for dual chip support)
    pub chip_sub: usize,
    /// Channel sub-index within chip
    pub chan_sub: usize,
    /// MML text for this channel
    pub text: String,
    /// Loop point in samples (-1 if no loop)
    pub loop_point: i64,
    /// Total duration in samples
    pub duration: i64,
}

impl Channel {
    pub fn new(chip_name: String, chip_sub: usize, chan_sub: usize) -> Self {
        Self {
            chip_name,
            chip_sub,
            chan_sub,
            text: String::new(),
            loop_point: -1,
            duration: 0,
        }
    }

    pub fn append_text(&mut self, text: &str) {
        self.text.push_str(text);
    }
}

/// Channel state during compilation
#[derive(Debug, Clone)]
pub struct ChannelState {
    /// Current octave
    pub octave: i32,
    /// Current tempo (BPM)
    pub tempo: i32,
    /// Default note length (in samples)
    pub default_length: i64,
    /// Current time position (in samples)
    pub time: i64,
    /// Transpose amount
    pub transpose: i32,
    /// Detune amount
    pub detune: i64,
    /// Quantize amount
    pub quantize: i64,
    /// Current note (-1 for rest, -2 for wait)
    pub current_note: i32,
    /// Current note length
    pub current_length: i64,
    /// Active macro envelopes by type
    pub active_macros: [i32; 13],
    /// Note off event mode
    pub note_off_event: i32,
    /// Sample list ID
    pub sample_list: i32,
    /// Phase for channel grouping
    pub phase: i32,
    /// Phase count for channel grouping
    pub phase_count: i32,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            octave: 4,
            tempo: 120,
            default_length: 0, // Will be calculated
            time: 0,
            transpose: 0,
            detune: 0,
            quantize: 0,
            current_note: -1,
            current_length: 0,
            active_macros: [-1; 13],
            note_off_event: 0,
            sample_list: -1,
            phase: 0,
            phase_count: 1,
        }
    }
}

impl ChannelState {
    pub fn new(tempo: i32) -> Self {
        let mut state = Self::default();
        state.tempo = tempo;
        state.default_length = calc_note_length(tempo, 4, 0);
        state
    }
}

/// Calculate note length in samples
/// tempo: BPM
/// length: note value (4 = quarter, 8 = eighth, etc.)
/// dots: number of dots
pub fn calc_note_length(tempo: i32, length: i32, dots: i32) -> i64 {
    if length == 0 {
        return 0;
    }
    // 10584000 = 44100 * 60 * 4 (samples per whole note at 1 BPM)
    let mut k = 10584000i64 / length as i64;
    let mut j = k;
    for _ in 0..dots {
        j /= 2;
        k += j;
    }
    k / tempo as i64
}
