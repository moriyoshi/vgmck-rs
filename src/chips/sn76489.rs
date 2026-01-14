//! SN76489 (PSG) sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// SN76489 PSG chip
pub struct Sn76489 {
    clock: i32,
    feedback: u8,
    shift_width: u8,
    #[allow(dead_code)]
    flags: u8,
    // State tracking for optimization
    stereo: [u8; 2],
    dual: bool,
    vol: [[i32; 4]; 2],
    tone: [[i64; 4]; 2],
    noteon: [[bool; 4]; 2],
    ltone: [i32; 2],
    // Options
    flag_f: bool,
    flag_n: bool,
    flag_s: bool,
    flag_d: bool,
}

impl Sn76489 {
    pub fn new() -> Self {
        Self {
            clock: 3579545,
            feedback: 9,
            shift_width: 16,
            flags: 0,
            stereo: [0xFF, 0xFF],
            dual: false,
            vol: [[-1; 4]; 2],
            tone: [[-1; 4]; 2],
            noteon: [[false; 4]; 2],
            ltone: [-1, -1],
            flag_f: false,
            flag_n: false,
            flag_s: true,
            flag_d: true,
        }
    }
}

impl Default for Sn76489 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Sn76489 {
    fn name(&self) -> &'static str {
        "PSG"
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
            self.clock = 3579545;
        }
        self.feedback = options.get('F') as u8;
        if self.feedback == 0 {
            self.feedback = 9;
        }
        self.shift_width = options.get('S') as u8;
        if self.shift_width == 0 {
            self.shift_width = 16;
        }
        self.flag_f = options.get('f') != 0;
        self.flag_n = options.get('n') != 0;
        self.flag_s = options.get('s') == 0; // inverted in original
        self.flag_d = options.get('d') == 0; // inverted in original
    }

    fn file_begin(&mut self, _writer: &mut VgmWriter) {
        // Reset state
        for i in 0..2 {
            for j in 0..4 {
                self.vol[i][j] = -1;
                self.tone[i][j] = -1;
                self.noteon[i][j] = false;
            }
            self.stereo[i] = 0xFF;
            self.ltone[i] = -1;
        }
        self.dual = false;
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::SN76489_CLOCK, clock_val);
        header.write_u8(offset::SN76489_FEEDBACK, self.feedback);
        header.write_u8(offset::SN76489_SHIFT_WIDTH, self.shift_width);

        // Build flags byte
        let flags = (self.flag_f as u8)
            | ((self.flag_n as u8) << 1)
            | ((self.flag_s as u8) << 2)
            | ((self.flag_d as u8) << 3);
        header.write_u8(offset::SN76489_FLAGS, flags);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {
        // Nothing special needed
    }

    fn start_channel(&mut self, _channel: usize) {
        // Nothing special needed
    }

    fn set_macro(
        &mut self,
        _channel: usize,
        _is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent> {
        match command {
            MacroCommand::Volume => Some(ChipEvent::new(2, value as i32, 0)),
            MacroCommand::Panning => Some(ChipEvent::new(1, value as i32, 0)),
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
        Some(ChipEvent::new(3, note, 0))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(3, note, 0))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(4, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(4, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, _value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(0, address as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {

        // c = which chip (0 or 1 for dual), d = which channel on chip (0-3)
        let c = if chip_sub > 0 {
            chan_sub >= 1
        } else {
            chan_sub >= 3
        } as usize;
        let d = if chip_sub > 0 { 3 } else { chan_sub % 3 };

        // Track if dual chip mode is used
        if c > 0 {
            self.dual = true;
        }

        // Bounds check
        if chip_sub > 1 || chan_sub > 5 {
            return;
        }

        let cmd_byte = if c > 0 { 0x30u8 } else { 0x50u8 };

        match event.event_type {
            0 => {
                // Direct write
                let _ = writer.write_data(&[cmd_byte, event.value1 as u8]);
                self.ltone[c] = -1;
            }
            1 => {
                // Stereo (GG)
                let mut x = self.stereo[c];
                x &= !(0x11 << d);
                if event.value1 <= 0 {
                    x |= 0x10 << d;
                }
                if event.value1 >= 0 {
                    x |= 0x01 << d;
                }
                if x != self.stereo[c] {
                    let stereo_cmd = if c > 0 { 0x3F } else { 0x4F };
                    let _ = writer.write_data(&[stereo_cmd, event.value1 as u8]);
                    self.stereo[c] = x;
                    self.ltone[c] = -1;
                }
            }
            2 => {
                // Volume
                let x = event.value1;
                if self.noteon[c][d] && x != self.vol[c][d] {
                    let _ = writer.write_data(&[cmd_byte, (0x9F ^ (x as u8)) | ((d as u8) << 5)]);
                    self.ltone[c] = -1;
                }
                self.vol[c][d] = x;
            }
            3 => {
                // Note on/change
                let note = event.value1 as i64;

                // If volume is set but note not on, send volume first
                if self.vol[c][d] > 0 && !self.noteon[c][d] {
                    let _ = writer.write_data(&[cmd_byte, (0x9F ^ (self.vol[c][d] as u8)) | ((d as u8) << 5)]);
                    self.ltone[c] = -1;
                }
                self.noteon[c][d] = true;

                // Send tone if low nibble changed or different channel
                if ((note ^ self.tone[c][d]) & 15) != 0 || (note != self.tone[c][d] && self.ltone[c] != d as i32) {
                    let _ = writer.write_data(&[cmd_byte, 0x80 | ((note as u8) & 0x0F) | ((d as u8) << 5)]);
                    self.ltone[c] = d as i32;
                }

                // Send high bits if same channel and not noise channel
                if self.ltone[c] == d as i32 && chip_sub == 0 {
                    let _ = writer.write_data(&[cmd_byte, ((note >> 4) & 0x3F) as u8]);
                }

                self.tone[c][d] = note;
            }
            4 => {
                // Note off
                if self.noteon[c][d] && self.vol[c][d] > 0 {
                    let _ = writer.write_data(&[cmd_byte, 0x9F | ((d as u8) << 5)]);
                    self.ltone[c] = -1;
                }
                self.noteon[c][d] = false;
            }
            _ => {}
        }
    }
}
