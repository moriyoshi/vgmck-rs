//! Macro envelope handling
//!
//! Corresponds to MacroEnv and macro_env[][] in original vgmck.c

/// Maximum envelope data length
pub const MAX_ENVELOPE_DATA: usize = 2048;

/// Number of macro types
pub const MAX_MACRO_TYPES: usize = 13;

/// Macro command types (matching original MC_* constants)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MacroType {
    Volume = 0,      // v @v @vr
    Panning = 1,     // P @P
    Tone = 2,        // @ @@
    Option = 3,      // @x @xr
    Arpeggio = 4,    // @EN
    Global = 5,      // @G
    Multiply = 6,    // M @M
    Waveform = 7,    // @W
    ModWaveform = 8, // @WM
    VolumeEnv = 9,   // ve
    Sample = 10,     // @S
    SampleList = 11, // @SL
    Midi = 12,       // @MIDI
}

impl MacroType {
    /// Get static command name (for channel commands)
    pub fn stat_name(&self) -> &'static str {
        match self {
            Self::Volume => "v",
            Self::Panning => "P",
            Self::Tone => "@",
            Self::Option => "",
            Self::Arpeggio => "",
            Self::Global => "@G",
            Self::Multiply => "M",
            Self::Waveform => "@W",
            Self::ModWaveform => "@WM",
            Self::VolumeEnv => "ve",
            Self::Sample => "@S",
            Self::SampleList => "@SL",
            Self::Midi => "",
        }
    }

    /// Get dynamic command name (for envelope definitions)
    pub fn dyn_name(&self) -> &'static str {
        match self {
            Self::Volume => "@v",
            Self::Panning => "@P",
            Self::Tone => "@@",
            Self::Option => "@x",
            Self::Arpeggio => "@EN",
            Self::Global => "",
            Self::Multiply => "@M",
            Self::Waveform => "@W",
            Self::ModWaveform => "",
            Self::VolumeEnv => "",
            Self::Sample => "@S",
            Self::SampleList => "@SL",
            Self::Midi => "@MIDI",
        }
    }

    /// Get dynamic relative command name
    pub fn dyn_rel_name(&self) -> &'static str {
        match self {
            Self::Volume => "@vr",
            Self::Option => "@xr",
            _ => "",
        }
    }

    /// Try to parse from dynamic command name
    pub fn from_dyn_name(name: &str) -> Option<Self> {
        match name {
            "@v" => Some(Self::Volume),
            "@P" => Some(Self::Panning),
            "@@" => Some(Self::Tone),
            "@x" => Some(Self::Option),
            "@EN" => Some(Self::Arpeggio),
            "@M" => Some(Self::Multiply),
            "@W" => Some(Self::Waveform),
            "@S" => Some(Self::Sample),
            "@SL" => Some(Self::SampleList),
            "@MIDI" => Some(Self::Midi),
            _ => None,
        }
    }

    /// Try to parse from static command name
    pub fn from_stat_name(name: &str) -> Option<Self> {
        match name {
            "v" => Some(Self::Volume),
            "P" => Some(Self::Panning),
            "@" => Some(Self::Tone),
            "@G" => Some(Self::Global),
            "M" => Some(Self::Multiply),
            "@W" => Some(Self::Waveform),
            "@WM" => Some(Self::ModWaveform),
            "ve" => Some(Self::VolumeEnv),
            "@S" => Some(Self::Sample),
            "@SL" => Some(Self::SampleList),
            _ => None,
        }
    }

    /// Iterate over all macro types
    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Volume,
            Self::Panning,
            Self::Tone,
            Self::Option,
            Self::Arpeggio,
            Self::Global,
            Self::Multiply,
            Self::Waveform,
            Self::ModWaveform,
            Self::VolumeEnv,
            Self::Sample,
            Self::SampleList,
            Self::Midi,
        ]
        .into_iter()
    }
}

/// Macro envelope data
///
/// Corresponds to MacroEnv struct in original
#[derive(Debug, Clone)]
pub struct MacroEnvelope {
    /// Loop start index (-1 if no loop)
    pub loop_start: i32,
    /// Loop end index (also serves as data length)
    pub loop_end: i32,
    /// Envelope data
    pub data: Vec<i16>,
    /// Optional text label (for samples)
    pub text: String,
}

impl MacroEnvelope {
    pub fn new() -> Self {
        Self {
            loop_start: -1,
            loop_end: 0,
            data: Vec::with_capacity(MAX_ENVELOPE_DATA),
            text: String::new(),
        }
    }

    /// Reset envelope to empty state
    pub fn reset(&mut self) {
        self.loop_start = -1;
        self.loop_end = 0;
        self.data.clear();
        self.text.clear();
    }

    /// Get the length of the envelope data
    pub fn len(&self) -> usize {
        self.loop_end as usize
    }

    /// Check if envelope is empty
    pub fn is_empty(&self) -> bool {
        self.loop_end == 0
    }

    /// Add a value to the envelope
    pub fn push(&mut self, value: i16) {
        if (self.loop_end as usize) < MAX_ENVELOPE_DATA {
            if self.data.len() <= self.loop_end as usize {
                self.data.push(value);
            } else {
                self.data[self.loop_end as usize] = value;
            }
            self.loop_end += 1;
        }
    }

    /// Set loop point at current position
    pub fn set_loop_point(&mut self) {
        self.loop_start = self.loop_end;
    }

    /// Get value at index
    pub fn get(&self, index: usize) -> Option<i16> {
        if index < self.data.len() {
            Some(self.data[index])
        } else {
            None
        }
    }

    /// Get last value
    pub fn last(&self) -> Option<i16> {
        if self.loop_end > 0 {
            self.data.get((self.loop_end - 1) as usize).copied()
        } else {
            None
        }
    }
}

impl Default for MacroEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage for all macro envelopes
/// macro_env[macro_type][envelope_id]
pub type MacroEnvStorage = [[MacroEnvelope; 256]; MAX_MACRO_TYPES];

/// Create default macro envelope storage
pub fn create_macro_env_storage() -> Box<MacroEnvStorage> {
    Box::new(std::array::from_fn(|_| std::array::from_fn(|_| MacroEnvelope::new())))
}
