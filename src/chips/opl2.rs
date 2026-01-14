//! YM3812 (OPL2) sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// Operator offset table
const OPER: [usize; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];

/// YM3812 OPL2 chip
pub struct Opl2 {
    clock: i32,
    memory: [[i16; 256]; 2],
    dual: usize,
    subc: [usize; 2],
    instr: [[usize; 18]; 6],
    vol: [[i32; 18]; 6],
}

impl Opl2 {
    pub fn new() -> Self {
        Self {
            clock: 3579545,
            memory: [[-1; 256]; 2],
            dual: 0,
            subc: [0, 0],
            instr: [[0; 18]; 6],
            vol: [[0; 18]; 6],
        }
    }

    fn write_opl(&mut self, chip: usize, address: usize, value: u8, writer: &mut VgmWriter) {
        if self.memory[chip][address] != value as i16 {
            self.memory[chip][address] = value as i16;
            let cmd = if chip != 0 { 0xAA } else { 0x5A };
            let _ = writer.write_data(&[cmd, address as u8, value]);
        }
    }

    fn set_opl(&mut self, chip: usize, address: usize, mask: u8, set: u8, writer: &mut VgmWriter) {
        let current = self.memory[chip][address].max(0) as u8;
        let value = (current & !mask) | (set & mask);
        self.write_opl(chip, address, value, writer);
    }

    fn set_instrument(
        &mut self,
        c: usize,
        ch: usize,
        o: usize,
        inst_data: &[i16],
        v: i32,
        writer: &mut VgmWriter,
    ) {
        let s = ((o & 7) / 3) as usize; // is second operator
        let bd_mode = (self.memory[c][0xBD].max(0) as u8 & 0x20) != 0;
        let h = (bd_mode && o > 16) || ((inst_data.get(10).copied().unwrap_or(0) & 1) != 0) || s != 0;

        let mut vol = v + (inst_data.get(s | 2).copied().unwrap_or(0) as i32 & 0x3F);
        if vol > 63 {
            vol = 63;
        }

        self.write_opl(c, o | 0x20, inst_data.get(s).copied().unwrap_or(0) as u8, writer);
        let tl = if h {
            ((inst_data.get(s | 2).copied().unwrap_or(0) as u8) & 0xC0) | (vol as u8)
        } else {
            inst_data.get(s | 2).copied().unwrap_or(0) as u8
        };
        self.write_opl(c, o | 0x40, tl, writer);
        self.write_opl(c, o | 0x60, inst_data.get(s | 4).copied().unwrap_or(0) as u8, writer);
        self.write_opl(c, o | 0x80, inst_data.get(s | 6).copied().unwrap_or(0) as u8, writer);
        self.write_opl(c, o | 0xE0, inst_data.get(s | 8).copied().unwrap_or(0) as u8, writer);
        if s == 0 {
            self.write_opl(c, ch | 0xC0, inst_data.get(10).copied().unwrap_or(0) as u8, writer);
        }
    }
}

