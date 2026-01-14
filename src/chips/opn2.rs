//! YM2612 (OPN2) sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// YM2612 OPN2 chip
pub struct Opn2 {
    clock: i32,
    nor: usize,      // Normal channels used
    sup: usize,      // Supplementary channels used
    dual: bool,      // Dual chip mode
    assign: [u8; 12], // Channel assignment table
    mem: Vec<i16>,    // Register memory cache
    vol: [u8; 12],    // Volume per channel
    pan: [u8; 12],    // Panning per channel
}

impl Opn2 {
    pub fn new() -> Self {
        Self {
            clock: 7670454,
            nor: 0,
            sup: 0,
            dual: false,
            assign: [0, 1, 4, 5, 8, 9, 12, 13, 14, 10, 6, 2],
            mem: vec![-1; 0x400],
            vol: [127; 12],
            pan: [0xC0; 12],
        }
    }

    /// Write to OPN2 register with caching
    fn opn2_put(&mut self, address: usize, data: u8, writer: &mut VgmWriter) {
        // Write if value changed, or if it's a frequency register (0xA0-0xAF)
        if (self.mem[address] != data as i16 || (address & 0xA0) == 0xA0)
            && (self.dual || (address & 0x200) == 0)
        {
            self.mem[address] = data as i16;
            let cmd = if (address & 0x200) != 0 { 0xA2 } else { 0x52 }
                | ((address >> 8) & 1) as u8;
            let _ = writer.write_data(&[cmd, (address & 0xFF) as u8, data]);
        }
    }

    /// Update FM operators for a channel
    fn update_oper(
        &mut self,
        mo: bool,
        ch: usize,
        oper_data: &[i16],
        writer: &mut VgmWriter,
    ) {
        let ad = (((self.assign[ch] as usize) & 12) << 6) | ((self.assign[ch] as usize) & 3);

        // Determine which operators affect output based on algorithm
        let mut aff = [0i32, 0, 0, 16];
        let alg = (oper_data.get(28).copied().unwrap_or(0) & 7) as usize;
        if alg > 3 {
            aff[2] = 16;
        }
        if alg > 4 {
            aff[1] = 16;
        }
        if alg == 7 {
            aff[0] = 16;
        }

        // Write operator data
        for i in 0..4 {
            let op_aff = if mo {
                oper_data.get(i * 3 + 32).copied().unwrap_or(0) as i32
            } else {
                aff[i]
            };

            for j in 0..7 {
                let mut k = oper_data.get(i * 7 + j).copied().unwrap_or(0) as i32;
                if j == 1 {
                    // Total level - apply volume
                    k += ((self.vol[ch] as i32) * op_aff) >> 4;
                    k = k.clamp(0, 127);
                }
                self.opn2_put(ad | (i << 2) | ((j + 3) << 4), k as u8, writer);
            }
        }

        // Algorithm and feedback
        let alg_fb = oper_data.get(28).copied().unwrap_or(0) as u8;
        self.opn2_put(ad | 0xB0, alg_fb, writer);

        // Panning and LFO sensitivity
        let pan_lfo = (oper_data.get(29).copied().unwrap_or(0) as u8) | self.pan[ch];
        self.opn2_put(ad | 0xB4, pan_lfo, writer);
    }

    /// Update note frequency for a channel
    fn update_note(
        &mut self,
        mo: bool,
        ch: usize,
        note: i32,
        oper_data: &[i16],
        writer: &mut VgmWriter,
    ) {
        let mut ad = (((self.assign[ch] as usize) & 12) << 6) | ((self.assign[ch] as usize) & 3);

        if mo {
            // Multi-operator mode - each operator can have different frequency
            for i in 0..4 {
                let op_note = oper_data.get(i * 3 + 31).copied().unwrap_or(0);
                let op_block = oper_data.get(i * 3 + 30).copied().unwrap_or(0);
                let h = if op_note != 0 { op_note as i32 } else { note } | ((op_block as i32) << 11);
                self.opn2_put((ad | 0xA4) + i, (h >> 8) as u8, writer);
                self.opn2_put((ad | 0xA0) + i, (h & 0xFF) as u8, writer);
                ad |= 4;
            }
        } else {
            self.opn2_put(ad | 0xA4, (note >> 8) as u8, writer);
            self.opn2_put(ad | 0xA0, (note & 0xFF) as u8, writer);
        }
    }
}

