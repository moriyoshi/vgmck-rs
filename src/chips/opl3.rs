//! YMF262 (OPL3) sound chip driver

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// Operator offset table (2-op mode)
const CHOP: [u8; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];

/// 4-operator offsets
const FOP: [u8; 4] = [0, 3, 8, 11];

/// VGM command bytes for each port/instance combination
const INST: [u8; 4] = [0x5E, 0x5F, 0xAE, 0xAF];

/// YMF262 OPL3 chip
pub struct Opl3 {
    clock: i32,
    a2op: [u8; 36],       // 2-op channel assignments
    a4op: [u8; 12],       // 4-op channel assignments
    use_count: [usize; 3], // [two-ops, four-ops, rhythm]
    dual: bool,
    drum: [u8; 2],
    sam: [u16; 2],
    tone: u16,
}

impl Opl3 {
    pub fn new() -> Self {
        Self {
            clock: 14318180,
            a2op: [0; 36],
            a4op: [0; 12],
            use_count: [0, 0, 0],
            dual: false,
            drum: [0, 0],
            sam: [0, 0],
            tone: 0xC000,
        }
    }

    fn poke(&self, id: usize, addr: u8, data: u8, writer: &mut VgmWriter) {
        if (id & 2) != 0 && !self.dual {
            return;
        }
        let _ = writer.write_data(&[INST[id & 3], addr, data]);
    }

    fn poke_chan(&self, ch: usize, addr: u8, data: u8, writer: &mut VgmWriter) {
        if (ch & 15) == 15 {
            // Rhythm channels 6, 7, 8
            self.poke(ch >> 6, addr | 6, data, writer);
            self.poke(ch >> 6, addr | 7, data, writer);
            self.poke(ch >> 6, addr | 8, data, writer);
        } else {
            self.poke(ch >> 6, addr | (ch & 15) as u8, data, writer);
        }
    }

    fn poke_oper(&self, ch: usize, op: usize, addr: u8, data: u8, writer: &mut VgmWriter) {
        if (ch & 15) == 15 {
            // Rhythm mode operator
            self.poke(ch >> 6, (op as u8) + addr + 16, data, writer);
        } else {
            self.poke(ch >> 6, CHOP[ch & 15] + FOP[op & 3] + addr, data, writer);
        }
    }

    fn instrument(&self, sub: usize, ch: usize, patch: bool, data: u16, macro_env: &MacroEnvStorage, writer: &mut VgmWriter) {
        let inst_idx = (data & 255) as usize;
        let inst_data = &macro_env[3][inst_idx.min(255)].data; // MC_Option = 3

        let mut op = (sub + 1) << 1;
        let fb_data = inst_data.get(op * 5).copied().unwrap_or(0);
        let alg = ((fb_data >> 4) & 3) as u8;
        let fb = (fb_data & 7) as u8;
        let vol = ((data >> 8) & 0x3F) as i32;
        let pan = ((data >> 10) & 0x30) as u8;

        while op > 0 {
            op -= 1;
            if patch {
                self.poke_oper(ch, op, 0x20, inst_data.get(op * 5).copied().unwrap_or(0) as u8, writer);
                let op_flags = inst_data.get(op * 5 + 4).copied().unwrap_or(0);
                if (op_flags & 0x10) != 0 {
                    self.poke_oper(ch, op, 0x40, inst_data.get(op * 5 + 1).copied().unwrap_or(0) as u8, writer);
                }
                self.poke_oper(ch, op, 0x60, inst_data.get(op * 5 + 2).copied().unwrap_or(0) as u8, writer);
                self.poke_oper(ch, op, 0x80, inst_data.get(op * 5 + 3).copied().unwrap_or(0) as u8, writer);
                self.poke_oper(ch, op, 0xE0, (op_flags & 0x07) as u8, writer);
            }
            let op_flags = inst_data.get(op * 5 + 4).copied().unwrap_or(0);
            if (op_flags & 0x10) == 0 {
                let tl = inst_data.get(op * 5 + 1).copied().unwrap_or(0);
                let mut x = (tl & 0x3F) as i32 + vol;
                if x > 63 {
                    x = 63;
                }
                self.poke_oper(ch, op, 0x40, (x as u8) | ((tl as u8) & 0xC0), writer);
            }
        }

        if sub == 1 {
            self.poke_chan(ch + 3, 0xC0, alg >> 1, writer);
        }
        let x = (fb << 1) | (alg & 1);
        self.poke_chan(ch, 0xC0, x | pan, writer);
    }
}

