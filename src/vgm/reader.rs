//! VGM file reader and parser

use super::commands::{command_size, opcode, VgmCommand};
use super::header::offset;
use crate::error::{Error, Result};
use std::collections::HashMap;

/// Parsed VGM header information
#[derive(Debug, Clone, Default)]
pub struct VgmHeader {
    pub version: u32,
    pub eof_offset: u32,
    pub total_samples: u32,
    pub loop_offset: u32,
    pub loop_samples: u32,
    pub rate: u32,
    pub data_offset: u32,
    pub gd3_offset: u32,
    pub volume_modifier: i8,
    pub loop_base: i8,
    pub loop_modifier: u8,
    pub chips: HashMap<String, ChipInfo>,
}

/// Information about a chip in the VGM
#[derive(Debug, Clone)]
pub struct ChipInfo {
    pub clock: u32,
    pub dual: bool,
    pub extra: HashMap<String, u32>,
}

/// Parsed GD3 metadata
#[derive(Debug, Clone, Default)]
pub struct Gd3Info {
    pub title: String,
    pub title_jp: String,
    pub game: String,
    pub game_jp: String,
    pub system: String,
    pub system_jp: String,
    pub composer: String,
    pub composer_jp: String,
    pub date: String,
    pub converter: String,
    pub notes: String,
}

