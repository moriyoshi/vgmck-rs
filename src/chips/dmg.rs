//! GameBoy DMG sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// Noise frequency table
const NOISE_TABLE: [u8; 16] = [1, 9, 2, 10, 3, 5, 13, 6, 14, 7, 15, 11, 4, 8, 12, 0];

/// GameBoy DMG chip
pub struct Dmg {
    clock: i32,
    dual: bool,
    pan: [u8; 2],
    vol: u8,
}

impl Dmg {
    pub fn new() -> Self {
        Self {
            clock: 4194304,
            dual: false,
            pan: [0xFF, 0xFF],
            vol: 0xF0,
        }
    }
}

impl Default for Dmg {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Dmg {
    fn name(&self) -> &'static str {
        "GAMEBOY"
    }

    fn chip_id(&self) -> u8 {
        chip_id::GB_DMG
    }

    fn clock_div(&self) -> i32 {
        -self.clock
    }

    fn note_bits(&self) -> i32 {
        11
    }

    fn basic_octave(&self) -> i32 {
        1
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 4194304;
        }
    }

    fn file_begin(&mut self, writer: &mut VgmWriter) {
        self.pan = [0xFF, 0xFF];
        self.vol = 0xF0;

        // Initialize sound system
        let _ = writer.write_data(&[0xB3, 0x16, 0xFF]); // NR52 - Master control
        let _ = writer.write_data(&[0xB3, 0x14, 0x77]); // NR50 - Master volume
        let _ = writer.write_data(&[0xB3, 0x15, 0xFF]); // NR51 - Panning

        if self.dual {
            let _ = writer.write_data(&[0xB3, 0x96, 0xFF]); // Second chip NR52
            let _ = writer.write_data(&[0xB3, 0x94, 0x77]); // Second chip NR50
            let _ = writer.write_data(&[0xB3, 0x95, 0xFF]); // Second chip NR51
        }
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::GB_DMG_CLOCK, clock_val);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {
        self.vol = 0xF0;
    }

    fn set_macro(
        &mut self,
        _channel: usize,
        _is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent> {
        match command {
            MacroCommand::Panning => {
                // event_type 0xFFF0 = stereo command
                Some(ChipEvent::new(0xFFF0, value as i32, 0))
            }
            MacroCommand::Volume => {
                // For wave channel (chip_sub==1), volume is different
                // event_type 0xFFF1 = volume (handled in send based on chip_sub)
                let new_vol = (self.vol & 0x0F) | ((value as u8) << 4);
                self.vol = new_vol;
                Some(ChipEvent::new(0xFFF1, value as i32, 0))
            }
            MacroCommand::VolumeEnv => {
                // Volume envelope
                let env_val = if value <= 0 {
                    (-value) as u8
                } else {
                    (value as u8) | 8
                };
                self.vol = (self.vol & 0xF0) | env_val;
                None
            }
            MacroCommand::Waveform => {
                // Wave table select - needs macro env access
                Some(ChipEvent::new(0xFFF2, value as i32, 0))
            }
            MacroCommand::Tone => {
                // Duty cycle for square channels
                Some(ChipEvent::new(0xFFF3, value as i32, 0))
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
        // event_type 0xFFF4 = note on
        // value1 = note period, value2 = volume | flags
        Some(ChipEvent::new(0xFFF4, note, (self.vol as i32) | (octave << 8)))
    }

    fn note_change(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        // event_type 0xFFF5 = note change
        Some(ChipEvent::new(0xFFF5, note, octave))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // event_type 0xFFF6 = note off
        Some(ChipEvent::new(0xFFF6, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        None
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let a = chip_sub;
        let b = chan_sub;
        let c = (b > (a == 0) as usize) as usize;
        let d = a + (b & (a == 0) as usize) + (a != 0) as usize;

        if c > 0 {
            self.dual = true;
        }

        match event.event_type {
            0xFFF0 => {
                // Stereo/panning
                let mask = (0x11 << d) as u8;
                let val = event.value1;
                let period = if val < 0 {
                    0x10 << d
                } else if val > 0 {
                    0x01 << d
                } else {
                    0x11 << d
                } as u8;
                self.pan[c] = (self.pan[c] & !mask) | period;
                let _ = writer.write_data(&[0xB3, ((c << 7) | 0x15) as u8, self.pan[c]]);
            }
            0xFFF1 => {
                // Volume for wave channel
                if a == 1 {
                    let vol = event.value1 as u8;
                    self.vol = vol;
                    let _ = writer.write_data(&[0xB3, ((c << 7) | 0x0C) as u8, (4 - vol) << 5]);
                }
            }
            0xFFF2 => {
                // Wave table - needs macro env (handled in send_with_macro_env)
            }
            0xFFF3 => {
                // Duty cycle
                let duty = (event.value1 << 6) as u8;
                let _ = writer.write_data(&[0xB3, ((c << 7) | (b * 5 + 1)) as u8, duty]);
            }
            0xFFF4 => {
                // Note on
                let mut note = event.value1;
                let vol = (event.value2 & 0xFF) as u8;
                let octave = (event.value2 >> 8) as i32;

                // For noise channel, convert to DMG format
                if a == 2 {
                    note = (NOISE_TABLE[(note & 15) as usize] as i32) | (((15 - octave) as i32) << 4);
                }

                let period = (note ^ 0x7FF) as u16;
                let vol_reg = vol | if a == 1 { 0x80 } else { 0 };

                // Write volume/envelope register
                let _ = writer.write_data(&[0xB3, ((c << 7) | (d * 5 + 2 * (a != 1) as usize)) as u8, vol_reg]);

                // Write period low
                let _ = writer.write_data(&[0xB3, ((c << 7) | (d * 5 + 3)) as u8, (period & 0xFF) as u8]);

                // Write period high with trigger bit
                let _ = writer.write_data(&[0xB3, ((c << 7) | (d * 5 + 4)) as u8, ((period >> 8) | 0x80) as u8]);
            }
            0xFFF5 => {
                // Note change
                let mut note = event.value1;
                let octave = event.value2;

                if a == 2 {
                    // Noise channel - direct write to register
                    note = (NOISE_TABLE[(note & 15) as usize] as i32) | (((15 - octave) as i32) << 4);
                    let _ = writer.write_data(&[0xB3, ((c << 7) | 0x12) as u8, note as u8]);
                } else {
                    let period = (note ^ 0x7FF) as u16;
                    let _ = writer.write_data(&[0xB3, ((c << 7) | (d * 5 + 3)) as u8, (period & 0xFF) as u8]);
                    let _ = writer.write_data(&[0xB3, ((c << 7) | (d * 5 + 4)) as u8, (period >> 8) as u8]);
                }
            }
            0xFFF6 => {
                // Note off
                let reg = if a == 1 { 0x0A } else { d * 5 + 2 };
                let _ = writer.write_data(&[0xB3, ((c << 7) | reg) as u8, 0x00]);
            }
            _ => {
                // Direct register write
                let _ = writer.write_data(&[0xB3, event.event_type as u8, event.value1 as u8]);
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
        let c = ((chan_sub > (chip_sub == 0) as usize) as usize) as u8;

        if event.event_type == 0xFFF2 {
            // Wave table write
            let idx = (event.value1 as usize).min(255);
            let wave_data = &macro_env[7][idx].data; // MC_Waveform = 7

            for i in 0..16usize {
                let high = wave_data.get(i * 2).copied().unwrap_or(0) as u8;
                let low = wave_data.get(i * 2 + 1).copied().unwrap_or(0) as u8;
                let byte = (high << 4) | (low & 0x0F);
                let _ = writer.write_data(&[0xB3, (c << 7) | 0x20 | (i as u8), byte]);
            }
        } else {
            self.send(event, channel, chip_sub, chan_sub, writer);
        }
    }
}