impl Default for Opl2 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Opl2 {
    fn name(&self) -> &'static str {
        "OPL2"
    }

    fn chip_id(&self) -> u8 {
        chip_id::YM3812
    }

    fn clock_div(&self) -> i32 {
        self.clock / 9
    }

    fn note_bits(&self) -> i32 {
        -10
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
        self.memory = [[-1; 256]; 2];

        // Determine dual mode
        self.dual = if self.subc[1] == 2 { 6 } else { 9 };
        if self.subc[1] < 2 && self.subc[0] <= self.dual {
            self.dual = 0;
        }

        // Initialize chips
        let chip_count = if self.dual != 0 { 2 } else { 1 };
        for i in 0..chip_count {
            self.write_opl(i, 0x01, 0x20, writer); // Waveform select enable
            self.write_opl(i, 0x08, 0x00, writer); // CSM/Keyboard split

            // Clear all registers
            for j in 0x20..0xB9 {
                self.write_opl(i, j, 0x00, writer);
            }

            // Set rhythm mode if needed
            let rhythm = if self.subc[1] > i { 0x20 } else { 0x00 };
            self.write_opl(i, 0xBD, rhythm, writer);
        }
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual != 0 {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::YM3812_CLOCK, clock_val);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {
        // Invalidate frequency registers at loop point
        for i in 0xA0..0xB0 {
            self.memory[0][i] = -1;
            self.memory[1][i] = -1;
        }
    }

    fn start_channel(&mut self, _channel: usize) {}

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        let x = if chip_sub != 0 { 1 } else { 0 };
        if self.subc[x] <= chan_sub {
            self.subc[x] = chan_sub + 1;
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
                // type 4 = volume
                Some(ChipEvent::new(4, (63 & !value) as i32, 0))
            }
            MacroCommand::Tone => {
                // type 3 = instrument
                Some(ChipEvent::new(3, (value & 255) as i32, 0))
            }
            MacroCommand::Global => {
                // type 5 = global (tremolo/vibrato depth)
                let data1 = ((value & 3) << 6) as i32;
                let data2 = ((value & 12) << 4) as i32;
                Some(ChipEvent::new(5, data1, data2))
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
        // type 1 = note on
        let data1 = note & 255;
        let data2 = ((note >> 8) & 3) | (octave << 2) | 0x20; // Key-on bit set
        Some(ChipEvent::new(1, data1, data2))
    }

    fn note_change(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        let data1 = note & 255;
        let data2 = ((note >> 8) & 3) | (octave << 2) | 0x20;
        Some(ChipEvent::new(1, data1, data2))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(2, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(2, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(0, address as i32, value as i32))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let a = chip_sub;
        let b = chan_sub;
        let c = if a != 0 { b } else { if self.dual != 0 && b >= self.dual { 1 } else { 0 } };
        let d = if a != 0 { 0 } else { if self.dual != 0 { b % self.dual } else { b } };

        match event.event_type {
            0 => {
                // Direct write
                self.write_opl(c, event.value1 as usize, event.value2 as u8, writer);
            }
            1 => {
                // Note on
                let mut ch = d;
                if a != 0 {
                    ch = 11 - a;
                }
                if a == 1 || a == 2 {
                    // Hat/Cymbal use channel 7/8
                    self.write_opl(c, 0xA7, event.value1 as u8, writer);
                    // Note: would need instrument data for offset
                    self.write_opl(c, 0xB7, event.value2 as u8, writer);
                    ch = 8;
                }
                self.write_opl(c, 0xA0 | ch, event.value1 as u8, writer);
                self.write_opl(c, 0xB0 | ch, event.value2 as u8, writer);
                if a != 0 {
                    self.set_opl(c, 0xBD, 1 << (a - 1), 0xFF, writer);
                }
            }
            2 => {
                // Note off
                if a != 0 {
                    self.set_opl(c, 0xBD, 1 << (a - 1), 0, writer);
                } else {
                    self.set_opl(c, 0xB0 | d, 0x20, 0, writer);
                }
            }
            3 => {
                // Instrument change
                self.instr[a][b] = event.value1 as usize;
                // Full instrument set handled in send_with_macro_env
            }
            4 => {
                // Volume change
                self.vol[a][b] = event.value1;
                // Full volume set handled in send_with_macro_env
            }
            5 => {
                // Global setting
                let bd = self.memory[c][0xBD].max(0) as u8;
                self.write_opl(c, 0xBD, (bd & 0x3F) | (event.value1 as u8), writer);
                self.write_opl(c, 0x08, event.value2 as u8, writer);
            }
            _ => {}
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
        let a = chip_sub;
        let b = chan_sub;
        let c = if a != 0 { b } else { if self.dual != 0 && b >= self.dual { 1 } else { 0 } };
        let d = if a != 0 { 0 } else { if self.dual != 0 { b % self.dual } else { b } };

        match event.event_type {
            3 | 4 => {
                // Instrument or volume change
                if event.event_type == 3 {
                    self.instr[a][b] = event.value1 as usize;
                } else {
                    self.vol[a][b] = event.value1;
                }

                let inst_idx = self.instr[a][b].min(255);
                let inst_data = &macro_env[3][inst_idx].data; // MC_Option = 3
                let vol = self.vol[a][b];

                if a == 1 || a == 2 {
                    // Hat/Cymbal
                    self.set_instrument(c, 7, 17, inst_data, vol, writer);
                    self.set_instrument(c, 8, 21, inst_data, vol, writer);
                } else if a == 3 {
                    // Tom
                    self.set_instrument(c, 8, 18, inst_data, vol, writer);
                } else if a == 4 {
                    // SD
                    self.set_instrument(c, 7, 17, inst_data, vol, writer);
                    self.set_instrument(c, 7, 20, inst_data, vol, writer);
                } else if a == 5 {
                    // BD
                    self.set_instrument(c, 6, 16, inst_data, vol, writer);
                    self.set_instrument(c, 6, 19, inst_data, vol, writer);
                } else {
                    // Melody
                    self.set_instrument(c, d, OPER[d], inst_data, vol, writer);
                    self.set_instrument(c, d, OPER[d] + 3, inst_data, vol, writer);
                }
            }
            _ => {
                self.send(event, channel, chip_sub, chan_sub, writer);
            }
        }
    }
}
