//! Pokey (Atari) sound chip driver
//!
//! 4 channels with various modes:
//! - chip_sub=0: Normal 8-bit mode
//! - chip_sub=1: 16-bit mode (channels 0+1 or 2+3)
//! - chip_sub=2: High-pass filter mode

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// Pokey chip (Atari)
pub struct Pokey {
    clock: i32,
    opt_c: i32,          // Clock select option
    opt_p: i32,          // Poly counter select option
    opt_x: bool,         // Direct multiply mode
    audctl: u8,          // AUDCTL register value
    audc: u8,            // Current volume/distortion
    mul: i16,            // Filter multiplier
    stat: [[u8; 4]; 3],  // Channel state [chip_sub][chan_sub]
    ass: [u8; 4],        // Channel address assignment
}

impl Pokey {
    pub fn new() -> Self {
        Self {
            clock: 1789773,
            opt_c: 0,
            opt_p: 0,
            opt_x: false,
            audctl: 0,
            audc: 0,
            mul: 0,
            stat: [[0x10; 4]; 3],
            ass: [0, 2, 4, 6],
        }
    }

    fn poke(&self, address: u8, data: u8, writer: &mut VgmWriter) {
        let _ = writer.write_data(&[0xBB, address, data]);
    }
}

impl Default for Pokey {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Pokey {
    fn name(&self) -> &'static str {
        "Pokey"
    }

    fn chip_id(&self) -> u8 {
        chip_id::POKEY
    }

    fn clock_div(&self) -> i32 {
        // Note: this is modified per-channel in the C version
        // For now, use the base clock divided
        let divisor = if self.opt_c != 0 { 114 } else { 28 };
        -self.clock / divisor
    }

    fn note_bits(&self) -> i32 {
        8 // Can be 16 for chip_sub=1
    }

    fn basic_octave(&self) -> i32 {
        2
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 1789773;
        }
        self.opt_c = options.get('c');
        self.opt_p = options.get('p');
        self.opt_x = options.get('x') != 0;
        self.audctl = (self.opt_c | (self.opt_p << 7)) as u8;
    }

    fn file_begin(&mut self, writer: &mut VgmWriter) {
        // Reset state
        self.stat = [[0x10; 4]; 3];
        self.ass = [0, 2, 4, 6];
        self.audc = 0;
        self.mul = 0;

        // Initialize all channels
        self.poke(0, 0x00, writer);
        self.poke(1, 0xF0, writer);
        self.poke(2, 0x00, writer);
        self.poke(3, 0xF0, writer);
        self.poke(4, 0x00, writer);
        self.poke(5, 0xF0, writer);
        self.poke(6, 0x00, writer);
        self.poke(7, 0xF0, writer);
        self.poke(8, self.audctl, writer);
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        header.write_u32(offset::POKEY_CLOCK, self.clock as u32);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {
        self.audc = 0;
    }

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        self.audc = 0;
        self.stat[chip_sub][chan_sub] = 0x10;

        // Adjust assignments and audctl based on mode
        if chip_sub == 1 {
            // 16-bit mode
            self.ass[0] = 4;
            self.ass[1] = 6;
            self.audctl |= 0x10 >> chan_sub;
        } else if chip_sub == 2 {
            // High-pass filter mode
            self.ass[0] = 2;
            self.ass[1] = 6;
            self.audctl |= 0x04 >> chan_sub;
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
                if (self.audc & 0x0F) != (value as u8 & 0x0F) {
                    self.audc = (self.audc & 0xE0) | (value as u8 & 0x0F);
                    // type 0xFD = volume/distortion
                    Some(ChipEvent::new(0xFD, self.audc as i32, 0))
                } else {
                    None
                }
            }
            MacroCommand::Tone => {
                if (self.audc >> 5) != (value as u8 & 0x07) {
                    self.audc = (self.audc & 0x0F) | ((value as u8 & 0x07) << 5);
                    Some(ChipEvent::new(0xFD, self.audc as i32, 0))
                } else {
                    None
                }
            }
            MacroCommand::Multiply => {
                self.mul = value;
                None
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
        // type 0xFE = key on
        // value1 = note, value2 = filter mod value (for chip_sub=2)
        Some(ChipEvent::new(0xFE, note, self.mul as i32))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0xFE, note, self.mul as i32))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // type 0xFF = key off
        Some(ChipEvent::new(0xFF, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        None
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let c = chip_sub;
        let d = chan_sub;

        // Calculate register address based on mode
        let a = if c != 0 {
            (d << (c ^ 3)) as u8
        } else {
            self.ass[d]
        };

        match event.event_type {
            0xFD => {
                // Volume/distortion setting
                self.stat[c][d] &= 0x10;
                self.stat[c][d] |= event.value1 as u8 & 0xEF;
                if (self.stat[c][d] & 0x10) == 0 {
                    self.poke(a | 1, self.stat[c][d], writer);
                }
            }
            0xFE => {
                // Key on
                let mut note = event.value1;

                // Adjust note value based on mode
                if c == 1 {
                    note -= 7;
                    // Swap bytes for 16-bit mode
                    note = ((note & 0xFF) << 8) | ((note >> 8) & 0xFF);
                } else {
                    note -= 1;
                }

                self.poke(a, (note & 0xFF) as u8, writer);

                if c == 1 {
                    // 16-bit mode: write high byte
                    self.poke(a | 2, ((note >> 8) & 0xFF) as u8, writer);
                }

                if c == 2 {
                    // Filter mode: calculate and write filter value
                    let mul = event.value2;
                    let mut filter_val = if mul > 0 {
                        event.value1 * mul - 1
                    } else if mul < 0 {
                        (event.value1 / (-mul)) - 1
                    } else {
                        0x40
                    };
                    if filter_val < 0 {
                        filter_val = 0;
                    }
                    if filter_val > 255 {
                        filter_val = 255;
                    }
                    self.poke(a | 4, filter_val as u8, writer);
                }

                // Turn on volume if muted
                if (self.stat[c][d] & 0x10) != 0 {
                    self.stat[c][d] &= 0xEF;
                    self.poke(a | 1, self.stat[c][d], writer);
                }
            }
            0xFF => {
                // Key off
                if (self.stat[c][d] & 0x10) != 0 {
                    return; // Already off
                }
                self.stat[c][d] |= 0x10;
                self.poke(a | 1, 0xF0, writer);
                self.poke(a, 0x00, writer);
                if c == 1 {
                    self.poke(a | 2, 0x00, writer);
                }
            }
            _ => {
                // Direct register write
                self.poke(event.event_type as u8, event.value1 as u8, writer);
            }
        }
    }
}
