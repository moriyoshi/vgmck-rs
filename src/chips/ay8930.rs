//! AY8930 sound chip driver
//!
//! Enhanced AY-3-8910 with 16-bit tone periods and extended envelopes

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// Enhanced envelope high byte registers
const ENH: [u8; 6] = [0x0B, 0x10, 0x12, 0x8B, 0x90, 0x92];

/// Enhanced envelope mode registers
const ENM: [u8; 6] = [0x0D, 0x14, 0x15, 0x8D, 0x94, 0x95];

/// AY8930 chip (enhanced AY-3-8910)
pub struct Ay8930 {
    clock: i32,
    opt_s: i32,          // S option (envelope octave shift)
    opt_flags: u8,       // Various flags (l, s, d, r)
    ena: [u8; 2],        // Enable register per chip
    enva: [u8; 2],       // Envelope amplitude register per chip
    bank: [u8; 2],       // Current bank per chip
    vol: u8,             // Current volume
    mul: i32,            // Envelope multiplier
    dual: bool,          // Dual chip mode
}

impl Ay8930 {
    pub fn new() -> Self {
        Self {
            clock: 1789750,
            opt_s: 0,
            opt_flags: 1, // l=1 by default
            ena: [0; 2],
            enva: [0; 2],
            bank: [0; 2],
            vol: 31,
            mul: 0,
            dual: false,
        }
    }

    fn poke(&mut self, address: u8, data: u8, writer: &mut VgmWriter) {
        let mut data = data;
        let chip = (address >> 7) as usize;

        // Handle register 13 (envelope shape/mode)
        if (address & 15) == 13 {
            data |= (address & 0x10) | 0xA0;
            self.enva[chip] = data;
        } else if self.bank[chip] != (address & 0x10) {
            // Need to switch bank
            let _ = writer.write_data(&[0xA0, 0x0D | (address & 0x8D), self.enva[chip]]);
        }

        self.bank[chip] = address & 0x10;
        let _ = writer.write_data(&[0xA0, address & 0x8F, data]);
    }
}

impl Default for Ay8930 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Ay8930 {
    fn name(&self) -> &'static str {
        "AY8930"
    }

    fn chip_id(&self) -> u8 {
        chip_id::AY8910
    }

    fn clock_div(&self) -> i32 {
        -self.clock
    }

    fn note_bits(&self) -> i32 {
        16
    }

    fn basic_octave(&self) -> i32 {
        0
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 1789750;
        }
        self.opt_s = options.get('S');

        // Build flags
        let opt_l = (options.get('l') != 0) as u8;
        let opt_s_flag = (options.get('s') != 0) as u8;
        let opt_d = (options.get('d') != 0) as u8;
        let opt_r = (options.get('r') != 0) as u8;
        self.opt_flags = opt_l | (opt_s_flag << 1) | (opt_d << 2) | (opt_r << 3);
    }

    fn file_begin(&mut self, writer: &mut VgmWriter) {
        // Reset state
        self.ena = [0; 2];
        self.enva = [0; 2];
        self.bank = [0; 2];
        self.vol = 31;
        self.mul = 0;

        // Initialize with extended mode enable
        self.poke(0x0D, 0xA0, writer);
        if self.dual {
            self.poke(0x8D, 0xA0, writer);
        }
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::AY8910_CLOCK, clock_val);
        header.write_u8(offset::AY8910_TYPE, 3); // Type 3 = AY8930
        header.write_u8(offset::AY8910_FLAGS, self.opt_flags);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {
        self.mul = 0;
        self.vol = 31;
    }

    fn start_channel_with_info(&mut self, _chip_sub: usize, chan_sub: usize) {
        self.mul = 0;
        self.vol = 31;
        if chan_sub / 3 != 0 {
            self.dual = true;
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
                if is_dynamic && self.vol == (value as u8 & 31) {
                    return None;
                }
                self.vol = (value & 31) as u8;
                // type 0x21 = volume
                Some(ChipEvent::new(0x21, self.vol as i32, 0))
            }
            MacroCommand::Tone => {
                // type 0x22 = tone/enable control
                Some(ChipEvent::new(0x22, value as i32, 0))
            }
            MacroCommand::Multiply => {
                self.vol = 0x3F;
                self.mul = value as i32;
                None
            }
            MacroCommand::VolumeEnv => {
                self.vol = 0x3F;
                let env_shape = if value > 0 { 13 } else { 9 };
                self.mul = (value as i32).abs() * if value > 0 { -1 } else { 1 };
                // value2 = envelope shape
                Some(ChipEvent::new(0x21, self.vol as i32, env_shape))
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
        // type 0x20 = key on
        // value1 = note, value2 = envelope period
        let mut note_val = note;
        if self.opt_s < 0 {
            note_val >>= -self.opt_s;
        }

        let env_period = if self.mul > 0 {
            let mut ep = (note * self.mul) >> 6;
            if self.opt_s > 0 {
                ep >>= self.opt_s;
            }
            ep as u16
        } else {
            (-self.mul) as u16
        };

        // Pack: event_type=0x20, value1=note|(vol<<16), value2=env_period
        Some(ChipEvent::new(0x20, note_val | ((self.vol as i32) << 16), env_period as i32))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        let mut note_val = note;
        if self.opt_s < 0 {
            note_val >>= -self.opt_s;
        }

        let env_period = if self.mul > 0 {
            let mut ep = (note * self.mul) >> 6;
            if self.opt_s > 0 {
                ep >>= self.opt_s;
            }
            ep as u16
        } else {
            (-self.mul) as u16
        };

        Some(ChipEvent::new(0x20, note_val | ((self.vol as i32) << 16), env_period as i32))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // Note off: volume=0, note=0, env=0
        Some(ChipEvent::new(0x20, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x20, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, _chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let b = chan_sub;
        let c = b / 3;
        let d = b % 3;

        match event.event_type {
            0x20 => {
                // Key on/off
                let note = (event.value1 & 0xFFFF) as u16;
                let vol = ((event.value1 >> 16) & 0xFF) as u8;
                let env_period = event.value2 as u16;

                // Write envelope period high
                self.poke(ENH[b], (env_period & 0xFF) as u8, writer);
                self.poke(ENH[b] + 1, (env_period >> 8) as u8, writer);

                // Write volume
                self.poke((d | (c << 7) | 8) as u8, vol, writer);

                // Write tone period
                self.poke(((d << 1) | (c << 7)) as u8, (note & 0xFF) as u8, writer);
                self.poke(((d << 1) | (c << 7) | 1) as u8, (note >> 8) as u8, writer);
            }
            0x21 => {
                // Volume
                self.poke((d | (c << 7) | 8) as u8, event.value1 as u8, writer);
                if event.value2 != 0 {
                    self.poke(ENM[b], event.value2 as u8, writer);
                }
            }
            0x22 => {
                // Tone/enable control
                let val = event.value1 as u8;
                self.ena[c] &= !(9 << d);
                self.ena[c] |= ((val & 1) | ((val & 2) << 2)) << d;
                self.poke((7 | (c << 7)) as u8, self.ena[c], writer);
                self.poke(ENM[b], ((val >> 2) | 8) & 15, writer);
                self.poke(((c << 7) | 0x16 + d) as u8, val >> 5, writer);
            }
            _ => {
                // Direct register write - write to both chips
                self.poke(event.event_type as u8, event.value1 as u8, writer);
                self.poke(event.event_type as u8 | 0x80, event.value1 as u8, writer);
            }
        }
    }
}
