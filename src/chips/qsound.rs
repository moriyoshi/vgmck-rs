//! QSound sound chip driver
//!
//! 16-channel sample playback chip used by Capcom

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// QSound chip (Capcom)
pub struct QSound {
    clock: i32,
    vol: [i32; 16],      // Volume per channel
    key: [bool; 16],     // Key state per channel
    per: [bool; 16],     // Periodic/fixed pitch mode
    mru_sam: i32,        // Most recently used sample
}

impl QSound {
    pub fn new() -> Self {
        Self {
            clock: 4000000,
            vol: [0; 16],
            key: [false; 16],
            per: [false; 16],
            mru_sam: -1,
        }
    }

    fn qs_write(&self, address: u8, data: u16, writer: &mut VgmWriter) {
        let _ = writer.write_data(&[
            0xC4,
            (data >> 8) as u8,
            (data & 0xFF) as u8,
            address,
        ]);
    }
}

impl Default for QSound {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for QSound {
    fn name(&self) -> &'static str {
        "QSound"
    }

    fn chip_id(&self) -> u8 {
        chip_id::QSOUND
    }

    fn clock_div(&self) -> i32 {
        self.clock
    }

    fn note_bits(&self) -> i32 {
        16
    }

    fn basic_octave(&self) -> i32 {
        7
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 4000000;
        }
    }

    fn file_begin(&mut self, _writer: &mut VgmWriter) {
        // Reset state
        self.vol = [0; 16];
        self.key = [false; 16];
        self.per = [false; 16];
        self.mru_sam = -1;
        // Note: Sample data blocks would be written here if sample loading was implemented
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        header.write_u32(offset::QSOUND_CLOCK, self.clock as u32);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {
        self.mru_sam = -1;
    }

    fn set_macro(
        &mut self,
        _channel: usize,
        _is_dynamic: bool,
        command: MacroCommand,
        value: i16,
    ) -> Option<ChipEvent> {
        match command {
            MacroCommand::Sample => {
                // type 0xFFFC = sample select (negated: ~0)
                Some(ChipEvent::new(0xFFFC, value as i32, 0))
            }
            MacroCommand::Volume => {
                // type 0xFFFD = volume (negated: ~1)
                Some(ChipEvent::new(0xFFFD, value as i32, 0))
            }
            MacroCommand::Panning => {
                // Panning - direct register write
                // value2 will be set based on chan_sub in send()
                Some(ChipEvent::new(0xFFFB, value as i32 + 0x0120, 0))
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
        // type 0xFFFE = key on (negated: ~3)
        Some(ChipEvent::new(0xFFF9, note, 0))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        // type 0xFFFF = pitch change (negated: ~4)
        Some(ChipEvent::new(0xFFF8, note, 0))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // type 0xFFFA = key off (negated: ~2)
        Some(ChipEvent::new(0xFFFA, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        // Same as note off
        Some(ChipEvent::new(0xFFFA, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, _chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let ch = chan_sub;

        match event.event_type {
            0xFFFC => {
                // Sample select
                // Note: Full implementation would load sample data here
                // For now, just set up minimal state
                self.mru_sam = event.value1;
                // Simplified sample setup without actual sample loading
                self.per[ch] = false;
            }
            0xFFFD => {
                // Volume
                if self.vol[ch] != event.value1 {
                    self.vol[ch] = event.value1;
                    if self.key[ch] {
                        self.qs_write((ch << 3 | 6) as u8, event.value1 as u16, writer);
                    }
                }
            }
            0xFFFB => {
                // Panning
                self.qs_write((ch | 0x80) as u8, event.value1 as u16, writer);
            }
            0xFFFA => {
                // Key off
                if self.key[ch] {
                    self.qs_write((ch << 3 | 6) as u8, 0, writer);
                }
                self.key[ch] = false;
            }
            0xFFF9 => {
                // Key on
                if self.per[ch] {
                    self.qs_write((ch << 3 | 4) as u8, event.value1 as u16, writer);
                    self.qs_write((ch << 3 | 5) as u8, event.value1 as u16, writer);
                } else {
                    self.qs_write((ch << 3 | 2) as u8, event.value1 as u16, writer);
                }
                self.qs_write((ch << 3 | 6) as u8, self.vol[ch] as u16, writer);
                self.key[ch] = true;
            }
            0xFFF8 => {
                // Pitch change
                if self.per[ch] {
                    self.qs_write((ch << 3 | 4) as u8, event.value1 as u16, writer);
                    self.qs_write((ch << 3 | 5) as u8, event.value1 as u16, writer);
                } else {
                    self.qs_write((ch << 3 | 2) as u8, event.value1 as u16, writer);
                }
            }
            _ => {
                // Direct register write
                self.qs_write(event.event_type as u8, event.value1 as u16, writer);
            }
        }
    }
}
