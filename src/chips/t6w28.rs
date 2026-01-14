//! T6W28 sound chip driver
//!
//! Similar to SN76489 but with stereo output (separate L/R channels)
//! Used in Neo Geo Pocket

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// T6W28 chip (similar to SN76489, with stereo)
pub struct T6w28 {
    clock: i32,
    opt_f: i32,           // Feedback register
    opt_s: i32,           // Shift register width
    opt_flags: u8,        // Various flags
    vol: [i32; 4],        // Volume per channel
    pan: [i32; 4],        // Panning per channel
    tone: [i32; 4],       // Tone period per channel
    noteon: [bool; 4],    // Key state per channel
    noise: i32,           // Noise mode
}

impl T6w28 {
    pub fn new() -> Self {
        Self {
            clock: 3072000,
            opt_f: 9,
            opt_s: 16,
            opt_flags: 0,
            vol: [-1; 4],
            pan: [0; 4],
            tone: [0; 4],
            noteon: [false; 4],
            noise: -1,
        }
    }
}

impl Default for T6w28 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for T6w28 {
    fn name(&self) -> &'static str {
        "T6W28"
    }

    fn chip_id(&self) -> u8 {
        chip_id::SN76489
    }

    fn clock_div(&self) -> i32 {
        -self.clock
    }

    fn note_bits(&self) -> i32 {
        10
    }

    fn basic_octave(&self) -> i32 {
        0
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 3072000;
        }
        self.opt_f = options.get('F');
        if self.opt_f == 0 {
            self.opt_f = 9;
        }
        self.opt_s = options.get('S');
        if self.opt_s == 0 {
            self.opt_s = 16;
        }

        // Build flags
        let opt_freq_0 = options.get('f') != 0;
        let opt_neg_lfsr = options.get('n') != 0;
        let opt_sw_neg = options.get('s') == 0;
        let opt_disable_freq_reg3 = options.get('d') == 0;
        self.opt_flags = (opt_freq_0 as u8)
            | ((opt_neg_lfsr as u8) << 1)
            | ((opt_sw_neg as u8) << 2)
            | ((opt_disable_freq_reg3 as u8) << 3);
    }

    fn file_begin(&mut self, _writer: &mut VgmWriter) {
        // Reset state
        self.vol = [-1; 4];
        self.pan = [0; 4];
        self.tone = [0; 4];
        self.noteon = [false; 4];
        self.noise = -1;
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        // T6W28 uses SN76489 clock with 0xC0 flag (bit 6 and 7 set)
        header.write_u32(offset::SN76489_CLOCK, self.clock as u32 | 0xC0000000);
        header.write_u8(offset::SN76489_FEEDBACK, self.opt_f as u8);
        header.write_u8(offset::SN76489_SHIFT_WIDTH, self.opt_s as u8);
        header.write_u8(offset::SN76489_FLAGS, self.opt_flags);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {}

    fn set_macro(
        &mut self,
        _channel: usize,
        _is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent> {
        match command {
            MacroCommand::Tone => {
                // type 5 = noise mode
                Some(ChipEvent::new(5, value as i32, 0))
            }
            MacroCommand::Volume => {
                // type 2 = volume
                Some(ChipEvent::new(2, value as i32, 0))
            }
            MacroCommand::Panning => {
                // type 1 = stereo/panning
                Some(ChipEvent::new(1, value as i32, 0))
            }
            _ => None,
        }
    }

    fn note_on(
        &mut self,
        _channel: usize,
        note: i32,
        _octave: i32,
        _duration: i32,
    ) -> Option<ChipEvent> {
        // type 3 = note on/change
        Some(ChipEvent::new(3, note, 0))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(3, note, 0))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // type 4 = note off
        Some(ChipEvent::new(4, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(4, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, _value: u8) -> Option<ChipEvent> {
        // type 0 = direct write
        Some(ChipEvent::new(0, address as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let a = chip_sub;
        let b = chan_sub;
        let c = if a != 0 { 3 } else { b & 3 };
        let v = event.value1;

        match event.event_type {
            0 => {
                // Direct write
                let cmd = if a != 0 { 0x30 } else { 0x50 };
                let _ = writer.write_data(&[cmd, v as u8]);
            }
            1 => {
                // Stereo/panning
                self.pan[c] = v;
                if self.noteon[c] {
                    self.write_volume(c, writer);
                }
            }
            2 => {
                // Volume
                self.vol[c] = v;
                if self.noteon[c] {
                    self.write_volume(c, writer);
                }
            }
            3 => {
                // Note on/change
                if self.tone[c] != v {
                    self.tone[c] = v;
                    let cmd = if a != 0 { 0x30 } else { 0x50 };
                    let latch = (self.tone[c] & 0x0F) as u8
                        | 0x80
                        | if a != 0 { 0xC0 } else { (c << 5) as u8 };
                    let _ = writer.write_data(&[cmd, latch]);
                    let _ = writer.write_data(&[cmd, (self.tone[c] >> 4) as u8]);
                }

                if !self.noteon[c] {
                    self.noteon[c] = true;
                    self.write_volume(c, writer);
                }
            }
            4 => {
                // Note off
                if self.noteon[c] {
                    self.noteon[c] = false;
                    // Left channel off
                    let _ = writer.write_data(&[0x50, 0x9F | ((c << 5) as u8)]);
                    // Right channel off
                    let _ = writer.write_data(&[0x30, 0x9F | ((c << 5) as u8)]);
                }
            }
            5 => {
                // Noise mode (for chip_sub != 0)
                if a != 0 && self.noise != v {
                    self.noise = v;
                    let _ = writer.write_data(&[0x30, 0xE3 | ((v << 2) as u8)]);
                }
            }
            _ => {}
        }
    }
}

impl T6w28 {
    fn write_volume(&self, c: usize, writer: &mut VgmWriter) {
        // Left channel
        let left_vol = self.vol[c] - if self.pan[c] > 0 { self.pan[c] } else { 0 };
        let left_atten = if left_vol < 0 { 0 } else { left_vol };
        let _ = writer.write_data(&[0x50, (0x9F ^ left_atten as u8) | ((c << 5) as u8)]);

        // Right channel
        let right_vol = self.vol[c] + if self.pan[c] < 0 { self.pan[c] } else { 0 };
        let right_atten = if right_vol < 0 { 0 } else { right_vol };
        let _ = writer.write_data(&[0x30, (0x9F ^ right_atten as u8) | ((c << 5) as u8)]);
    }
}
