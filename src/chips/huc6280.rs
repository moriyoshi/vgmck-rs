//! HuC6280 (PC-Engine) sound chip driver
//!
//! 6 channels of wavetable sound, with noise on channels 4-5
//! LFO/FM capability (channel 1 modulates channel 0)

use super::{chip_id, ChipOptions, MacroCommand, SoundChip};
use crate::compiler::event::ChipEvent;
use crate::compiler::envelope::MacroEnvStorage;
use crate::vgm::header::offset;
use crate::vgm::VgmWriter;

/// Maximum sub-channels per chip_sub type
const MAXSUB: [usize; 3] = [6, 1, 2];

/// HuC6280 chip (PC-Engine/TurboGrafx-16)
pub struct HuC6280 {
    clock: i32,
    dual: bool,
    usesub: [usize; 3],            // [wave, fm, noise]
    assign: [[usize; 2]; 12],      // Channel assignments [chip, chan]
    memory: [[i32; 10]; 12],       // Register memory
    memw: [[bool; 10]; 12],        // Memory write flags (for loop invalidation)
    mult: [i32; 2],                // FM multiplier
    wave: [[i32; 6]; 2],           // Wave index per channel
    fixfm: [bool; 2],              // Fixed FM frequency mode
}

impl HuC6280 {
    pub fn new() -> Self {
        Self {
            clock: 3579545,
            dual: false,
            usesub: [0, 0, 0],
            assign: [[0; 2]; 12],
            memory: [[-1; 10]; 12],
            memw: [[false; 10]; 12],
            mult: [4, 4],
            wave: [[-1; 6]; 2],
            fixfm: [false, false],
        }
    }

    fn mem_write(&mut self, chip: usize, chan: usize, addr: usize, val: i32, writer: &mut VgmWriter) {
        // For registers 2-7, they're per-channel
        let is_per_channel = addr >= 2 && addr <= 7;
        let actual_chan = if is_per_channel { chan } else { 0 };
        let mem_idx = chip * 6 + actual_chan;

        if self.memory[mem_idx][addr] != val || addr == 6 || self.memw[mem_idx][addr] {
            self.memw[mem_idx][addr] = false;

            // Need to select channel first if per-channel register
            if is_per_channel && self.memory[chip * 6][0] != chan as i32 {
                self.mem_write(chip, 0, 0, chan as i32, writer);
            }

            self.memory[mem_idx][addr] = val;
            let cmd_addr = ((chip << 7) | addr) as u8;
            let _ = writer.write_data(&[0xB9, cmd_addr, val as u8]);
        }
    }
}

impl Default for HuC6280 {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundChip for HuC6280 {
    fn name(&self) -> &'static str {
        "HuC6280"
    }

    fn chip_id(&self) -> u8 {
        chip_id::HUC6280
    }

    fn clock_div(&self) -> i32 {
        -self.clock
    }

    fn note_bits(&self) -> i32 {
        12
    }

    fn basic_octave(&self) -> i32 {
        0
    }

    fn enable(&mut self, options: &ChipOptions) {
        self.clock = options.get('H');
        if self.clock == 0 {
            self.clock = 3579545;
        }
    }

    fn file_begin(&mut self, writer: &mut VgmWriter) {
        // Reset state
        self.memory = [[-1; 10]; 12];
        self.memw = [[false; 10]; 12];
        self.mult = [4, 4];
        self.wave = [[-1; 6]; 2];
        self.fixfm = [false, false];

        // Check if dual chip needed
        if self.usesub[0] > (6 - (self.usesub[1] * 2 + self.usesub[2])) {
            self.dual = true;
        }

        // Build channel assignments
        let mut i = 0;

        // Wave channels 2-3 first
        self.assign[i] = [0, 2]; i += 1;
        self.assign[i] = [0, 3]; i += 1;

        // Wave channels 0-1 (if not used for FM)
        if self.usesub[1] < 1 {
            self.assign[i] = [0, 0]; i += 1;
            self.assign[i] = [0, 1]; i += 1;
        }

        // Noise channels 4-5
        if self.usesub[2] < 1 {
            self.assign[i] = [0, 4]; i += 1;
        }
        if self.usesub[2] < 2 {
            self.assign[i] = [0, 5]; i += 1;
        }

        // Second chip assignments
        self.assign[i] = [1, 2]; i += 1;
        self.assign[i] = [1, 3]; i += 1;

        if self.usesub[1] < 2 {
            self.assign[i] = [1, 0]; i += 1;
            self.assign[i] = [1, 1]; i += 1;
        }

        if self.usesub[2] < 3 {
            self.assign[i] = [1, 4]; i += 1;
        }
        if self.usesub[2] < 4 {
            self.assign[i] = [1, 5];
        }

        // Initialize chips
        let chip_count = if self.dual { 2 } else { 1 };
        for c in 0..chip_count {
            self.mem_write(c, 0, 1, 0xFF, writer); // Master volume

            // LFO control
            let lfo_val = if self.usesub[1] > c { 0x80 } else { 0x00 };
            self.mem_write(c, 0, 9, lfo_val, writer);

            // Initialize all channels
            for ch in 0..6 {
                self.mem_write(c, ch, 4, 0, writer);    // Channel volume off
                self.mem_write(c, ch, 5, 0xFF, writer); // Panning center
            }

            // Noise mode for channels 4-5
            let noise4 = if self.usesub[2] <= c * 2 { 0x80 } else { 0x00 };
            self.mem_write(c, 4, 7, noise4, writer);
            let noise5 = if self.usesub[2] <= c * 2 + 1 { 0x80 } else { 0x00 };
            self.mem_write(c, 5, 7, noise5, writer);
        }
    }

