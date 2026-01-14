//! NES APU (2A03) sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// NES APU (2A03) chip
pub struct NesApu {
    clock: i32,
    enable: [u8; 2],       // Channel enable state per chip
    dutyvol: [[u8; 2]; 2], // Duty/volume for square channels
    dual: bool,            // Dual chip mode
}

impl NesApu {
    pub fn new() -> Self {
        Self {
            clock: 1789772,
            enable: [0, 0],
            dutyvol: [[0x30, 0x30], [0x30, 0x30]],
            dual: false,
        }
    }
}

impl Default for NesApu {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for NesApu {
    fn name(&self) -> &'static str {
        "FAMICOM"
    }

    fn chip_id(&self) -> u8 {
        chip_id::NES_APU
    }

    fn clock_div(&self) -> i32 {
        -self.clock
    }

    fn note_bits(&self) -> i32 {
        11
    }

    fn basic_octave(&self) -> i32 {
        2
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 1789772;
        }
    }

    fn file_begin(&mut self, _writer: &mut VgmWriter) {
        self.enable = [0, 0];
        self.dutyvol = [[0x30, 0x30], [0x30, 0x30]];
        self.dual = false;
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::NES_APU_CLOCK, clock_val);
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
            MacroCommand::Volume => {
                // Packed: address in event_type (negative = internal), value/mask in value1/value2
                // For square channels: set volume (lower nibble)
                // event_type = 0xFFFF - 3 = duty/volume command
                Some(ChipEvent::new(0xFFFD, value as i32, 0xF0))
            }
            MacroCommand::Tone => {
                // Duty cycle select for square channels
                Some(ChipEvent::new(0xFFFD, (value << 6) as i32, 0x3F))
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
        // event_type 0xFFFF = note on, value1 = period, value2 = channel mask
        Some(ChipEvent::new(0xFFFF, note, octave))
    }

    fn note_change(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0xFFFE, note, octave))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // 0xFFFC = note off
        Some(ChipEvent::new(0xFFFC, 0, 0))
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
            0xFFFC => {
                // Note off
                let mask = 0x1F ^ (1 << d);
                self.enable[c] &= mask as u8;
                let _ = writer.write_data(&[0xB4, ((c << 7) | 0x15) as u8, self.enable[c]]);
            }
            0xFFFD => {
                // Duty/volume for square channels
                let mask = event.value2 as u8;
                let val = event.value1 as u8;
                self.dutyvol[c][b & 1] = (self.dutyvol[c][b & 1] & mask) | val;
                let _ = writer.write_data(&[0xB4, ((c << 7) | ((b & 1) << 2)) as u8, self.dutyvol[c][b & 1]]);
            }
            0xFFFE => {
                // Note change
                let period = if a == 2 {
                    (event.value1 as u16) | ((event.value2 as u16) << 7)
                } else {
                    (event.value1 - 1) as u16
                };
                let _ = writer.write_data(&[0xB4, ((c << 7) | (d << 2) | 2) as u8, (period & 0xFF) as u8]);
                let _ = writer.write_data(&[0xB4, ((c << 7) | (d << 2) | 3) as u8, ((period >> 8) | 0xF8) as u8]);
            }
            0xFFFF => {
                // Note on
                let channel_bit = 1u8 << d;

                // For triangle channel (a==1), write 0xFF to linear counter
                if a == 1 {
                    let _ = writer.write_data(&[0xB4, ((c << 7) | 0x08) as u8, 0xFF]);
                }

                // Enable channel
                self.enable[c] |= channel_bit;
                let _ = writer.write_data(&[0xB4, ((c << 7) | 0x15) as u8, self.enable[c]]);

                // Write period
                let period = if a == 2 {
                    (event.value1 as u16) | ((event.value2 as u16) << 7)
                } else {
                    (event.value1 - 1) as u16
                };
                let _ = writer.write_data(&[0xB4, ((c << 7) | (d << 2) | 2) as u8, (period & 0xFF) as u8]);
                let _ = writer.write_data(&[0xB4, ((c << 7) | (d << 2) | 3) as u8, ((period >> 8) | 0xF8) as u8]);
            }
            _ => {
                // Direct register write
                let _ = writer.write_data(&[0xB4, event.event_type as u8, event.value1 as u8]);
            }
        }
    }
}
