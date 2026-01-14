//! MML Compiler - parses MML and generates VGM events
//!
//! This module closely follows the structure of the original vgmck.c

pub mod channel;
pub mod envelope;
pub mod event;
pub mod note;
pub mod sample;

/// GD3 text field indices
pub mod gd3 {
    pub const TITLE_EN: usize = 0;
    pub const TITLE_JP: usize = 1;
    pub const GAME_EN: usize = 2;
    pub const GAME_JP: usize = 3;
    pub const SYSTEM_EN: usize = 4;
    pub const SYSTEM_JP: usize = 5;
    pub const COMPOSER_EN: usize = 6;
    pub const COMPOSER_JP: usize = 7;
    pub const DATE: usize = 8;
    pub const CONVERTER: usize = 9;
    pub const NOTES: usize = 10;
    pub const COUNT: usize = 11;
}

use crate::chips::{self, ChipInstance, ChipOptions, MacroCommand};
use crate::error::{Error, Result};
use envelope::{create_macro_env_storage, MacroEnvStorage, MacroType, MAX_MACRO_TYPES};
use crate::vgm::VgmWriter;
use channel::Channel;
use event::{Event, EventData, EventQueue};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

/// Number of available channels (A-Z = 26, a-z = 26)
pub const MAX_CHANNELS: usize = 52;

/// Default frame rate (44100 / 60)
pub const DEFAULT_FRAMERATE: i32 = 735;

/// Main compiler state
pub struct Compiler {
    /// Channel definitions
    pub channels: [Option<Channel>; MAX_CHANNELS],
    /// Chip instances by name
    pub chips: HashMap<String, ChipInstance>,
    /// Event queue
    pub events: EventQueue,
    /// GD3 metadata text (indexed by gd3::* constants)
    pub gd3_text: [String; gd3::COUNT],
    /// Total samples in output
    pub total_samples: i64,
    /// Loop point (in samples)
    pub loop_point: i64,
    /// Loop enabled
    pub loop_on: bool,
    /// Frame rate (samples per frame)
    pub framerate: i32,
    /// Base frequency for note calculation
    pub base_freq: f64,
    /// Note frequencies for current scale
    pub note_freq: [f64; 32],
    /// Note letter mappings (a-j -> semitone offset)
    pub note_letter: [i32; 10],
    /// Calculated note values (set per-chip)
    pub note_value: [i64; 32],
    /// Notes per octave
    pub octave_count: i32,
    /// Volume modifier for VGM header
    pub volume_mod: i16,
    /// Loop base for VGM header
    pub loop_base: i8,
    /// Loop modifier for VGM header
    pub loop_mod: u8,
    /// Recording rate for VGM header
    pub recording_rate: i32,
    /// Text macros (*X definitions)
    pub text_macros: [String; 128],
    /// Macro envelopes
    pub macro_env: Box<MacroEnvStorage>,
    /// Currently active macro envelope indices per macro type
    pub macro_use: [i32; MAX_MACRO_TYPES],
    /// Fast forward amount
    pub fast_forward: i64,
    /// Portamento parameters
    pub portamento: [i64; 8],
    /// Note off event mode
    pub note_off_event: i32,
    /// Sample list ID
    pub sample_list: i32,
    /// Debug input lines flag
    pub debug_input_lines: bool,
    /// Base path for resolving #INCLUDE paths
    base_path: Option<PathBuf>,

    // Envelope parsing state (static in original)
    env_mac: i32,
    env_id: usize,
    env_block: usize,
    env_rep: i32,
    env_brep: [i32; 32],
    env_bst: [i32; 32],
}

impl Compiler {
    pub fn new() -> Self {
        let mut note_freq = [0.0; 32];
        // Initialize equal temperament (12-TET)
        for i in 0..12 {
            note_freq[i] = 2.0_f64.powf(i as f64 / 12.0);
        }
        for i in 12..32 {
            note_freq[i] = 1.99999;
        }

        // Base frequency: C8 = 3520 * 2^(3/12) Hz
        let base_freq = 3520.0 * 2.0_f64.powf(3.0 / 12.0);

        // Default note letter mapping: a=A(9), b=B(11), c=C(0), d=D(2), e=E(4), f=F(5), g=G(7)
        let note_letter = [9, 11, 0, 2, 4, 5, 7, 0, 0, 0];

        Self {
            channels: std::array::from_fn(|_| None),
            chips: HashMap::new(),
            events: EventQueue::new(),
            gd3_text: std::array::from_fn(|_| String::new()),
            total_samples: 0,
            loop_point: 0,
            loop_on: false,
            framerate: DEFAULT_FRAMERATE,
            base_freq,
            note_freq,
            note_letter,
            note_value: [0; 32],
            octave_count: 12,
            volume_mod: 0,
            loop_base: 0,
            loop_mod: 0,
            recording_rate: 0,
            text_macros: std::array::from_fn(|_| String::new()),
            macro_env: create_macro_env_storage(),
            macro_use: [-1; MAX_MACRO_TYPES],
            fast_forward: 0,
            portamento: [0; 8],
            note_off_event: 0,
            sample_list: -1,
            debug_input_lines: false,
            base_path: None,
            env_mac: -1,
            env_id: 0,
            env_block: 0,
            env_rep: 1,
            env_brep: [0; 32],
            env_bst: [0; 32],
        }
    }

    /// Compile MML input to VGM output
    pub fn compile<R: Read>(&mut self, input: R, output: &Path) -> Result<()> {
        // Parse input
        self.read_input(input)?;

        // Compile each channel
        for i in 0..MAX_CHANNELS {
            if self.channels[i].is_some() {
                self.compile_channel(i)?;
            }
        }

        // Write output
        let mut writer = VgmWriter::new(output)?;
        self.write_output(&mut writer)?;

        Ok(())
    }

    /// Compile MML file to VGM output
    ///
    /// This method sets the base path for resolving #INCLUDE directives.
    pub fn compile_file(&mut self, input: &Path, output: &Path) -> Result<()> {
        // Set base path for includes
        self.base_path = input.parent().map(|p| p.to_path_buf());

        // Read and parse input file
        self.read_input_from_path(input)?;

        // Compile each channel
        for i in 0..MAX_CHANNELS {
            if self.channels[i].is_some() {
                self.compile_channel(i)?;
            }
        }

        // Write output
        let mut writer = VgmWriter::new(output)?;
        self.write_output(&mut writer)?;

        Ok(())
    }