    fn file_end(&mut self, writer: &mut VgmWriter) {
        let header = writer.header_mut();
        let clock_val = if self.dual {
            (self.clock as u32) | 0x40000000
        } else {
            self.clock as u32
        };
        header.write_u32(offset::HUC6280_CLOCK, clock_val);
    }

    fn loop_start(&mut self, _writer: &mut VgmWriter) {
        // Invalidate all memory for loop point
        for i in 0..12 {
            for j in 0..10 {
                self.memw[i][j] = true;
            }
        }
    }

    fn start_channel(&mut self, _channel: usize) {}

    fn start_channel_with_info(&mut self, chip_sub: usize, chan_sub: usize) {
        // Check if dual chip needed
        if chan_sub >= MAXSUB[chip_sub] {
            self.dual = true;
        }
        if self.usesub[chip_sub] <= chan_sub {
            self.usesub[chip_sub] = chan_sub + 1;
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
                // type 3 = volume
                Some(ChipEvent::new(3, (value & 31) as i32, 0))
            }
            MacroCommand::Panning => {
                // type 4 = stereo
                let pan = if value < 0 {
                    0xFF ^ ((-value) as i32)
                } else {
                    0xFF ^ ((value as i32) << 4)
                };
                Some(ChipEvent::new(4, pan, 0))
            }
            MacroCommand::Tone => {
                // type 5 = FM tone (only for chip_sub==1)
                Some(ChipEvent::new(5, value as i32, 0))
            }
            MacroCommand::Multiply => {
                // type 6 = FM multiplier (only for chip_sub==1)
                Some(ChipEvent::new(6, value as i32, 0))
            }
            MacroCommand::ModWaveform => {
                // type 7 = modulator waveform (only for chip_sub==1)
                Some(ChipEvent::new(7, value as i32, 0))
            }
            MacroCommand::Waveform => {
                // type 8 = carrier waveform
                Some(ChipEvent::new(8, value as i32, 0))
            }
            MacroCommand::Global => {
                // type 9 = global stereo
                Some(ChipEvent::new(9, value as i32, 0))
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
        // type 2 = note
        Some(ChipEvent::new(2, note, 0))
    }

    fn note_change(&mut self, _channel: usize, note: i32, _octave: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(2, note, 0))
    }

    fn note_off(&mut self, _channel: usize, _note: i32, _octave: i32) -> Option<ChipEvent> {
        // type 1 = rest/note off
        Some(ChipEvent::new(1, 0, 0))
    }

    fn rest(&mut self, _channel: usize, _duration: i32) -> Option<ChipEvent> {
        Some(ChipEvent::new(1, 0, 0))
    }

    fn direct(&mut self, _channel: usize, address: u16, value: u8) -> Option<ChipEvent> {
        // type 0 = direct write
        Some(ChipEvent::new(0, address as i32, value as i32))
    }