/// VGM file reader
pub struct VgmReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> VgmReader<'a> {
    /// Create a new reader from raw VGM data
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Check if we've reached the end of data
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Seek to a position
    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Read a single byte
    pub fn read_u8(&mut self) -> Result<u8> {
        if self.pos >= self.data.len() {
            return Err(Error::VgmParse("Unexpected end of data".into()));
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Ok(b)
    }

    /// Read a 16-bit little-endian value
    pub fn read_u16_le(&mut self) -> Result<u16> {
        let lo = self.read_u8()? as u16;
        let hi = self.read_u8()? as u16;
        Ok(lo | (hi << 8))
    }

    /// Read a 32-bit little-endian value
    pub fn read_u32_le(&mut self) -> Result<u32> {
        let lo = self.read_u16_le()? as u32;
        let hi = self.read_u16_le()? as u32;
        Ok(lo | (hi << 16))
    }

    /// Read a 24-bit little-endian value
    pub fn read_u24_le(&mut self) -> Result<u32> {
        let b0 = self.read_u8()? as u32;
        let b1 = self.read_u8()? as u32;
        let b2 = self.read_u8()? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16))
    }

    /// Read bytes into a buffer
    pub fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>> {
        if self.pos + len > self.data.len() {
            return Err(Error::VgmParse("Unexpected end of data".into()));
        }
        let bytes = self.data[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(bytes)
    }

    /// Read a u32 at a specific offset without advancing position
    fn peek_u32_at(&self, offset: usize) -> Result<u32> {
        if offset + 4 > self.data.len() {
            return Err(Error::VgmParse("Offset out of bounds".into()));
        }
        Ok(u32::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]))
    }

    /// Read a u16 at a specific offset without advancing position
    fn peek_u16_at(&self, offset: usize) -> Result<u16> {
        if offset + 2 > self.data.len() {
            return Err(Error::VgmParse("Offset out of bounds".into()));
        }
        Ok(u16::from_le_bytes([
            self.data[offset],
            self.data[offset + 1],
        ]))
    }

    /// Read a u8 at a specific offset without advancing position
    fn peek_u8_at(&self, offset: usize) -> Result<u8> {
        if offset >= self.data.len() {
            return Err(Error::VgmParse("Offset out of bounds".into()));
        }
        Ok(self.data[offset])
    }

    /// Validate VGM magic and parse header
    pub fn parse_header(&mut self) -> Result<VgmHeader> {
        // Check magic
        if self.data.len() < 64 {
            return Err(Error::VgmParse("File too small for VGM header".into()));
        }
        if &self.data[0..4] != b"Vgm " {
            return Err(Error::VgmParse("Invalid VGM magic".into()));
        }

        let version = self.peek_u32_at(offset::VERSION)?;
        let eof_offset = self.peek_u32_at(offset::EOF_OFFSET)?;
        let total_samples = self.peek_u32_at(offset::TOTAL_SAMPLES)?;
        let loop_offset = self.peek_u32_at(offset::LOOP_OFFSET)?;
        let loop_samples = self.peek_u32_at(offset::LOOP_SAMPLES)?;
        let rate = self.peek_u32_at(offset::RATE)?;
        let gd3_offset = self.peek_u32_at(offset::GD3_OFFSET)?;

        // Data offset is relative to 0x34, default to 0x0C (so data starts at 0x40) for older versions
        let data_offset = if version >= 0x150 {
            let rel_offset = self.peek_u32_at(offset::DATA_OFFSET)?;
            if rel_offset == 0 {
                0x0C // Default for older files
            } else {
                rel_offset
            }
        } else {
            0x0C
        };

        // Parse chip clocks
        let mut chips = HashMap::new();
        self.parse_chip_clock(&mut chips, "sn76489", offset::SN76489_CLOCK)?;
        self.parse_chip_clock(&mut chips, "ym2413", offset::YM2413_CLOCK)?;
        self.parse_chip_clock(&mut chips, "ym2612", offset::YM2612_CLOCK)?;
        self.parse_chip_clock(&mut chips, "ym2151", offset::YM2151_CLOCK)?;

        // Extended header chips (version >= 1.51)
        if version >= 0x151 {
            self.parse_chip_clock(&mut chips, "sega_pcm", offset::SEGA_PCM_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ym2203", offset::YM2203_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ym2608", offset::YM2608_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ym2610", offset::YM2610_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ym3812", offset::YM3812_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ym3526", offset::YM3526_CLOCK)?;
            self.parse_chip_clock(&mut chips, "y8950", offset::Y8950_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ymf262", offset::YMF262_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ymf278b", offset::YMF278B_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ymf271", offset::YMF271_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ymz280b", offset::YMZ280B_CLOCK)?;
            self.parse_chip_clock(&mut chips, "rf5c164", offset::RF5C164_CLOCK)?;
            self.parse_chip_clock(&mut chips, "pwm", offset::PWM_CLOCK)?;
            self.parse_chip_clock(&mut chips, "ay8910", offset::AY8910_CLOCK)?;
        }

        // Version >= 1.61 chips
        if version >= 0x161 {
            self.parse_chip_clock(&mut chips, "gb_dmg", offset::GB_DMG_CLOCK)?;
            self.parse_chip_clock(&mut chips, "nes_apu", offset::NES_APU_CLOCK)?;
            self.parse_chip_clock(&mut chips, "multi_pcm", offset::MULTI_PCM_CLOCK)?;
            self.parse_chip_clock(&mut chips, "upd7759", offset::UPD7759_CLOCK)?;
            self.parse_chip_clock(&mut chips, "okim6258", offset::OKIM6258_CLOCK)?;
            self.parse_chip_clock(&mut chips, "k051649", offset::K051649_CLOCK)?;
            self.parse_chip_clock(&mut chips, "k054539", offset::K054539_CLOCK)?;
            self.parse_chip_clock(&mut chips, "huc6280", offset::HUC6280_CLOCK)?;
            self.parse_chip_clock(&mut chips, "c140", offset::C140_CLOCK)?;
            self.parse_chip_clock(&mut chips, "k053260", offset::K053260_CLOCK)?;
            self.parse_chip_clock(&mut chips, "pokey", offset::POKEY_CLOCK)?;
            self.parse_chip_clock(&mut chips, "qsound", offset::QSOUND_CLOCK)?;
        }

        // Add SN76489 extra info
        if chips.contains_key("sn76489") {
            let feedback = self.peek_u16_at(offset::SN76489_FEEDBACK)?;
            let shift_width = self.peek_u8_at(offset::SN76489_SHIFT_WIDTH)?;
            let flags = self.peek_u8_at(offset::SN76489_FLAGS)?;
            if let Some(chip) = chips.get_mut("sn76489") {
                chip.extra.insert("feedback".into(), feedback as u32);
                chip.extra.insert("shift_width".into(), shift_width as u32);
                chip.extra.insert("flags".into(), flags as u32);
            }
        }

        // Volume/loop modifiers
        let volume_modifier = self.peek_u8_at(offset::VOLUME_MODIFIER)? as i8;
        let loop_base = self.peek_u8_at(offset::LOOP_BASE)? as i8;
        let loop_modifier = self.peek_u8_at(offset::LOOP_MODIFIER)?;

        Ok(VgmHeader {
            version,
            eof_offset,
            total_samples,
            loop_offset,
            loop_samples,
            rate,
            data_offset,
            gd3_offset,
            volume_modifier,
            loop_base,
            loop_modifier,
            chips,
        })
    }

    /// Parse a chip clock from the header
    fn parse_chip_clock(
        &self,
        chips: &mut HashMap<String, ChipInfo>,
        name: &str,
        clock_offset: usize,
    ) -> Result<()> {
        if clock_offset + 4 > self.data.len() {
            return Ok(());
        }
        let clock = self.peek_u32_at(clock_offset)?;
        if clock != 0 {
            let dual = (clock & 0x4000_0000) != 0;
            let clock_hz = clock & 0x3FFF_FFFF;
            chips.insert(
                name.to_string(),
                ChipInfo {
                    clock: clock_hz,
                    dual,
                    extra: HashMap::new(),
                },
            );
        }
        Ok(())
    }

    /// Parse GD3 metadata
    pub fn parse_gd3(&mut self, header: &VgmHeader) -> Result<Option<Gd3Info>> {
        if header.gd3_offset == 0 {
            return Ok(None);
        }

        // GD3 offset is relative to 0x14
        let gd3_pos = (header.gd3_offset as usize) + 0x14;
        if gd3_pos + 12 > self.data.len() {
            return Ok(None);
        }

        self.seek(gd3_pos);

        // Check GD3 magic
        let magic = self.read_bytes(4)?;
        if magic != b"Gd3 " {
            return Ok(None);
        }

        let _version = self.read_u32_le()?;
        let _size = self.read_u32_le()?;

        // Read 11 UTF-16LE strings
        let title = self.read_utf16_string()?;
        let title_jp = self.read_utf16_string()?;
        let game = self.read_utf16_string()?;
        let game_jp = self.read_utf16_string()?;
        let system = self.read_utf16_string()?;
        let system_jp = self.read_utf16_string()?;
        let composer = self.read_utf16_string()?;
        let composer_jp = self.read_utf16_string()?;
        let date = self.read_utf16_string()?;
        let converter = self.read_utf16_string()?;
        let notes = self.read_utf16_string()?;

        Ok(Some(Gd3Info {
            title,
            title_jp,
            game,
            game_jp,
            system,
            system_jp,
            composer,
            composer_jp,
            date,
            converter,
            notes,
        }))
    }

    /// Read a null-terminated UTF-16LE string
    fn read_utf16_string(&mut self) -> Result<String> {
        let mut chars = Vec::new();

        loop {
            if self.pos + 2 > self.data.len() {
                break;
            }
            let code = self.read_u16_le()?;
            if code == 0 {
                break;
            }

            // Handle surrogate pairs
            if (0xD800..=0xDBFF).contains(&code) {
                if self.pos + 2 > self.data.len() {
                    break;
                }
                let low = self.read_u16_le()?;
                if (0xDC00..=0xDFFF).contains(&low) {
                    let code_point =
                        0x10000 + (((code - 0xD800) as u32) << 10) + ((low - 0xDC00) as u32);
                    if let Some(c) = char::from_u32(code_point) {
                        chars.push(c);
                    }
                }
            } else if let Some(c) = char::from_u32(code as u32) {
                chars.push(c);
            }
        }

        Ok(chars.into_iter().collect())
    }

    /// Parse all VGM commands from the data section
    pub fn parse_commands(&mut self, header: &VgmHeader) -> Result<Vec<VgmCommand>> {
        // Data starts at data_offset + 0x34
        let data_start = (header.data_offset as usize) + 0x34;
        self.seek(data_start);

        let mut commands = Vec::new();

        while !self.is_eof() {
            match self.parse_command()? {
                Some(cmd) => {
                    let is_end = matches!(cmd, VgmCommand::End);
                    commands.push(cmd);
                    if is_end {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(commands)
    }

    /// Parse a single VGM command
    fn parse_command(&mut self) -> Result<Option<VgmCommand>> {
        if self.is_eof() {
            return Ok(None);
        }

        let op = self.read_u8()?;

        let cmd = match op {
            opcode::GG_STEREO => {
                let data = self.read_u8()?;
                VgmCommand::GgStereo { data }
            }
            opcode::SN76489 => {
                let data = self.read_u8()?;
                VgmCommand::Sn76489Write { data }
            }
            opcode::YM2413 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2413Write { reg, data }
            }
            opcode::YM2612_PORT0 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2612Write {
                    port: 0,
                    reg,
                    data,
                }
            }
            opcode::YM2612_PORT1 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2612Write {
                    port: 1,
                    reg,
                    data,
                }
            }
            opcode::YM2151 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2151Write { reg, data }
            }
            opcode::YM2203 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2203Write { reg, data }
            }
            opcode::YM2608_PORT0 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2608Write {
                    port: 0,
                    reg,
                    data,
                }
            }
            opcode::YM2608_PORT1 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2608Write {
                    port: 1,
                    reg,
                    data,
                }
            }
            opcode::YM2610_PORT0 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2610Write {
                    port: 0,
                    reg,
                    data,
                }
            }
            opcode::YM2610_PORT1 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym2610Write {
                    port: 1,
                    reg,
                    data,
                }
            }
            opcode::YM3812 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym3812Write { reg, data }
            }
            opcode::YM3526 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ym3526Write { reg, data }
            }
            opcode::Y8950 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Y8950Write { reg, data }
            }
            opcode::YMZ280B => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ymz280bWrite { reg, data }
            }
            opcode::YMF262_PORT0 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ymf262Write {
                    port: 0,
                    reg,
                    data,
                }
            }
            opcode::YMF262_PORT1 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ymf262Write {
                    port: 1,
                    reg,
                    data,
                }
            }
            opcode::WAIT_NNNN => {
                let samples = self.read_u16_le()? as u32;
                VgmCommand::Wait { samples }
            }
            opcode::WAIT_60TH => VgmCommand::Wait { samples: 735 },
            opcode::WAIT_50TH => VgmCommand::Wait { samples: 882 },
            opcode::END => VgmCommand::End,
            opcode::DATA_BLOCK => {
                // 0x67 0x66 tt ss ss ss ss (data)
                let _compat = self.read_u8()?; // Should be 0x66
                let block_type = self.read_u8()?;
                let size = self.read_u32_le()?;
                // Skip the data block content
                let actual_size = (size & 0x7FFF_FFFF) as usize;
                if self.pos + actual_size <= self.data.len() {
                    self.pos += actual_size;
                }
                VgmCommand::DataBlock {
                    block_type,
                    size: Some(size),
                }
            }
            opcode::PCM_RAM_WRITE => {
                let _compat = self.read_u8()?; // Should be 0x66
                let chip_type = self.read_u8()?;
                let read_offset = self.read_u24_le()?;
                let write_offset = self.read_u24_le()?;
                let size = self.read_u24_le()?;
                VgmCommand::PcmRamWrite {
                    chip_type,
                    read_offset,
                    write_offset,
                    size,
                }
            }
            // Short wait (0x70-0x7F): wait n+1 samples
            0x70..=0x7F => {
                let n = (op - 0x70) as u32 + 1;
                VgmCommand::Wait { samples: n }
            }
            // YM2612 DAC write + wait (0x80-0x8F)
            0x80..=0x8F => {
                let wait = op - 0x80;
                VgmCommand::Ym2612Dac { data: 0x2A, wait }
            }
            opcode::DAC_STREAM_SETUP => {
                let stream_id = self.read_u8()?;
                let chip_type = self.read_u8()?;
                let port = self.read_u8()?;
                let reg = self.read_u8()?;
                VgmCommand::DacStreamSetup {
                    stream_id,
                    chip_type,
                    port,
                    reg,
                }
            }
            opcode::DAC_STREAM_DATA => {
                let stream_id = self.read_u8()?;
                let bank_id = self.read_u8()?;
                let step_base = self.read_u8()?;
                let step_size = self.read_u8()?;
                VgmCommand::DacStreamData {
                    stream_id,
                    bank_id,
                    step_base,
                    step_size,
                }
            }
            opcode::DAC_STREAM_FREQ => {
                let stream_id = self.read_u8()?;
                let frequency = self.read_u32_le()?;
                VgmCommand::DacStreamFreq {
                    stream_id,
                    frequency,
                }
            }
            opcode::DAC_STREAM_START => {
                let stream_id = self.read_u8()?;
                let data_start = self.read_u32_le()?;
                let length_mode = self.read_u8()?;
                let data_length = self.read_u32_le()?;
                VgmCommand::DacStreamStart {
                    stream_id,
                    data_start,
                    length_mode,
                    data_length,
                }
            }
            opcode::DAC_STREAM_STOP => {
                let stream_id = self.read_u8()?;
                VgmCommand::DacStreamStop { stream_id }
            }
            opcode::DAC_STREAM_FAST => {
                let stream_id = self.read_u8()?;
                let block_id = self.read_u16_le()?;
                let flags = self.read_u8()?;
                VgmCommand::DacStreamFast {
                    stream_id,
                    block_id,
                    flags,
                }
            }
            opcode::AY8910 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ay8910Write { reg, data }
            }
            // 0xBx commands (2 bytes each)
            0xB0 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Rf5c68Write { reg, data }
            }
            0xB1 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Rf5c164Write { reg, data }
            }
            0xB2 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                let data16 = ((reg as u16 & 0xF0) << 4) | (data as u16);
                let reg4 = reg & 0x0F;
                VgmCommand::PwmWrite {
                    reg: reg4,
                    data: data16,
                }
            }
            0xB3 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::GbDmgWrite { reg, data }
            }
            0xB4 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::NesApuWrite { reg, data }
            }
            0xB5 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::MultiPcmWrite { reg, data }
            }
            0xB6 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Upd7759Write { reg, data }
            }
            0xB7 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Okim6258Write { reg, data }
            }
            0xB8 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Okim6295Write { reg, data }
            }
            0xB9 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Huc6280Write { reg, data }
            }
            0xBA => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::K053260Write { reg, data }
            }
            0xBB => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::PokeyWrite { reg, data }
            }
            0xBC => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::WonderSwanWrite { reg, data }
            }
            0xBD => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Saa1099Write { reg, data }
            }
            0xBE => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Es5503Write { reg, data }
            }
            0xBF => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ga20Write { reg, data }
            }
            // 0xCx commands (3 bytes each)
            0xC0 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                // Sega PCM memory write
                VgmCommand::Unknown {
                    opcode: op,
                    bytes: vec![reg_lo, reg_hi, data],
                }
            }
            0xC1 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let _data = self.read_u8()?;
                VgmCommand::Rf5c68Write {
                    reg: reg_lo,
                    data: reg_hi,
                }
            }
            0xC2 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let _data = self.read_u8()?;
                VgmCommand::Rf5c164Write {
                    reg: reg_lo,
                    data: reg_hi,
                }
            }
            0xC3 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let _data = self.read_u8()?;
                VgmCommand::MultiPcmWrite {
                    reg: reg_lo,
                    data: reg_hi,
                }
            }
            0xC4 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                let data16 = ((reg_hi as u16) << 8) | (data as u16);
                VgmCommand::QsoundWrite {
                    reg: reg_lo,
                    data: data16,
                }
            }
            0xC5 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                let reg = ((reg_hi as u16) << 8) | (reg_lo as u16);
                VgmCommand::ScspWrite { reg, data }
            }
            0xC6 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let _data = self.read_u8()?;
                VgmCommand::WonderSwanWrite {
                    reg: reg_lo,
                    data: reg_hi,
                }
            }
            0xC7 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let _data = self.read_u8()?;
                VgmCommand::VsuWrite {
                    reg: reg_lo,
                    data: reg_hi,
                }
            }
            0xC8 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                let reg = ((reg_hi as u16) << 8) | (reg_lo as u16);
                VgmCommand::X1010Write { reg, data }
            }
            // 0xDx commands (3-4 bytes each)
            0xD0 => {
                let port = self.read_u8()?;
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ymf278Write { port, reg, data }
            }
            0xD1 => {
                let port = self.read_u8()?;
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Ymf271Write { port, reg, data }
            }
            0xD2 => {
                let reg = self.read_u8()?;
                let data = self.read_u8()?;
                let _extra = self.read_u8()?;
                VgmCommand::K051649Write { reg, data }
            }
            0xD3 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                let reg = ((reg_hi as u16) << 8) | (reg_lo as u16);
                VgmCommand::K054539Write { reg, data }
            }
            0xD4 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                let reg = ((reg_hi as u16) << 8) | (reg_lo as u16);
                VgmCommand::C140Write { reg, data }
            }
            0xD5 => {
                let reg = self.read_u8()?;
                let data_lo = self.read_u8()?;
                let data_hi = self.read_u8()?;
                let data = ((data_hi as u16) << 8) | (data_lo as u16);
                VgmCommand::Es5506Write { reg, data }
            }
            0xD6 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data = self.read_u8()?;
                VgmCommand::Es5506Write {
                    reg: reg_lo,
                    data: (reg_hi as u16) << 8 | (data as u16),
                }
            }
            // 0xE0: Seek to PCM data bank position
            opcode::SEEK_PCM => {
                let offset = self.read_u32_le()?;
                VgmCommand::SeekPcm { offset }
            }
            0xE1 => {
                let reg_lo = self.read_u8()?;
                let reg_hi = self.read_u8()?;
                let data_lo = self.read_u8()?;
                let data_hi = self.read_u8()?;
                let reg = ((reg_hi as u16) << 8) | (reg_lo as u16);
                let data = ((data_hi as u16) << 8) | (data_lo as u16);
                VgmCommand::C352Write { reg, data }
            }
            // Unknown command
            _ => {
                let size = command_size(op);
                let bytes = if size > 0 {
                    self.read_bytes(size)?
                } else {
                    vec![]
                };
                VgmCommand::Unknown { opcode: op, bytes }
            }
        };

        Ok(Some(cmd))
    }
}

/// Additional methods for VgmCommand
impl VgmCommand {
    /// Check if this is a wait command
    pub fn is_wait(&self) -> bool {
        matches!(self, VgmCommand::Wait { .. })
    }

    /// Get wait samples if this is a wait command
    pub fn wait_samples(&self) -> Option<u32> {
        match self {
            VgmCommand::Wait { samples } => Some(*samples),
            _ => None,
        }
    }
}
