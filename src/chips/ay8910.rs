//! AY-3-8910 sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// AY-3-8910 chip
pub struct Ay8910 {
    clock: i32,
    ena: [u8; 2],    // Enable register state per chip
    vol: u8,         // Current volume
    dual: i32,       // Dual chip mode
    spec: bool,      // Special (envelope) channel used
    mul: i32,        // Envelope multiplier
    opt_s: i32,      // S option (envelope octave shift)
    opt_t: u8,       // T option (type)
    opt_l: bool,     // l option (legacy)
    opt_s_flag: bool, // s option
    opt_d_flag: bool, // d option
    opt_r_flag: bool, // r option
}

impl Ay8910 {
    pub fn new() -> Self {
        Self {
            clock: 1789750,
            ena: [0; 2],
            vol: 15,
            dual: 0,
            spec: false,
            mul: 0,
            opt_s: 1,
            opt_t: 0,
            opt_l: true,
            opt_s_flag: false,
            opt_d_flag: false,
            opt_r_flag: false,
        }
    }

    fn poke(&self, address: u8, data: u8, writer: &mut VgmWriter) {
        let _ = writer.write_data(&[0xA0, address, data]);
    }
}

impl Default for Ay8910 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Ay8910 {
    fn name(&self) -> &'static str {
        "GI-AY"
    }

    fn chip_id(&self) -> u8 {
        chip_id::AY8910
    }

    fn clock_div(&self) -> i32 {
        -self.clock
    }

    fn note_bits(&self) -> i32 {
        12
    }

    fn basic_octave(&self) -> i32 {
        1
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 1789750;
        }
        self.opt_s = options.get('S');
        if self.opt_s == 0 {
            self.opt_s = 1;
        }
        self.opt_t = options.get('T') as u8;
        self.opt_l = options.get('l') != 0;
        self.opt_s_flag = options.get('s') != 0;
        self.opt_d_flag = options.get('d') != 0;
        self.opt_r_flag = options.get('r') != 0;
    }

    fn file_begin(&mut self, _writer: &mut VgmWriter) {
        self.ena = [0; 2];
        let spec_val = if self.spec { 1 } else { 0 };
        self.dual = if self.dual > 2 - spec_val { 1 } else { 0 };
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual != 0 {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::AY8910_CLOCK, clock_val);
        header.write_u8(offset::AY8910_TYPE, self.opt_t);

        let flags = (self.opt_l as u8)
            | ((self.opt_s_flag as u8) << 1)
            | ((self.opt_d_flag as u8) << 2)
            | ((self.opt_r_flag as u8) << 3);
        header.write_u8(offset::AY8910_FLAGS, flags);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {}

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        self.mul = 0;
        self.vol = 15;
        if chip_sub != 0 {
            self.spec = true;
        }
        if (chan_sub as i32) > self.dual {
            self.dual = chan_sub as i32;
        }
        if chip_sub != 0 && chan_sub != 0 {
            self.dual = 6;
        }
    }

    fn set_macro(
        &mut self,
        _channel: usize,
        is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent> {
        match command {
            MacroCommand::Volume => {
                if is_dynamic && self.vol == (value as u8) {
                    return None;
                }
                self.vol = (value & 15) as u8;
                // event_type 0x21 = volume, value1 = volume, value2 = env shape (0 = none)
                Some(ChipEvent::new(0x21, self.vol as i32, 0))
            }
            MacroCommand::Tone => {
                // event_type 0x22 = tone/enable control
                Some(ChipEvent::new(0x22, value as i32, 0))
            }
            MacroCommand::Multiply => {
                self.vol = 0x1F;
                self.mul = value as i32;
                None
            }
            MacroCommand::VolumeEnv => {
                self.vol = 0x1F;
                let env_shape = if value > 0 { 13 } else { 9 };
                self.mul = (value as i32).abs() * if value > 0 { -1 } else { 1 };
                Some(ChipEvent::new(0x21, self.vol as i32, env_shape))
            }
            MacroCommand::Sample => {
                // Noise period register
                Some(ChipEvent::new(0x06, value as i32, 0))
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
        // event_type 0x20 = key on/off
        // value1 = note/period, value2 = volume | (envelope_period << 8)
        Some(ChipEvent::new(0x20, note, (self.vol as i32) | (self.mul << 16)))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x20, note, (self.vol as i32) | (self.mul << 16)))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x20, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x20, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let a = chip_sub;
        let b = chan_sub;
        let spec_val = if self.spec { 1 } else { 0 };
        let c = ((a & b) | (b > 2 - spec_val) as usize) as u8;
        let d = if a != 0 { 2 } else { (b % (3 - spec_val)) as u8 };

        match event.event_type {
            0x20 => {
                // Key on/off
                let note = event.value1 as u16;
                let vol = (event.value2 & 0xFF) as u8;
                let env_period = ((event.value2 >> 16) as i32).unsigned_abs() as u16;

                if a != 0 {
                    // Special channel - envelope mode
                    self.poke(11 | (c << 7), (env_period & 0xFF) as u8, writer);
                    self.poke(12 | (c << 7), (env_period >> 8) as u8, writer);
                }
                self.poke(d | (c << 7) | 8, vol, writer);
                self.poke((d << 1) | (c << 7), (note & 0xFF) as u8, writer);
                self.poke((d << 1) | (c << 7) | 1, (note >> 8) as u8, writer);
            }
            0x21 => {
                // Volume
                let vol = event.value1 as u8;
                let env_shape = event.value2 as u8;
                self.poke(d | (c << 7) | 8, vol, writer);
                if a != 0 && env_shape != 0 {
                    self.poke(13 | (c << 7), env_shape, writer);
                }
            }
            0x22 => {
                // Tone enable control
                let val = event.value1 as u8;
                self.ena[c as usize] &= !(9 << d);
                self.ena[c as usize] |= ((val & 1) | ((val & 2) << 2)) << d;
                self.poke(7 | (c << 7), self.ena[c as usize], writer);
                if a != 0 {
                    self.poke(13 | (c << 7), (val >> 2) | 8, writer);
                }
            }
            _ => {
                // Direct register write
                self.poke((event.event_type as u8) ^ (c << 7), event.value1 as u8, writer);
            }
        }
    }
}