    fn send(&mut self, event: &ChipEvent, _channel: usize, chip_sub: usize, chan_sub: usize, writer: &mut VgmWriter) {
        let cs = chip_sub;
        let ca = chan_sub;

        // Determine chip and channel
        let (chip, chan) = if cs != 0 {
            // FM (cs=1) or noise (cs=2)
            let c = ca / cs;
            let ch = if cs == 2 {
                4 | (ca & 5)
            } else {
                0
            };
            (c, ch)
        } else {
            // Wave channel - use assignment table
            (self.assign[ca][0], self.assign[ca][1])
        };

        let v = event.value1;

        match event.event_type as i32 {
            0 => {
                // Direct register write
                let addr = event.value1;
                let val = event.value2;
                self.mem_write(chip, (addr >> 4) as usize, (addr & 15) as usize, val, writer);
            }
            1 => {
                // Rest/note off
                let current = self.memory[chip * 6 + chan][4].max(0);
                self.mem_write(chip, chan, 4, (current & 0x1F) | 0x40, writer);
            }
            2 => {
                // Note
                let w = v;
                let mut v = v;

                if cs == 1 {
                    // FM mode
                    if self.fixfm[chip] {
                        v = 256;
                    }
                    let mut mult = self.mult[chip];
                    if (mult & 1) != 0 {
                        v >>= 2;
                    } else if (mult & 2) != 0 {
                        v >>= 1;
                        mult >>= 1;
                    } else {
                        mult >>= 2;
                    }
                    self.mem_write(chip, 0, 8, mult, writer);
                    self.mem_write(chip, 1, 2, v & 0xFF, writer);
                    self.mem_write(chip, 1, 3, v >> 8, writer);
                }

                if cs == 2 {
                    // Noise mode
                    self.mem_write(chip, chan, 7, v | 0x80, writer);
                } else {
                    self.mem_write(chip, chan, 2, w & 0xFF, writer);
                    self.mem_write(chip, chan, 3, w >> 8, writer);
                }

                // Key on
                let current = self.memory[chip * 6 + chan][4].max(0);
                self.mem_write(chip, chan, 4, (current & 0x1F) | 0x80, writer);
            }
            3 => {
                // Volume
                let current = self.memory[chip * 6 + chan][4].max(0);
                self.mem_write(chip, chan, 4, (current & 0xC0) | v, writer);
            }
            4 => {
                // Stereo/panning
                self.mem_write(chip, chan, 5, v, writer);
            }
            5 => {
                // FM tone
                self.mem_write(chip, 0, 9, v & 3, writer);
                self.fixfm[chip] = (v >> 2) != 0;
            }
            6 => {
                // FM multiplier
                self.mult[ca] = v;
                // Retriggering would need current note stored - simplified version
            }
            9 => {
                // Global stereo
                self.mem_write(0, 0, 1, v, writer);
                if self.dual {
                    self.mem_write(1, 0, 1, v, writer);
                }
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
        let cs = chip_sub;
        let ca = chan_sub;

        let (chip, chan) = if cs != 0 {
            let c = ca / cs;
            let ch = if cs == 2 { 4 | (ca & 5) } else { 0 };
            (c, ch)
        } else {
            (self.assign[ca][0], self.assign[ca][1])
        };

        match event.event_type as i32 {
            7 => {
                // Modulator waveform (FM mode)
                let wave_idx = event.value1 as usize;
                if self.wave[chip][1] != wave_idx as i32 {
                    self.wave[chip][1] = wave_idx as i32;
                    // Turn off channel
                    let current = self.memory[chip * 6 + 1][4].max(0);
                    self.mem_write(chip, 1, 4, current & 0x1F, writer);

                    let wave_data = &macro_env[7][wave_idx.min(255)].data; // MC_Waveform = 7
                    let loop_end = macro_env[7][wave_idx.min(255)].loop_end.saturating_sub(1) as i32;

                    for i in 0..32 {
                        let sample = wave_data.get(i).copied().unwrap_or(0) as i32 & loop_end;
                        self.mem_write(chip, 1, 6, sample, writer);
                    }
                }
            }
            8 => {
                // Carrier waveform
                let wave_idx = event.value1 as usize;
                if self.wave[chip][chan] != wave_idx as i32 {
                    self.wave[chip][chan] = wave_idx as i32;
                    // Turn off channel
                    let current = self.memory[chip * 6 + chan][4].max(0);
                    self.mem_write(chip, chan, 4, current & 0x1F, writer);

                    let wave_data = &macro_env[7][wave_idx.min(255)].data;
                    let loop_end = macro_env[7][wave_idx.min(255)].loop_end.saturating_sub(1) as i32;

                    for i in 0..32 {
                        let sample = wave_data.get(i).copied().unwrap_or(0) as i32 & loop_end;
                        self.mem_write(chip, chan, 6, sample, writer);
                    }
                }
            }
            _ => {
                self.send(event, channel, chip_sub, chan_sub, writer);
            }
        }
    }
}