impl Default for Opn2 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Opn2 {
    fn name(&self) -> &'static str {
        "OPN2"
    }

    fn chip_id(&self) -> u8 {
        chip_id::YM2612
    }

    fn clock_div(&self) -> i32 {
        self.clock
    }

    fn note_bits(&self) -> i32 {
        -11
    }

    fn basic_octave(&self) -> i32 {
        7
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 7670454;
        }
    }

    fn file_begin(&mut self, _writer: &mut VgmWriter) {
        // Reset state (but preserve nor/sup from channel parsing)
        self.mem.fill(-1);
        self.vol = [127; 12];
        self.pan = [0xC0; 12];

        // Build channel assignment based on supplementary channels used
        let mut i = 0;
        self.assign[i] = 0;
        i += 1;
        self.assign[i] = 1;
        i += 1;
        if self.sup < 1 {
            self.assign[i] = 2;
            i += 1;
        }
        self.assign[i] = 4;
        i += 1;
        self.assign[i] = 5;
        i += 1;
        if self.sup < 2 {
            self.assign[i] = 6;
            i += 1;
        }
        self.assign[i] = 8;
        i += 1;
        self.assign[i] = 9;
        i += 1;
        if self.sup < 3 {
            self.assign[i] = 10;
            i += 1;
        }
        self.assign[i] = 12;
        i += 1;
        self.assign[i] = 13;
        i += 1;
        if self.sup < 4 {
            self.assign[i] = 14;
        }
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        self.dual = self.sup > 2 || self.nor > 6 - self.sup;

        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::YM2612_CLOCK, clock_val);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {}

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        let y = chan_sub + 1;
        if chip_sub != 0 {
            if y > self.sup {
                self.sup = y;
            }
        } else if y > self.nor {
            self.nor = y;
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
            MacroCommand::Volume => Some(ChipEvent::new(0x6000, (value ^ 127) as i32, 0)),
            MacroCommand::Panning => {
                let pan = if value < 0 {
                    0x80
                } else if value > 0 {
                    0x40
                } else {
                    0xC0
                };
                Some(ChipEvent::new(0x7000, pan, 0))
            }
            MacroCommand::Tone => Some(ChipEvent::new(0x5000, (value & 255) as i32, 0)),
            MacroCommand::Global => Some(ChipEvent::new(0x1022, value as i32, 0)),
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
        Some(ChipEvent::new(0x3000, note | (octave << 11), 0))
    }

    fn note_change(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x4000, note | (octave << 11), 0))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x2000, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        None
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        Some(ChipEvent::new(address, value as i32, 0))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let cs = chan_sub;
        let mo = chip_sub != 0;
        let ch = if mo { 12 - cs } else { cs };

        match event.event_type >> 12 {
            0 => {
                // Direct write
                let addr = (event.event_type & 0x3FF) as usize;
                self.opn2_put(addr, event.value1 as u8, writer);
            }
            1 => {
                // Write global (all ports)
                let addr = (event.event_type & 0xFF) as usize;
                self.opn2_put(addr, event.value1 as u8, writer);
                self.opn2_put(addr | 0x100, event.value1 as u8, writer);
                self.opn2_put(addr | 0x200, event.value1 as u8, writer);
                self.opn2_put(addr | 0x300, event.value1 as u8, writer);
            }
            2 => {
                // Note off
                let key_addr = (((self.assign[ch] as usize) & 8) << 5) | 0x28;
                self.opn2_put(key_addr, self.assign[ch] & 7, writer);
            }
            3 => {
                // Note on - update note then key on
                // Note: In full implementation, would call update_note with macro env data
                let note = event.value1;
                let ad = (((self.assign[ch] as usize) & 12) << 6) | ((self.assign[ch] as usize) & 3);
                self.opn2_put(ad | 0xA4, (note >> 8) as u8, writer);
                self.opn2_put(ad | 0xA0, (note & 0xFF) as u8, writer);
                let key_addr = (((self.assign[ch] as usize) & 8) << 5) | 0x28;
                self.opn2_put(key_addr, 0xF0 | (self.assign[ch] & 0xF7), writer);
            }
            4 => {
                // Note change
                let note = event.value1;
                let ad = (((self.assign[ch] as usize) & 12) << 6) | ((self.assign[ch] as usize) & 3);
                self.opn2_put(ad | 0xA4, (note >> 8) as u8, writer);
                self.opn2_put(ad | 0xA0, (note & 0xFF) as u8, writer);
            }
            5 => {
                // Set operators (tone/instrument change)
                // Note: Would need macro_env access for full implementation
            }
            6 => {
                // Set volume
                self.vol[ch] = event.value1 as u8;
                // Note: Would call update_oper with macro env data
            }
            7 => {
                // Set panning
                self.pan[ch] = event.value1 as u8;
                // Note: Would call update_oper with macro env data
            }
            _ => {}
        }
    }

    fn send_with_macro_env(
        &mut self,
        event: &ChipEvent,
        _channel: usize,
        chip_sub: usize,
        chan_sub: usize,
        writer: &mut VgmWriter,
        macro_env: &MacroEnvStorage,
    ) {
        let cs = chan_sub;
        let mo = chip_sub != 0;
        let ch = if mo { 12 - cs } else { cs };

        // Get operator data from macro env
        let oper_idx = event.value2 as usize;
        let oper_data = &macro_env[3][oper_idx.min(255)].data; // MC_Option = 3

        match event.event_type >> 12 {
            0 => {
                // Direct write
                let addr = (event.event_type & 0x3FF) as usize;
                self.opn2_put(addr, event.value1 as u8, writer);
            }
            1 => {
                // Write global (all ports)
                let addr = (event.event_type & 0xFF) as usize;
                self.opn2_put(addr, event.value1 as u8, writer);
                self.opn2_put(addr | 0x100, event.value1 as u8, writer);
                self.opn2_put(addr | 0x200, event.value1 as u8, writer);
                self.opn2_put(addr | 0x300, event.value1 as u8, writer);
            }
            2 => {
                // Note off
                let key_addr = (((self.assign[ch] as usize) & 8) << 5) | 0x28;
                self.opn2_put(key_addr, self.assign[ch] & 7, writer);
            }
            3 => {
                // Note on
                self.update_note(mo, ch, event.value1, oper_data, writer);
                let key_addr = (((self.assign[ch] as usize) & 8) << 5) | 0x28;
                self.opn2_put(key_addr, 0xF0 | (self.assign[ch] & 0xF7), writer);
            }
            4 => {
                // Note change
                self.update_note(mo, ch, event.value1, oper_data, writer);
            }
            5 => {
                // Set operators (tone/instrument change)
                let idx = (event.value1 & 255) as usize;
                let new_oper = &macro_env[3][idx.min(255)].data;
                self.update_oper(mo, ch, new_oper, writer);
            }
            6 => {
                // Set volume
                self.vol[ch] = event.value1 as u8;
                self.update_oper(mo, ch, oper_data, writer);
            }
            7 => {
                // Set panning
                self.pan[ch] = event.value1 as u8;
                self.update_oper(mo, ch, oper_data, writer);
            }
            _ => {}
        }
    }
}
