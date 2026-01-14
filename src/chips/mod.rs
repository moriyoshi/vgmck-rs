//! Sound chip drivers

pub mod ay8910;
pub mod ay8930;
pub mod dmg;
pub mod huc6280;
pub mod nes_apu;
pub mod opl2;
pub mod opl3;
pub mod opl4;
pub mod opll;
pub mod opn2;
pub mod pokey;
pub mod qsound;
pub mod sn76489;
pub mod t6w28;

use crate::compiler::event::ChipEvent;
use crate::error::{Error, Result};
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::VgmWriter;
use std::collections::HashMap;

/// Chip ID constants (matching VGM spec)
pub mod chip_id {
    pub const SN76489: u8 = 0;
    pub const YM2413: u8 = 1;
    pub const YM2612: u8 = 2;
    pub const YM2151: u8 = 3;
    pub const SEGA_PCM: u8 = 4;
    pub const RF5C68: u8 = 5;
    pub const YM2203: u8 = 6;
    pub const YM2608: u8 = 7;
    pub const YM2610: u8 = 8;
    pub const YM3812: u8 = 9;
    pub const YM3526: u8 = 10;
    pub const Y8950: u8 = 11;
    pub const YMF262: u8 = 12;
    pub const YMF278B: u8 = 13;
    pub const YMF271: u8 = 14;
    pub const YMZ280B: u8 = 15;
    pub const RF5C164: u8 = 16;
    pub const PWM: u8 = 17;
    pub const AY8910: u8 = 18;
    pub const GB_DMG: u8 = 19;
    pub const NES_APU: u8 = 20;
    pub const MULTI_PCM: u8 = 21;
    pub const UPD7759: u8 = 22;
    pub const OKIM6258: u8 = 23;
    pub const OKIM6295: u8 = 24;
    pub const K051649: u8 = 25;
    pub const K054539: u8 = 26;
    pub const HUC6280: u8 = 27;
    pub const C140: u8 = 28;
    pub const K053260: u8 = 29;
    pub const POKEY: u8 = 30;
    pub const QSOUND: u8 = 31;
}

/// Macro command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroCommand {
    Volume = 0,
    Panning = 1,
    Tone = 2,
    Option = 3,
    Arpeggio = 4,
    Global = 5,
    Multiply = 6,
    Waveform = 7,
    ModWaveform = 8,
    VolumeEnv = 9,
    Sample = 10,
    SampleList = 11,
    Midi = 12,
}

/// Chip configuration options
#[derive(Debug, Clone, Default)]
pub struct ChipOptions {
    pub values: HashMap<char, i32>,
}

impl ChipOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: char) -> i32 {
        self.values.get(&key).copied().unwrap_or(0)
    }

    pub fn set(&mut self, key: char, value: i32) {
        self.values.insert(key, value);
    }
}

/// Sound chip trait
pub trait SoundChip: Send + Sync {
    /// Get chip name (e.g., "PSG", "OPN2")
    fn name(&self) -> &'static str;

    /// Get chip ID
    fn chip_id(&self) -> u8;

    /// Get clock divisor (negative for period-based, positive for frequency-based)
    fn clock_div(&self) -> i32;

    /// Get note bits (negative to not shift by octave)
    fn note_bits(&self) -> i32;

    /// Get basic octave number
    fn basic_octave(&self) -> i32;

    /// Enable chip with options
    fn enable(&mut self, options: &ChipOptions);

    /// Called at start of file output
    fn file_begin(&mut self, writer: &mut VgmWriter);

    /// Called at end of file output
    fn file_end(&mut self, writer: &mut VgmWriter);

    /// Called at loop start point
    fn loop_start(&mut self, writer: &mut VgmWriter);

    /// Called when starting a channel
    fn start_channel(&mut self, channel: usize);

    /// Called when starting a channel with chip_sub/chan_sub info
    fn start_channel_with_info(&mut self, _chip_sub: usize, _chan_sub: usize) {
        // Default: do nothing
    }

    /// Set a macro value
    fn set_macro(
        &mut self,
        channel: usize,
        is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent>;

    /// Note on event
    fn note_on(
        &mut self,
        channel: usize,
        note: i32,
        octave: i32,
        duration: i32,
    ) -> Option<ChipEvent>;

    /// Note change (pitch bend/portamento)
    fn note_change(&mut self, channel: usize, note: i32, octave: i32) -> Option<ChipEvent>;

    /// Note off event
    fn note_off(&mut self, channel: usize, note: i32, octave: i32) -> Option<ChipEvent>;

    /// Rest event
    fn rest(&mut self, channel: usize, duration: i32) -> Option<ChipEvent>;

    /// Direct register write
    fn direct(&mut self, channel: usize, address: u16, value: u8) -> Option<ChipEvent>;

    /// Send event to VGM writer
    fn send(&mut self, event: &ChipEvent, channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter);

    /// Send event with macro envelope access (for chips that need it)
    fn send_with_macro_env(
        &mut self,
        event: &ChipEvent,
        channel: usize,
        chip_sub: usize,
        chan_sub: usize,
        writer: &mut VgmWriter,
        _macro_env: &MacroEnvStorage,
    ) {
        // Default: just call regular send
        self.send(event, channel, chip_sub, chan_sub, writer);
    }
}

/// Chip instance wrapper
pub struct ChipInstance {
    pub chip: Box<dyn SoundChip>,
    pub options: ChipOptions,
}

impl ChipInstance {
    pub fn new(chip: Box<dyn SoundChip>) -> Self {
        Self {
            chip,
            options: ChipOptions::new(),
        }
    }
}

/// Create a chip instance by name
pub fn create_chip(name: &str) -> Result<ChipInstance> {
    let chip: Box<dyn SoundChip> = match name {
        "PSG" => Box::new(sn76489::Sn76489::new()),
        "OPN2" => Box::new(opn2::Opn2::new()),
        "OPLL" => Box::new(opll::Opll::new()),
        "OPL2" => Box::new(opl2::Opl2::new()),
        "OPL3" => Box::new(opl3::Opl3::new()),
        "OPL4" => Box::new(opl4::Opl4::new()),
        "AY8910" | "GI-AY" => Box::new(ay8910::Ay8910::new()),
        "AY8930" => Box::new(ay8930::Ay8930::new()),
        "2A03" | "FAMICOM" => Box::new(nes_apu::NesApu::new()),
        "DMG" | "GAMEBOY" => Box::new(dmg::Dmg::new()),
        "HuC6280" => Box::new(huc6280::HuC6280::new()),
        "Pokey" => Box::new(pokey::Pokey::new()),
        "QSound" => Box::new(qsound::QSound::new()),
        "T6W28" => Box::new(t6w28::T6w28::new()),
        _ => return Err(Error::UnknownChip(name.to_string())),
    };

    Ok(ChipInstance::new(chip))
}

/// List all available chip names
pub fn list_chips() -> Vec<&'static str> {
    vec![
        "PSG", "OPN2", "OPLL", "OPL2", "OPL3", "OPL4", "AY8910", "AY8930", "2A03", "DMG",
        "HuC6280", "Pokey", "QSound", "T6W28",
    ]
}