    /// Read input from a file path
    fn read_input_from_path(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to open '{}': {}", path.display(), e),
            ))
        })?;
        self.read_input(file)
    }

    /// Add text to a GD3 field
    fn add_gd3(&mut self, field: usize, text: &str) {
        if field < gd3::COUNT {
            if !self.gd3_text[field].is_empty() {
                self.gd3_text[field].push('\n');
            }
            self.gd3_text[field].push_str(text);
        }
    }

    /// Convert channel character to index (A-Z = 0-25, a-z = 26-51)
    fn channel_index(ch: char) -> Option<usize> {
        match ch {
            'A'..='Z' => Some((ch as usize) - ('A' as usize)),
            'a'..='z' => Some((ch as usize) - ('a' as usize) + 26),
            _ => None,
        }
    }

    /// Read a number from string, advancing the position
    /// Supports decimal and hex ($XX) with optional sign
    fn read_num(s: &str, pos: &mut usize) -> i64 {
        let bytes = s.as_bytes();
        let mut base = 10i64;
        let mut sign = 1i64;
        let mut value = 0i64;

        // Skip comma
        if *pos < bytes.len() && bytes[*pos] == b',' {
            *pos += 1;
        }

        // Check for hex prefix or sign
        while *pos < bytes.len() {
            match bytes[*pos] {
                b'$' => {
                    base = 16;
                    *pos += 1;
                }
                b'+' => {
                    sign = 1;
                    *pos += 1;
                }
                b'-' => {
                    sign = -1;
                    *pos += 1;
                }
                _ => break,
            }
        }

        // Parse digits
        while *pos < bytes.len() {
            let b = bytes[*pos];
            let digit = if b >= b'0' && b <= b'9' {
                Some((b - b'0') as i64)
            } else if base == 16 && b >= b'A' && b <= b'F' {
                Some((b - b'A' + 10) as i64)
            } else if base == 16 && b >= b'a' && b <= b'f' {
                Some((b - b'a' + 10) as i64)
            } else {
                None
            };

            if let Some(d) = digit {
                value = value * base + d;
                *pos += 1;
            } else {
                break;
            }
        }

        sign * value
    }

    /// Check if character is "graphic" (printable, > space)
    #[allow(dead_code)]
    fn is_graphic(c: u8) -> bool {
        c > b' '
    }

    /// Read and parse MML input
    fn read_input<R: Read>(&mut self, input: R) -> Result<()> {
        let reader = BufReader::new(input);

        for line in reader.lines() {
            let line = line?;

            // Strip trailing non-graphic characters
            let line = line.trim_end();

            // Strip UTF-8 BOM and leading whitespace
            let line = line.trim_start_matches('\u{FEFF}');
            let line = line.trim_start();

            if line.is_empty() {
                continue;
            }

            if self.debug_input_lines {
                eprintln!("{}", line);
            }

            let first_char = line.bytes().next().unwrap();

            match first_char {
                b'"' => {
                    // Notes (GD3 text field 10)
                    self.add_gd3(gd3::NOTES, &line[1..]);
                }
                b'#' => {
                    if line == "#EOF" {
                        break;
                    }
                    self.parse_global_command(&line[1..])?;
                }
                b'*' => {
                    // Text macro definition
                    if line.len() >= 2 {
                        let id = line.as_bytes()[1] as usize;
                        if id < 128 {
                            let text = if line.len() > 2 { &line[2..] } else { "" };
                            self.text_macros[id] = text.to_string();
                        }
                    }
                }
                b'@' | b'-' | b'+' | b'$' | b'[' | b']' | b'{' | b',' | b'|' | b'0'..=b'9' => {
                    self.parse_envelope(line);
                }
                b'A'..=b'Z' | b'a'..=b'z' => {
                    self.parse_channel_line(line)?;
                }
                _ => {
                    // Ignore other lines
                }
            }
        }

        Ok(())
    }

    /// Parse a global command (#COMMAND params)
    fn parse_global_command(&mut self, cmd: &str) -> Result<()> {
        // Split into command and parameter
        let mut parts = cmd.splitn(2, |c: char| c.is_whitespace());
        let command = parts.next().unwrap_or("");
        let param = parts.next().unwrap_or("").trim();

        match command {
            "TITLE" => {
                self.add_gd3(gd3::TITLE_EN, param);
                self.add_gd3(gd3::TITLE_JP, param);
            }
            "TITLE-E" => self.add_gd3(gd3::TITLE_EN, param),
            "TITLE-J" => self.add_gd3(gd3::TITLE_JP, param),
            "GAME" => {
                self.add_gd3(gd3::GAME_EN, param);
                self.add_gd3(gd3::GAME_JP, param);
            }
            "GAME-E" => self.add_gd3(gd3::GAME_EN, param),
            "GAME-J" => self.add_gd3(gd3::GAME_JP, param),
            "SYSTEM" => {
                self.add_gd3(gd3::SYSTEM_EN, param);
                self.add_gd3(gd3::SYSTEM_JP, param);
            }
            "SYSTEM-E" => self.add_gd3(gd3::SYSTEM_EN, param),
            "SYSTEM-J" => self.add_gd3(gd3::SYSTEM_JP, param),
            "COMPOSER" => {
                self.add_gd3(gd3::COMPOSER_EN, param);
                self.add_gd3(gd3::COMPOSER_JP, param);
            }
            "COMPOSER-E" => self.add_gd3(gd3::COMPOSER_EN, param),
            "COMPOSER-J" => self.add_gd3(gd3::COMPOSER_JP, param),
            "PROGRAMER" | "PROGRAMMER" => self.add_gd3(gd3::CONVERTER, param),
            "DATE" => self.add_gd3(gd3::DATE, param),
            "NOTES" => self.add_gd3(gd3::NOTES, param),
            "RATE" => {
                let mut pos = 0;
                let rate = Self::read_num(param, &mut pos) as i32;
                if rate < 0 {
                    self.framerate = 44100 / (-rate);
                    self.recording_rate = 0;
                } else if rate > 0 {
                    self.framerate = 44100 / rate;
                    self.recording_rate = rate;
                }
            }
            "VOLUME" => {
                let mut pos = 0;
                self.volume_mod = Self::read_num(param, &mut pos) as i16;
            }
            "LOOP-BASE" => {
                let mut pos = 0;
                self.loop_base = Self::read_num(param, &mut pos) as i8;
            }
            "LOOP-MODIFIER" => {
                let mut pos = 0;
                self.loop_mod = Self::read_num(param, &mut pos) as u8;
            }
            "SCALE" => self.parse_scale(param),
            "EQUAL-TEMPERAMENT" => self.make_equal_temperament(),
            "JUST-INTONATION" => self.parse_just_intonation(param),
            "PITCH-CHANGE" => {
                let mut pos = 0;
                self.base_freq = Self::read_num(param, &mut pos) as f64 * 10.0;
            }
            "INCLUDE" => {
                // Resolve path relative to base_path
                let include_path = if let Some(ref base) = self.base_path {
                    base.join(param)
                } else {
                    PathBuf::from(param)
                };

                // Read the included file
                if let Err(e) = self.read_input_from_path(&include_path) {
                    eprintln!("Warning: Failed to include '{}': {}", param, e);
                }
            }
            "DEBUG-INPUT-LINES" => {
                let mut pos = 0;
                self.debug_input_lines = Self::read_num(param, &mut pos) != 0;
            }
            _ if command.starts_with("EX-") => {
                let chip_name = &command[3..];
                self.parse_chip_enable(chip_name, param)?;
            }
            _ if command.starts_with("TEXT") => {
                // TEXTn commands - extract number and add to that GD3 field
                if let Ok(n) = command[4..].parse::<usize>() {
                    self.add_gd3(n, param);
                }
            }
            _ => {
                // Unknown command, ignore
            }
        }

        Ok(())
    }

    /// Parse #EX-CHIP channel_list options
    fn parse_chip_enable(&mut self, chip_name: &str, params: &str) -> Result<()> {
        // Create chip instance
        let mut instance = chips::create_chip(chip_name)?;

        // Parse parameters: "channels options"
        let mut parts = params.splitn(2, |c: char| c.is_whitespace());
        let channels_str = parts.next().unwrap_or("");
        let options_str = parts.next().unwrap_or("");

        // Parse channel assignments
        let mut chip_sub = 0usize;
        let mut chan_sub = 0usize;

        for c in channels_str.chars() {
            match c {
                ',' => {
                    chip_sub += 1;
                    chan_sub = 0;
                }
                '_' => {
                    chan_sub += 1;
                }
                _ => {
                    if let Some(idx) = Self::channel_index(c) {
                        self.channels[idx] = Some(Channel::new(
                            chip_name.to_string(),
                            chip_sub,
                            chan_sub,
                        ));
                        chan_sub += 1;
                    }
                }
            }
        }

        // Parse options
        let mut options = ChipOptions::new();
        let mut pos = 0usize;
        let opt_bytes = options_str.as_bytes();
        let mut current_key = 0u8;

        while pos < opt_bytes.len() {
            let b = opt_bytes[pos];
            match b {
                b' ' => {
                    current_key = 0;
                    pos += 1;
                }
                b'+' => {
                    if pos + 1 < opt_bytes.len() {
                        options.set(opt_bytes[pos + 1] as char, 1);
                        pos += 2;
                    } else {
                        pos += 1;
                    }
                }
                b'-' => {
                    if pos + 1 < opt_bytes.len() {
                        options.set(opt_bytes[pos + 1] as char, 0);
                        pos += 2;
                    } else {
                        pos += 1;
                    }
                }
                b'=' => {
                    pos += 1;
                    let value = Self::read_num(options_str, &mut pos);
                    options.set(current_key as char, value as i32);
                    current_key = 0;
                }
                b':' if current_key == b'o' => {
                    pos += 1;
                    let value = Self::read_num(options_str, &mut pos);
                    // Set basic octave on chip - this is handled in enable()
                    options.set('o', value as i32);
                    current_key = 0;
                }
                b':' if current_key == b'N' => {
                    pos += 1;
                    let value = Self::read_num(options_str, &mut pos);
                    options.set('N', value as i32);
                    current_key = 0;
                }
                _ => {
                    current_key = b;
                    pos += 1;
                }
            }
        }

        // Enable chip with options
        instance.chip.enable(&options);
        instance.options = options;

        self.chips.insert(chip_name.to_string(), instance);
        Ok(())
    }

    /// Parse #SCALE definition
    fn parse_scale(&mut self, scale: &str) {
        let mut x = 0i32;
        for c in scale.chars() {
            match c {
                'a'..='j' => {
                    let idx = (c as usize) - ('a' as usize);
                    self.note_letter[idx] = x;
                    x += 1;
                }
                '.' => x += 1,
                _ => {}
            }
        }
        self.octave_count = x;
    }

    /// Initialize equal temperament
    fn make_equal_temperament(&mut self) {
        for i in 0..self.octave_count as usize {
            self.note_freq[i] = 2.0_f64.powf(i as f64 / self.octave_count as f64);
        }
    }

    /// Parse #JUST-INTONATION ratios
    fn parse_just_intonation(&mut self, params: &str) {
        let mut pos = 0;
        for i in 0..self.octave_count as usize {
            let num = Self::read_num(params, &mut pos);
            let denom = Self::read_num(params, &mut pos);
            if denom != 0 {
                self.note_freq[i] = num as f64 / denom as f64;
            }
        }
    }

    /// Parse envelope definition line
    fn parse_envelope(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut pos = 0;

        // Check if this starts a new envelope definition
        if bytes.get(0) == Some(&b'@') {
            self.env_block = 0;
            self.env_rep = 1;

            // Extract macro name (up to 7 chars starting with @)
            let mut name = String::new();
            while pos < bytes.len() && pos < 7 {
                let b = bytes[pos];
                if b >= b'@' && b != b'{' {
                    name.push(b as char);
                    pos += 1;
                } else {
                    break;
                }
            }

            // Find matching macro type
            self.env_mac = -1;
            for mac_type in MacroType::all() {
                if name == mac_type.dyn_name() {
                    self.env_mac = mac_type as i32;
                    break;
                }
            }

            if self.env_mac == -1 {
                return;
            }

            // Read envelope ID
            self.env_id = (Self::read_num(line, &mut pos) & 255) as usize;

            // Reset envelope
            let env = &mut self.macro_env[self.env_mac as usize][self.env_id];
            env.loop_start = -1;
            env.loop_end = 0;
            env.data.clear();
        }

        if self.env_mac == -1 {
            return;
        }

        // Parse envelope data
        loop {
            // Skip whitespace
            while pos < bytes.len() && bytes[pos] <= b' ' {
                pos += 1;
            }

            if pos >= bytes.len() {
                break;
            }

            let b = bytes[pos];

            if (b >= b'0' && b <= b'9') || b == b'-' || b == b'+' || b == b'$' {
                // Number value
                let env = &mut self.macro_env[self.env_mac as usize][self.env_id];
                if env.loop_end as usize >= envelope::MAX_ENVELOPE_DATA {
                    return;
                }
                let x = Self::read_num(line, &mut pos) as i16;
                for _ in 0..self.env_rep {
                    env.push(x);
                }
            } else if b == b'|' {
                // Loop point
                let env = &mut self.macro_env[self.env_mac as usize][self.env_id];
                env.set_loop_point();
                pos += 1;
            } else if b == b'\'' {
                // Repeat count
                pos += 1;
                self.env_rep = Self::read_num(line, &mut pos) as i32;
            } else if b == b',' && pos + 1 < bytes.len() && bytes[pos + 1] >= b'a' && bytes[pos + 1] <= b'j' {
                // Note-based repeat (e.g., ",c" means repeat to note C)
                pos += 1;
                let note_idx = (bytes[pos] - b'a') as usize;
                pos += 1;
                let env = &mut self.macro_env[self.env_mac as usize][self.env_id];
                let mut x = self.note_letter[note_idx] - env.loop_end;

                // Handle accidentals
                while pos < bytes.len() {
                    if bytes[pos] == b'+' {
                        x += 1;
                        pos += 1;
                    } else if bytes[pos] == b'-' {
                        x -= 1;
                        pos += 1;
                    } else {
                        break;
                    }
                }

                x += Self::read_num(line, &mut pos) as i32 * self.octave_count;

                if let Some(last_val) = env.last() {
                    while x > 0 {
                        env.push(last_val);
                        x -= 1;
                    }
                }
            } else if b == b'=' || b == b'{' || b == b',' {
                pos += 1;
            } else if b == b'[' {
                // Block start
                self.env_brep[self.env_block] = self.env_rep;
                let env = &self.macro_env[self.env_mac as usize][self.env_id];
                self.env_bst[self.env_block] = env.loop_end;
                self.env_block += 1;
                pos += 1;
            } else if b == b']' && self.env_block > 0 {
                // Block end with repeat
                pos += 1;
                let repeat_count = Self::read_num(line, &mut pos) as i32;
                let env = &mut self.macro_env[self.env_mac as usize][self.env_id];
                let y = env.loop_end;
                self.env_block -= 1;
                let block_start = self.env_bst[self.env_block] as usize;

                // Repeat the block
                for _ in 1..repeat_count {
                    for j in block_start..(y as usize) {
                        if let Some(val) = env.data.get(j).copied() {
                            env.push(val);
                        }
                    }
                }
                self.env_rep = self.env_brep[self.env_block];
            } else if b == b'"' {
                // Text label
                pos += 1;
                let mut text = String::new();
                while pos < bytes.len() && bytes[pos] != b'"' && text.len() < 63 {
                    text.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < bytes.len() && bytes[pos] == b'"' {
                    pos += 1;
                }
                self.macro_env[self.env_mac as usize][self.env_id].text = text;
            } else if b == b':' {
                // Ramp to value
                let mut step_size = 0;
                while pos < bytes.len() && bytes[pos] == b':' {
                    step_size += 1;
                    pos += 1;
                }
                let target = Self::read_num(line, &mut pos) as i16;
                let env = &mut self.macro_env[self.env_mac as usize][self.env_id];
                if let Some(mut current) = env.last() {
                    let dir = if target > current { step_size } else { -step_size };
                    while current != target {
                        current += dir as i16;
                        for _ in 0..self.env_rep {
                            env.push(current);
                        }
                        if (dir > 0 && current >= target) || (dir < 0 && current <= target) {
                            break;
                        }
                    }
                }
            } else {
                // Unknown character, end parsing
                return;
            }
        }
    }

    /// Parse channel data line (e.g., "ABC cdefg")
    fn parse_channel_line(&mut self, line: &str) -> Result<()> {
        let bytes = line.as_bytes();
        let mut pos = 0;

        // Collect channel names
        let mut channel_indices = Vec::new();
        while pos < bytes.len() && bytes[pos] > b' ' {
            if let Some(idx) = Self::channel_index(bytes[pos] as char) {
                channel_indices.push(idx);
            } else {
                break;
            }
            pos += 1;
        }

        if channel_indices.is_empty() {
            return Ok(());
        }

        // Process remaining text, expanding text macros
        let mut text = String::new();
        while pos < bytes.len() {
            let b = bytes[pos];
            if b == b';' {
                // Comment - stop here
                break;
            } else if b == b'*' && pos + 1 < bytes.len() {
                // Text macro expansion
                let macro_id = bytes[pos + 1] as usize;
                if macro_id < 128 {
                    text.push_str(&self.text_macros[macro_id]);
                }
                pos += 2;
            } else {
                text.push(b as char);
                pos += 1;
            }
        }

        // Append to all specified channels
        for &idx in &channel_indices {
            if let Some(ref mut channel) = self.channels[idx] {
                channel.text.push_str(&text);
            } else {
                let ch = if idx < 26 {
                    (b'A' + idx as u8) as char
                } else {
                    (b'a' + (idx - 26) as u8) as char
                };
                return Err(Error::UndeclaredChannel(ch));
            }
        }

        Ok(())
    }

    /// Calculate note values for a chip
    fn figure_out_note_values(&mut self, clock_div: i32, note_bits: i32) {
        if clock_div == 0 {
            return;
        }
        let is_period = clock_div < 0;
        let q = clock_div.abs() as u64;
        let bits = note_bits.abs();
        let mask = (!0u64) << bits;

        let mut u = [0u64; 32];
        let mut w = 0u64;

        for i in 0..32 {
            let freq = self.note_freq[i] * self.base_freq + 0.000001;
            let v = if is_period {
                ((q as u64) << 24) / (freq as u64).max(1)
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
            self.note_value[i] = u[i] as i64;
        }
    }

    /// Calculate note length in samples
    fn calc_note_len(tempo: i32, len: i32, dots: i32) -> i64 {
        if len == 0 {
            return 0;
        }
        // 10584000 = 44100 * 60 * 4 (samples per whole note at 1 BPM)
        let mut k = 10584000i64 / len as i64;
        let mut j = k;
        for _ in 0..dots {
            j /= 2;
            k += j;
        }
        k / tempo as i64
    }

    /// Compile a single channel's MML to events
    fn compile_channel(&mut self, chan_idx: usize) -> Result<()> {
        let channel = match &self.channels[chan_idx] {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let chip_name = channel.chip_name.clone();

        // Get chip parameters first (immutable borrow)
        let (clock_div, note_bits, basic_octave) = {
            let chip_instance = match self.chips.get(&chip_name) {
                Some(c) => c,
                None => {
                    eprintln!("Warning: chip {} not found for channel", chip_name);
                    return Ok(());
                }
            };
            (chip_instance.chip.clock_div(), chip_instance.chip.note_bits(), chip_instance.chip.basic_octave())
        };

        // Calculate note values for this chip
        self.figure_out_note_values(clock_div, note_bits);

        // Initialize channel compilation state
        let mut state = ChannelCompileState::new(self.framerate);

        // Reset macro usage
        self.macro_use = [-1; MAX_MACRO_TYPES];
        self.note_off_event = 0;
        self.sample_list = -1;

        // Start channel on chip
        if let Some(chip_instance) = self.chips.get_mut(&chip_name) {
            chip_instance.chip.start_channel(chan_idx);
        }

        let text = channel.text.clone();
        let bytes = text.as_bytes();
        let mut pos = 0;

        while pos < bytes.len() {
            let b = bytes[pos];

            if b >= b'a' && b <= b'j' {
                // Note
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                let note_idx = (b - b'a') as usize;
                state.current_note = state.octave * self.octave_count + self.note_letter[note_idx] + state.transpose;
                state.current_len = state.default_len;
                pos += 1;
                self.read_note(&text, &mut pos, &mut state);
            } else if b == b'r' {
                // Rest
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                state.current_len = state.default_len;
                pos += 1;
                self.read_note(&text, &mut pos, &mut state);
                state.current_note = -1;
            } else if b == b'w' {
                // Wait (no note off)
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                state.current_len = state.default_len;
                pos += 1;
                self.read_note(&text, &mut pos, &mut state);
                state.current_note = -2;
            } else if b == b'n' {
                // Note by number
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 1;
                state.current_note = Self::read_num(&text, &mut pos) as i32 + state.transpose;
                state.current_len = state.default_len;
                self.read_note(&text, &mut pos, &mut state);
            } else if b == b'l' {
                // Set default length
                pos += 1;
                state.default_len = self.read_len(&text, &mut pos, state.tempo);
            } else if b == b'^' {
                // Tie
                pos += 1;
                let mut tie_len = state.default_len;
                let mut dummy_note = 0;
                self.read_note_params(&text, &mut pos, &mut tie_len, &mut dummy_note, state.tempo);
                state.current_len += tie_len;
            } else if b == b'&' {
                // Slur (no note off)
                pos += 1;
                state.kind |= 1;
            } else if b == b'/' {
                // Legato
                pos += 1;
                state.kind |= 2;
            } else if b == b'o' {
                // Set octave
                pos += 1;
                state.octave = Self::read_num(&text, &mut pos) as i32;
            } else if b == b'>' {
                // Octave up
                pos += 1;
                state.octave += 1;
            } else if b == b'<' {
                // Octave down
                pos += 1;
                state.octave -= 1;
            } else if b == b't' {
                // Set tempo
                pos += 1;
                state.tempo = Self::read_num(&text, &mut pos) as i32;
            } else if b == b'D' {
                // Detune
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 1;
                state.detune = Self::read_num(&text, &mut pos);
            } else if b == b'K' {
                // Transpose
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 1;
                state.transpose = Self::read_num(&text, &mut pos) as i32;
            } else if b == b'!' {
                // Stop parsing
                break;
            } else if b == b'L' {
                // Loop point
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                if let Some(ref mut ch) = self.channels[chan_idx] {
                    ch.loop_point = state.time;
                }
                self.loop_on = true;
                self.loop_point = state.time;
                pos += 1;
            } else if b == b'@' && pos + 1 < bytes.len() && bytes[pos + 1] == b'q' {
                // Quantize
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 2;
                state.quantize = Self::read_num(&text, &mut pos) * self.framerate as i64;
                state.quantize -= Self::read_num(&text, &mut pos);
            } else if b == b'[' && state.loop_depth < 127 {
                // Loop start
                state.loop_depth += 1;
                pos += 1;
                state.loop_start[state.loop_depth as usize] = pos;
                state.loop_end[state.loop_depth as usize] = 0;
                state.loop_count[state.loop_depth as usize] = 0;
            } else if b == b']' && state.loop_depth >= 0 {
                // Loop end
                let depth = state.loop_depth as usize;
                state.loop_end[depth] = pos;
                pos += 1;
                let repeat = Self::read_num(&text, &mut pos) as i32;
                state.loop_count[depth] += 1;
                if state.loop_count[depth] < repeat {
                    pos = state.loop_start[depth];
                } else {
                    state.loop_depth -= 1;
                }
            } else if b == b'\\' && state.loop_depth >= 0 {
                // Loop break
                let depth = state.loop_depth as usize;
                if state.loop_end[depth] != 0 {
                    pos = state.loop_end[depth];
                } else {
                    pos += 1;
                }
            } else if b == b'?' {
                // Conditional (channel-specific)
                pos += 1;
                if pos < bytes.len() {
                    let cond_ch = bytes[pos];
                    pos += 1;
                    let cond_idx = Self::channel_index(cond_ch as char);
                    if cond_ch != b'.' && cond_idx != Some(chan_idx) {
                        // Skip until next ?
                        while pos < bytes.len() && bytes[pos] != b'?' {
                            pos += 1;
                        }
                    }
                }
            } else if b == b'E' && pos + 3 < bytes.len()
                && bytes[pos + 1] == b'N' && bytes[pos + 2] == b'O' && bytes[pos + 3] == b'F' {
                // Arpeggio off
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 4;
                self.macro_use[MacroType::Arpeggio as usize] = -1;
            } else if b == b'E' && pos + 1 < bytes.len() && bytes[pos + 1] == b'N' {
                // Arpeggio on
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 2;
                self.macro_use[MacroType::Arpeggio as usize] = Self::read_num(&text, &mut pos) as i32;
            } else if b == b'x' {
                // Direct register write
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 1;
                let addr = Self::read_num(&text, &mut pos) as u16;
                let value = Self::read_num(&text, &mut pos) as u8;

                let chip = self.chips.get_mut(&chip_name).unwrap();
                if let Some(chip_event) = chip.chip.direct(chan_idx, addr, value) {
                    self.events.insert(Event::new(
                        state.time,
                        chan_idx as i8,
                        EventData::Chip(chip_event),
                    ));
                }
            } else if b == b'y' {
                // Raw VGM byte
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 1;
                let value = Self::read_num(&text, &mut pos) as u8;
                self.events.insert(Event::raw(state.time, value));
            } else if b == b'{' {
                // Tuplet start (2/3 length)
                pos += 1;
                state.default_len = state.default_len * 2 / 3;
            } else if b == b'}' {
                // Tuplet end (3/2 length)
                pos += 1;
                state.default_len = state.default_len * 3 / 2;
            } else if b == b'N' && pos + 2 < bytes.len()
                && bytes[pos + 1] == b'O' && bytes[pos + 2] == b'E' {
                // Note off event mode
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 3;
                self.note_off_event = Self::read_num(&text, &mut pos) as i32;
            } else if b == b'@' && pos + 1 < bytes.len() && bytes[pos + 1] == b'[' {
                // Phase sync
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 2;
                state.phase = 0;
                state.phase_count = 0;
                while pos < bytes.len() && bytes[pos] != b']' {
                    if Self::channel_index(bytes[pos] as char) == Some(chan_idx) {
                        state.phase = state.phase_count;
                    }
                    state.phase_count += 1;
                    pos += 1;
                }
                if state.phase_count > 0 {
                    state.phase_count += 1;
                }
                if pos < bytes.len() && bytes[pos] == b']' {
                    pos += 1;
                }
            } else if b == b'@' && pos + 1 < bytes.len() && bytes[pos + 1] == b'!' {
                // Fast forward
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 2;
                self.fast_forward = state.time - Self::read_num(&text, &mut pos) * self.framerate as i64;
            } else if b == b'@' && pos + 1 < bytes.len() && bytes[pos + 1] == b'w' {
                // Wait frames
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);
                pos += 2;
                let x = Self::read_num(&text, &mut pos);
                let y = Self::read_num(&text, &mut pos);
                state.time += (x * self.framerate as i64) >> y;
            } else if b == b'@' && pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                // Portamento parameters
                pos += 2;
                for i in 0..8 {
                    self.portamento[i] = Self::read_num(&text, &mut pos);
                }
            } else if b >= b'@' {
                // Macro command
                self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);

                // Extract command name
                let mut name = String::new();
                while pos < bytes.len() && bytes[pos] >= b'@' {
                    name.push(bytes[pos] as char);
                    pos += 1;
                    if name.len() >= 7 {
                        break;
                    }
                }

                let value = Self::read_num(&text, &mut pos) as i16;

                // Try to match static command
                if let Some(mac_type) = MacroType::from_stat_name(&name) {
                    self.macro_use[mac_type as usize] = -1;
                    let chip = self.chips.get_mut(&chip_name).unwrap();
                    let mac_cmd = match mac_type {
                        MacroType::Volume => MacroCommand::Volume,
                        MacroType::Panning => MacroCommand::Panning,
                        MacroType::Tone => MacroCommand::Tone,
                        MacroType::Global => MacroCommand::Global,
                        MacroType::Multiply => MacroCommand::Multiply,
                        MacroType::Waveform => MacroCommand::Waveform,
                        MacroType::ModWaveform => MacroCommand::Waveform,
                        MacroType::VolumeEnv => MacroCommand::Volume,
                        MacroType::Sample => MacroCommand::Sample,
                        MacroType::SampleList => MacroCommand::SampleList,
                        _ => MacroCommand::Volume,
                    };
                    if let Some(chip_event) = chip.chip.set_macro(chan_idx, false, mac_cmd, value) {
                        self.events.insert(Event::new(
                            state.time,
                            chan_idx as i8,
                            EventData::Chip(chip_event),
                        ));
                    }
                } else if let Some(mac_type) = MacroType::from_dyn_name(&name) {
                    self.macro_use[mac_type as usize] = (value & 255) as i32;
                }
            } else {
                // Skip unknown characters
                pos += 1;
            }
        }

        // Send final note
        self.send_note_if_pending(&mut state, chan_idx, clock_div, note_bits, basic_octave);

        // Update channel duration
        if let Some(ref mut ch) = self.channels[chan_idx] {
            ch.duration = state.time;
        }

        if self.total_samples < state.time {
            self.total_samples = state.time;
        }

        // Print channel info
        let ch_char = if chan_idx < 26 {
            (b'A' + chan_idx as u8) as char
        } else {
            (b'a' + (chan_idx - 26) as u8) as char
        };
        println!("|  {}  |  {:8}  |  {:8}  |", ch_char, state.time, self.loop_point);

        Ok(())
    }

    /// Read note length value
    fn read_len(&self, text: &str, pos: &mut usize, tempo: i32) -> i64 {
        let x = Self::read_num(text, pos) as i32;
        let mut dots = 0;
        let bytes = text.as_bytes();
        while *pos < bytes.len() && bytes[*pos] == b'.' {
            dots += 1;
            *pos += 1;
        }
        Self::calc_note_len(tempo, x, dots)
    }

    /// Read note modifiers (accidentals, length, dots)
    fn read_note(&self, text: &str, pos: &mut usize, state: &mut ChannelCompileState) {
        self.read_note_params(text, pos, &mut state.current_len, &mut state.current_note, state.tempo);
    }

    /// Read note parameters
    fn read_note_params(&self, text: &str, pos: &mut usize, len: &mut i64, note: &mut i32, tempo: i32) {
        let bytes = text.as_bytes();
        let len2 = *len;

        // Parse accidentals (if note >= 0)
        if *note >= 0 {
            while *pos < bytes.len() {
                match bytes[*pos] {
                    b'+' => {
                        *note += 1;
                        *pos += 1;
                    }
                    b'-' => {
                        *note -= 1;
                        *pos += 1;
                    }
                    b'\'' => {
                        *note += self.octave_count;
                        *pos += 1;
                    }
                    _ => break,
                }
            }
        }

        // Parse length
        let x = Self::read_num(text, pos) as i32;
        let mut dots = 0;
        while *pos < bytes.len() && bytes[*pos] == b'.' {
            dots += 1;
            *pos += 1;
        }

        if x != 0 {
            *len = Self::calc_note_len(tempo, x, dots);
        } else {
            // Just dots - extend current length
            let mut j = len2;
            for _ in 0..dots {
                j /= 2;
                *len += j;
            }
        }
    }

    /// Send pending note/rest and advance time
    fn send_note_if_pending(
        &mut self,
        state: &mut ChannelCompileState,
        chan_idx: usize,
        clock_div: i32,
        note_bits: i32,
        basic_octave: i32,
    ) {
        // Phase check
        if state.current_len > 0 {
            state.phase_counter = (state.phase_counter + 1) % state.phase_count.max(1);
            if state.phase_counter != state.phase {
                state.time += state.current_len;
                state.current_len = 0;
                state.kind <<= 2;
                return;
            }
        }

        if state.current_len == 0 {
            return;
        }

        let channel = match &self.channels[chan_idx] {
            Some(c) => c.clone(),
            None => return,
        };

        let chip_name = &channel.chip_name;

        let note = state.current_note;
        let dur = state.current_len;
        let detune = state.detune;
        let mut quantize = state.quantize;
        let kind = state.kind;

        // Slur disables quantize
        if kind & 1 != 0 {
            quantize = 0;
        }

        if note == -1 {
            // Rest
            let chip = self.chips.get_mut(chip_name).unwrap();
            if let Some(chip_event) = chip.chip.rest(chan_idx, dur as i32) {
                self.events.insert(Event::new(
                    state.time,
                    chan_idx as i8,
                    EventData::Chip(chip_event),
                ));
            }
        } else if note >= 0 {
            // Note
            let o1 = note / self.octave_count;
            let o = if note_bits < 0 {
                0
            } else if clock_div < 0 {
                o1 - basic_octave
            } else {
                basic_octave - o1
            };
            let n = (note % self.octave_count) as usize;
            let v = if clock_div != 0 {
                (self.note_value[n] >> o) - detune
            } else {
                n as i64
            };
            let d = (dur - quantize).max(0);

            // Sample list handling
            if self.sample_list != -1 {
                let sample_id = self.macro_env[MacroType::SampleList as usize][self.sample_list as usize]
                    .data.get(note as usize).copied().unwrap_or(0);
                let chip = self.chips.get_mut(chip_name).unwrap();
                if let Some(chip_event) = chip.chip.set_macro(chan_idx, true, MacroCommand::Sample, sample_id) {
                    self.events.insert(Event::new(
                        state.time,
                        chan_idx as i8,
                        EventData::Chip(chip_event),
                    ));
                }
            }

            // Note off before note on (if mode 1)
            if self.note_off_event == 1 && (kind & 12) == 0 {
                let chip = self.chips.get_mut(chip_name).unwrap();
                if let Some(chip_event) = chip.chip.note_off(chan_idx, v as i32, o1) {
                    self.events.insert(Event::new(
                        state.time,
                        chan_idx as i8,
                        EventData::Chip(chip_event),
                    ));
                }
            }

            // Note on or change
            let chip_event = {
                let chip = self.chips.get_mut(chip_name).unwrap();
                if kind & 12 != 0 {
                    chip.chip.note_change(chan_idx, v as i32, o1)
                } else {
                    chip.chip.note_on(chan_idx, v as i32, o1, d as i32)
                }
            };
            if let Some(event) = chip_event {
                self.events.insert(Event::new(
                    state.time,
                    chan_idx as i8,
                    EventData::Chip(event),
                ));
            }

            // Process macro envelopes during note
            let mut macro_indices = [0i32; MAX_MACRO_TYPES];
            let mut t = state.time;
            while t < state.time + d {
                for mac_type_idx in 0..MAX_MACRO_TYPES {
                    if self.macro_use[mac_type_idx] != -1 && macro_indices[mac_type_idx] != -1 {
                        let env_id = self.macro_use[mac_type_idx] as usize;
                        let env = &self.macro_env[mac_type_idx][env_id];
                        let idx = macro_indices[mac_type_idx] as usize;

                        if idx < env.data.len() {
                            if mac_type_idx == MacroType::Arpeggio as usize {
                                // Arpeggio modifies note pitch
                                let arp_offset = env.data[idx];
                                if arp_offset != 0 {
                                    let arp_note = note + arp_offset as i32;
                                    let arp_o1 = arp_note / self.octave_count;
                                    let arp_o = if note_bits < 0 {
                                        0
                                    } else if clock_div < 0 {
                                        arp_o1 - basic_octave
                                    } else {
                                        basic_octave - arp_o1
                                    };
                                    let arp_n = (arp_note % self.octave_count) as usize;
                                    let arp_v = if clock_div != 0 {
                                        (self.note_value[arp_n] >> arp_o) - detune
                                    } else {
                                        arp_n as i64
                                    };
                                    let chip = self.chips.get_mut(chip_name).unwrap();
                                    if let Some(event) = chip.chip.note_change(chan_idx, arp_v as i32, arp_o1) {
                                        self.events.insert(Event::new(t, chan_idx as i8, EventData::Chip(event)));
                                    }
                                }
                            } else {
                                // Other macros
                                let value = env.data[idx];
                                let mac_cmd = match MacroType::all().nth(mac_type_idx).unwrap() {
                                    MacroType::Volume => MacroCommand::Volume,
                                    MacroType::Panning => MacroCommand::Panning,
                                    MacroType::Tone => MacroCommand::Tone,
                                    MacroType::Option => MacroCommand::Option,
                                    MacroType::Multiply => MacroCommand::Multiply,
                                    MacroType::Waveform => MacroCommand::Waveform,
                                    MacroType::Sample => MacroCommand::Sample,
                                    _ => continue,
                                };
                                let chip = self.chips.get_mut(chip_name).unwrap();
                                if let Some(event) = chip.chip.set_macro(chan_idx, true, mac_cmd, value) {
                                    self.events.insert(Event::new(t, chan_idx as i8, EventData::Chip(event)));
                                }
                            }

                            // Advance macro index
                            macro_indices[mac_type_idx] += 1;
                            let new_idx = macro_indices[mac_type_idx];
                            if new_idx >= env.loop_end {
                                macro_indices[mac_type_idx] = env.loop_start;
                            }
                        }
                    }
                }
                t += self.framerate as i64;
            }

            // Note off after note (if mode 0)
            if self.note_off_event == 0 && (kind & 3) == 0 {
                let chip = self.chips.get_mut(chip_name).unwrap();
                if let Some(chip_event) = chip.chip.note_off(chan_idx, v as i32, o1) {
                    self.events.insert(Event::new(
                        state.time + d,
                        chan_idx as i8,
                        EventData::Chip(chip_event),
                    ));
                }
            }

            state.old_note = note;
        }

        state.time += state.current_len;
        state.current_len = 0;
        state.kind <<= 2;
    }

    /// Write output to VGM file
    fn write_output(&mut self, writer: &mut VgmWriter) -> Result<()> {
        // Write header placeholder
        writer.write_header()?;

        // Begin file for all chips
        for (_, instance) in &mut self.chips {
            instance.chip.file_begin(writer);
        }

        // Output events
        let mut current_time = 0i64;
        let events: Vec<Event> = self.events.iter().cloned().collect();

        for event in &events {
            // Handle loop point
            if self.loop_on && self.loop_point >= current_time && self.loop_point <= event.time {
                let delay = (self.loop_point - current_time) as u64;
                if delay > 0 {
                    writer.write_delay(delay)?;
                }
                writer.mark_loop_start();
                current_time = self.loop_point;

                // Notify chips of loop start
                for (_, instance) in &mut self.chips {
                    instance.chip.loop_start(writer);
                }
                self.loop_on = false;
            }

            // Write delay
            let delay = (event.time - current_time) as u64;
            if delay > 0 {
                writer.write_delay(delay)?;
            }
            current_time = event.time;

            // Write event
            match &event.data {
                EventData::Raw(byte) => {
                    writer.write_byte(*byte)?;
                }
                EventData::Chip(chip_event) => {
                    let chan_idx = event.channel as usize;
                    if let Some(channel) = &self.channels[chan_idx] {
                        let chip_name = &channel.chip_name;
                        if let Some(instance) = self.chips.get_mut(chip_name) {
                            instance.chip.send_with_macro_env(
                                chip_event,
                                chan_idx,
                                channel.chip_sub,
                                channel.chan_sub,
                                writer,
                                &self.macro_env,
                            );
                        }
                    }
                }
            }
        }

        // Write final delay
        let final_delay = (self.total_samples - current_time) as u64;
        if final_delay > 0 {
            writer.write_delay(final_delay)?;
        }

        // End file for all chips
        for (_, instance) in &mut self.chips {
            instance.chip.file_end(writer);
        }

        // Set header values
        writer.set_total_samples((self.total_samples - self.fast_forward) as u32);
        writer.set_loop_samples((self.total_samples - self.fast_forward - self.loop_point) as u32);
        writer.set_rate(self.recording_rate as u32);
        writer.set_volume_modifier(if self.volume_mod == -64 { -63 } else { self.volume_mod as i8 });
        writer.set_loop_base(self.loop_base);
        writer.set_loop_modifier(self.loop_mod);

        // Generate GD3 metadata
        let metadata = crate::compiler::Gd3Metadata {
            title_en: self.gd3_text[gd3::TITLE_EN].clone(),
            title_jp: self.gd3_text[gd3::TITLE_JP].clone(),
            game_en: self.gd3_text[gd3::GAME_EN].clone(),
            game_jp: self.gd3_text[gd3::GAME_JP].clone(),
            system_en: self.gd3_text[gd3::SYSTEM_EN].clone(),
            system_jp: self.gd3_text[gd3::SYSTEM_JP].clone(),
            composer_en: self.gd3_text[gd3::COMPOSER_EN].clone(),
            composer_jp: self.gd3_text[gd3::COMPOSER_JP].clone(),
            date: self.gd3_text[gd3::DATE].clone(),
            converter: self.gd3_text[gd3::CONVERTER].clone(),
            notes: self.gd3_text[gd3::NOTES].clone(),
        };

        writer.finalize(&metadata)?;

        Ok(())
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Channel compile state (local to parse_music)
struct ChannelCompileState {
    octave: i32,
    tempo: i32,
    default_len: i64,
    time: i64,
    transpose: i32,
    detune: i64,
    quantize: i64,
    current_note: i32,
    current_len: i64,
    kind: u8,
    old_note: i32,
    loop_depth: i32,
    loop_start: [usize; 128],
    loop_end: [usize; 128],
    loop_count: [i32; 128],
    phase: i32,
    phase_count: i32,
    phase_counter: i32,
}

impl ChannelCompileState {
    fn new(framerate: i32) -> Self {
        let _ = framerate;
        Self {
            octave: 0,
            tempo: 120,
            default_len: Compiler::calc_note_len(120, 4, 0),
            time: 0,
            transpose: 0,
            detune: 0,
            quantize: 0,
            current_note: -1,
            current_len: 0,
            kind: 0,
            old_note: 0,
            loop_depth: -1,
            loop_start: [0; 128],
            loop_end: [0; 128],
            loop_count: [0; 128],
            phase: 0,
            phase_count: 1,
            phase_counter: 0,
        }
    }
}

/// GD3 metadata
#[derive(Debug, Default)]
pub struct Gd3Metadata {
    pub title_en: String,
    pub title_jp: String,
    pub game_en: String,
    pub game_jp: String,
    pub system_en: String,
    pub system_jp: String,
    pub composer_en: String,
    pub composer_jp: String,
    pub date: String,
    pub converter: String,
    pub notes: String,
}

/// Convert channel character to index
pub fn channel_index(ch: char) -> Result<usize> {
    Compiler::channel_index(ch).ok_or(Error::InvalidChannel(ch))
}

/// Convert index to channel character
pub fn index_to_channel(idx: usize) -> Option<char> {
    match idx {
        0..=25 => Some((b'A' + idx as u8) as char),
        26..=51 => Some((b'a' + (idx - 26) as u8) as char),
        _ => None,
    }
}