impl Default for Opl3 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for Opl3 {
    fn name(&self) -> &'static str {
        "OPL3"
    }

    fn chip_id(&self) -> u8 {
        chip_id::YMF262
    }

    fn clock_div(&self) -> i32 {
        self.clock / 9
    }

    fn note_bits(&self) -> i32 {
        -10
    }

    fn basic_octave(&self) -> i32 {
        0
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 14318180;
        }
    }

    fn file_begin(&mut self, writer: &mut VgmWriter) {
        let mut a2 = 0usize;
        let mut a4 = 0usize;

        // Assignment of operators and dual chips
        self.dual = self.use_count[2] > 1;

        // Rhythm channels for second chip
        self.a2op[a2] = 0x46; a2 += 1;
        self.a2op[a2] = 0x47; a2 += 1;
        self.a2op[a2] = 0x48; a2 += 1;

        // First chip rhythm channels (if not in rhythm mode)
        if self.use_count[2] < 1 {
            self.a2op[a2] = 0x06; a2 += 1;
            self.a2op[a2] = 0x07; a2 += 1;
            self.a2op[a2] = 0x08; a2 += 1;
        }

        // Channel pairs - 2-op or 4-op based on use_count[1]
        if self.use_count[1] < 1 { self.a2op[a2] = 0x00; a2 += 1; self.a2op[a2] = 0x03; a2 += 1; }
        else { self.a4op[a4] = 0x00; a4 += 1; }

        if self.use_count[1] < 2 { self.a2op[a2] = 0x01; a2 += 1; self.a2op[a2] = 0x04; a2 += 1; }
        else { self.a4op[a4] = 0x01; a4 += 1; }

        if self.use_count[1] < 3 { self.a2op[a2] = 0x02; a2 += 1; self.a2op[a2] = 0x05; a2 += 1; }
        else { self.a4op[a4] = 0x02; a4 += 1; }

        if self.use_count[1] < 4 { self.a2op[a2] = 0x40; a2 += 1; self.a2op[a2] = 0x43; a2 += 1; }
        else { self.a4op[a4] = 0x40; a4 += 1; }

        if self.use_count[1] < 5 { self.a2op[a2] = 0x41; a2 += 1; self.a2op[a2] = 0x44; a2 += 1; }
        else { self.a4op[a4] = 0x41; a4 += 1; }

        if self.use_count[1] < 6 { self.a2op[a2] = 0x42; a2 += 1; self.a2op[a2] = 0x45; a2 += 1; }
        else { self.a4op[a4] = 0x42; a4 += 1; }

        if self.use_count[1] < 7 { self.a2op[a2] = 0x80; a2 += 1; self.a2op[a2] = 0x83; a2 += 1; }
        else { self.a4op[a4] = 0x80; a4 += 1; }

        if self.use_count[1] < 8 { self.a2op[a2] = 0x81; a2 += 1; self.a2op[a2] = 0x84; a2 += 1; }
        else { self.a4op[a4] = 0x81; a4 += 1; }

        if self.use_count[1] < 9 { self.a2op[a2] = 0x82; a2 += 1; self.a2op[a2] = 0x85; a2 += 1; }
        else { self.a4op[a4] = 0x82; a4 += 1; }

        if self.use_count[1] < 10 { self.a2op[a2] = 0xC0; a2 += 1; self.a2op[a2] = 0xC3; a2 += 1; }
        else { self.a4op[a4] = 0xC0; a4 += 1; }

        if self.use_count[1] < 11 { self.a2op[a2] = 0xC1; a2 += 1; self.a2op[a2] = 0xC4; a2 += 1; }
        else { self.a4op[a4] = 0xC1; a4 += 1; }

        if self.use_count[1] < 12 { self.a2op[a2] = 0xC2; a2 += 1; self.a2op[a2] = 0xC5; a2 += 1; }
        else { self.a4op[a4] = 0xC2; }

        // Last rhythm channels
        self.a2op[a2] = 0xC6; a2 += 1;
        self.a2op[a2] = 0xC7; a2 += 1;
        self.a2op[a2] = 0xC8; a2 += 1;

        if self.use_count[2] < 2 {
            self.a2op[a2] = 0x86; a2 += 1;
            self.a2op[a2] = 0x87; a2 += 1;
            self.a2op[a2] = 0x88;
        }

        // Check if dual chip needed
        for x in 0..self.use_count[0] {
            if (self.a2op[x] & 0x80) != 0 {
                self.dual = true;
            }
        }
        if self.use_count[1] > 6 {
            self.dual = true;
        }

        // Initialize
        self.poke(0, 0x01, 0x20, writer); // Waveform select enable
        self.poke(1, 0x05, 0x01, writer); // OPL3 mode enable
        self.poke(2, 0x01, 0x20, writer); // Second chip waveform select
        self.poke(3, 0x05, 0x01, writer); // Second chip OPL3 mode

        // 4-op connection enable
        let conn1 = ((1u8 << self.use_count[1].min(6)) - 1) & 0x3F;
        self.poke(1, 0x04, conn1, writer);
        let conn2 = if self.use_count[1] > 6 {
            ((1u8 << (self.use_count[1] - 6).min(6)) - 1) & 0x3F
        } else {
            0
        };
        self.poke(3, 0x04, conn2, writer);

        // Reset drum/sample state
        self.drum = [0, 0];
        self.sam = [0, 0];
        self.tone = 0xC000;
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::YMF262_CLOCK, clock_val);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {}

    fn start_channel(&mut self, _channel: usize) {}

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        let b = chan_sub + 1;
        if self.use_count[chip_sub] < b {
            self.use_count[chip_sub] = b;
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
                // type 3 = volume/panning
                self.tone = (self.tone & !0x3F00) | (((63 & !value) as u16) << 8);
                Some(ChipEvent::new(0x403, self.tone as i32, 0))
            }
            MacroCommand::Panning => {
                // Panning
                let pan = if value < 0 {
                    0x4000u16
                } else if value > 0 {
                    0x8000u16
                } else {
                    0xC000u16
                };
                self.tone = (self.tone & !0xC000) | pan;
                Some(ChipEvent::new(0x403, self.tone as i32, 0))
            }
            MacroCommand::Tone => {
                // Instrument
                self.tone = (self.tone & !0xFF) | ((value as u16) & 255);
                Some(ChipEvent::new(0x405, self.tone as i32, 0))
            }
            MacroCommand::Global => {
                // Global (tremolo/vibrato depth)
                Some(ChipEvent::new(0x406, value as i32, 0))
            }
            MacroCommand::Sample => {
                // Rhythm sample
                Some(ChipEvent::new(0x404, value as i32, 0))
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
        // Will be modified in send based on chip_sub
        // value1 = note, value2 = octave | key_on_flag
        Some(ChipEvent::new(0x400, note | (octave << 10) | 0x2000, 0))
    }

    fn note_change(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(0x400, note | (octave << 10) | 0x2000, 0))
    }

    fn note_off(&mut self, _channel: usize, note: i32, octave: i32) -> Option<ChipEvent> {
        // Note off - no key-on bit
        Some(ChipEvent::new(0x400, note | (octave << 10), 0))
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
        let c = if a == 2 {
            15 | (b << 7)
        } else if a != 0 {
            self.a4op[b] as usize
        } else {
            self.a2op[b] as usize
        };

        if event.event_type >= 0x400 {
            let cmd = event.event_type & 7;
            match cmd {
                0 => {
                    // Note on/off/change
                    let d = event.value1 as u16;
                    self.poke_chan(c, 0xA0, (d & 255) as u8, writer);
                    self.poke_chan(c, 0xB0, (d >> 8) as u8, writer);
                }
                1 => {
                    // Rhythm on
                    let mut d = event.value1 as u16;
                    if (self.sam[b] >> 5) != 0 {
                        d = self.sam[b] >> 5;
                    }
                    self.poke_chan(c, 0xA0, (d & 255) as u8, writer);
                    self.poke_chan(c, 0xB0, (d >> 8) as u8, writer);
                    self.drum[b] = (self.sam[b] as u8 & 0x1F) | 0x20 | (self.drum[b] & 0xC0);
                    self.poke(b << 1, 0xBD, self.drum[b], writer);
                }
                2 => {
                    // Rhythm off
                    self.drum[b] &= 0xE0;
                    self.poke(b << 1, 0xBD, self.drum[b], writer);
                }
                4 => {
                    // Rhythm sample set
                    self.sam[b] = event.value1 as u16;
                }
                6 => {
                    // Global setting
                    let d = event.value1 as u8;
                    self.drum[0] &= 0x3F;
                    self.drum[0] |= (d & 3) << 6;
                    self.poke(0, 0xBD, self.drum[0], writer);
                    self.drum[1] &= 0x3F;
                    self.drum[1] |= (d & 3) << 6;
                    self.poke(2, 0xBD, self.drum[1], writer);
                    self.poke(0, 0x08, (d & 12) << 4, writer);
                    self.poke(2, 0x08, (d & 12) << 4, writer);
                }
                _ => {}
            }
        } else {
            // Direct register write
            let port = (event.event_type >> 8) as usize;
            let addr = (event.event_type & 0xFF) as u8;
            self.poke(port, addr, event.value1 as u8, writer);
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
        let a = chip_sub;
        let b = chan_sub;
        let c = if a == 2 {
            15 | (b << 7)
        } else if a != 0 {
            self.a4op[b] as usize
        } else {
            self.a2op[b] as usize
        };

        if event.event_type >= 0x400 {
            let cmd = event.event_type & 7;
            match cmd {
                3 => {
                    // Volume/panning
                    self.instrument(a, c, false, event.value1 as u16, macro_env, writer);
                }
                5 => {
                    // Instrument set
                    self.instrument(a, c, true, event.value1 as u16, macro_env, writer);
                }
                _ => {
                    self.send(event, _channel, chip_sub, chan_sub, writer);
                }
            }
        } else {
            self.send(event, _channel, chip_sub, chan_sub, writer);
        }
    }
}
