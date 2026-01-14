//! YM2413 (OPLL) sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// YM2413 OPLL chip
pub struct Opll {
    clock: i32,
    dual: i32,           // Dual chip tracking
    drum: bool,          // Rhythm mode enabled
    sus: u8,             // Sustain mode
    mem: [[i16; 64]; 2], // Register memory cache
}

impl Opll {
    pub fn new() -> Self {
        Self {
            clock: 3579545,
            dual: 0,
            drum: false,
            sus: 0,
            mem: [[256; 64]; 2],
        }
    }

    /// Write to OPLL register with caching
    fn opll_put(&mut self, chip: usize, address: usize, mask: u8, data: u8, writer: &mut VgmWriter) {
        let actual_chip = if (address & 0x80) != 0 {
            (address >> 6) & 1
        } else {
            chip
        };
        let addr = address & 0x3F;
        let combined = (data as i16) | (self.mem[actual_chip][addr] & (mask as i16));

        if self.mem[actual_chip][addr] == combined {
            return;
        }

        let cmd = if actual_chip != 0 { 0xA1 } else { 0x51 };
        let _ = writer.write_data(&[cmd, addr as u8, combined as u8]);
        self.mem[actual_chip][addr] = combined;
    }
}

impl Default for Opll {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Opll {
    fn name(&self) -> &'static str {
        "OPLL"
    }

    fn chip_id(&self) -> u8 {
        chip_id::YM2413
    }

    fn clock_div(&self) -> i32 {
        self.clock / 9
    }

    fn note_bits(&self) -> i32 {
        -9
    }

    fn basic_octave(&self) -> i32 {
        7
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 3579545;
        }
    }

    fn file_begin(&mut self, writer: &mut VgmWriter) {
        // Reset memory
        self.mem = [[256; 64]; 2];

        // Determine dual mode
        let dual_val = if self.drum && self.dual >= 6 {
            6
        } else if self.dual >= 9 {
            9
        } else {
            127
        };
        self.dual = dual_val;

        // Initialize rhythm register if not drum mode
        if !self.drum {
            self.opll_put(0, 0x0E, 0x00, 0x00, writer);
            if dual_val != 127 {
                self.opll_put(1, 0x0E, 0x00, 0x00, writer);
            }
        }
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual != 127 {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::YM2413_CLOCK, clock_val);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {}

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        self.sus = (chip_sub as u8) << 5;
        if chip_sub != 0 {
            self.drum = true;
        }
        if (chan_sub as i32) > self.dual {
            self.dual = chan_sub as i32;
        }
    }

    fn set_macro(
        &mut self,
        _channel: usize,
        _is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent> {
        match command {
            MacroCommand::Volume => {
                // Volume command - event_type encodes register, value2 is mask, value1 is data
                Some(ChipEvent::new(0xF3, (0x0F & !value) as i32, 0xF0))
            }
            MacroCommand::Tone | MacroCommand::Sample => {
                // Tone/instrument select
                if (value & !0x1F) != 0 {
                    self.sus = 0;
                    None
                } else {
                    self.sus = (value & 0x10) as u8;
                    Some(ChipEvent::new(0xF3, ((value & 15) << 4) as i32, 0x0F))
                }
            }
            _ => None,
        }
    }

    fn note_on(
        &mut self,
        _channel: usize,
        note: i32,
        octave: i32,
        _duration: i32,
    ) -> Option<ChipEvent> {
        // For melody mode: event_type = 0xFF
        // value1 = low byte of note, value2 = high byte | octave | key-on
        let actual_note = if (self.sus & !0x1F) != 0 {
            (self.sus >> 5) as i32
        } else {
            note
        };
        Some(ChipEvent::new(
            0xFF,
            actual_note & 0xFF,
            ((actual_note >> 8) & 1) | (octave << 1) | 0x10,
        ))
    }

    fn note_change(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        let actual_note = if (self.sus & !0x1F) != 0 {
            (self.sus >> 5) as i32
        } else {
            note
        };
        Some(ChipEvent::new(
            0xFF,
            actual_note & 0xFF,
            ((actual_note >> 8) & 1) | (octave << 1) | 0x10,
        ))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // Melody note off: clear key-on bit
        Some(ChipEvent::new(0xF2, 0x00, 0xEF))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0xF2, 0x00, 0xEF))
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let b = chip_sub;
        let dual_val = self.dual.max(1) as usize;
        let c = (b & chan_sub) | (chan_sub >= dual_val) as usize;
        let d = chan_sub % dual_val;

        match event.event_type {
            0xF0..=0xF7 => {
                // Command selecting a register of this channel
                let x = ((event.event_type & 7) as usize) << 4;
                let mask = event.value2 as u8;
                let data = event.value1 as u8;
                if b != 0 {
                    // Rhythm mode - write to channels 6, 7, 8
                    self.opll_put(c, x | 6, mask, data, writer);
                    self.opll_put(c, x | 7, mask, data, writer);
                    self.opll_put(c, x | 8, mask, data, writer);
                } else {
                    self.opll_put(c, x | d, mask, data, writer);
                }
            }
            0xFD => {
                // Custom instrument select - needs macro env data
                // This is handled in send_with_macro_env
            }
            0xFE => {
                // Rhythm note on
                let freq_low = event.value1 as u8;
                let freq_high = event.value2 as u8;
                let sus_val = self.sus & 0x1F;
                self.opll_put(c, 0x16, 0, freq_low, writer);
                self.opll_put(c, 0x17, 0, freq_low, writer);
                self.opll_put(c, 0x18, 0, freq_low, writer);
                self.opll_put(c, 0x26, 0, freq_high, writer);
                self.opll_put(c, 0x27, 0, freq_high, writer);
                self.opll_put(c, 0x28, 0, freq_high, writer);
                self.opll_put(c, 0x0E, 0x20, sus_val, writer);
            }
            0xFF => {
                // Melody note on
                let freq_low = event.value1 as u8;
                let freq_high = event.value2 as u8;
                self.opll_put(c, 0x10 | d, 0, freq_low, writer);
                self.opll_put(c, 0x20 | d, 0, freq_high, writer);
            }
            _ => {
                // Direct register write
                self.opll_put(c, event.event_type as usize, event.value2 as u8, event.value1 as u8, writer);
            }
        }
    }

    fn send_with_macro_env(
        &mut self,
        event: &ChipEvent,
        channel: usize,
        chip_sub: usize,
        chan_sub: usize,
        writer: &mut VgmWriter,
        macro_env: &MacroEnvStorage,
    ) {
        let b = chip_sub;
        let dual_val = self.dual.max(1) as usize;
        let c = (b & chan_sub) | (chan_sub >= dual_val) as usize;
        let d = chan_sub % dual_val;

        if event.event_type == 0xFD {
            // Custom instrument select
            let idx = (event.value1 as usize).min(255);
            let inst_data = &macro_env[3][idx].data; // MC_Option = 3
            for x in 0..8 {
                let val = inst_data.get(x).copied().unwrap_or(0) as u8;
                self.opll_put(c, x, 0, val, writer);
            }
            self.opll_put(c, 0x30 | d, 0x0F, 0x00, writer);
        } else {
            self.send(event, channel, chip_sub, chan_sub, writer);
        }
    }
}
