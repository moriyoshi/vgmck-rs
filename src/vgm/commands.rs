//! VGM command definitions and parsing

use serde::Serialize;

/// VGM command opcodes
pub mod opcode {
    pub const GG_STEREO: u8 = 0x4F;
    pub const SN76489: u8 = 0x50;
    pub const YM2413: u8 = 0x51;
    pub const YM2612_PORT0: u8 = 0x52;
    pub const YM2612_PORT1: u8 = 0x53;
    pub const YM2151: u8 = 0x54;
    pub const YM2203: u8 = 0x55;
    pub const YM2608_PORT0: u8 = 0x56;
    pub const YM2608_PORT1: u8 = 0x57;
    pub const YM2610_PORT0: u8 = 0x58;
    pub const YM2610_PORT1: u8 = 0x59;
    pub const YM3812: u8 = 0x5A;
    pub const YM3526: u8 = 0x5B;
    pub const Y8950: u8 = 0x5C;
    pub const YMZ280B: u8 = 0x5D;
    pub const YMF262_PORT0: u8 = 0x5E;
    pub const YMF262_PORT1: u8 = 0x5F;
    pub const WAIT_NNNN: u8 = 0x61;
    pub const WAIT_60TH: u8 = 0x62;
    pub const WAIT_50TH: u8 = 0x63;
    pub const END: u8 = 0x66;
    pub const DATA_BLOCK: u8 = 0x67;
    pub const PCM_RAM_WRITE: u8 = 0x68;
    pub const AY8910: u8 = 0xA0;
    pub const DAC_STREAM_SETUP: u8 = 0x90;
    pub const DAC_STREAM_DATA: u8 = 0x91;
    pub const DAC_STREAM_FREQ: u8 = 0x92;
    pub const DAC_STREAM_START: u8 = 0x93;
    pub const DAC_STREAM_STOP: u8 = 0x94;
    pub const DAC_STREAM_FAST: u8 = 0x95;
    pub const SEEK_PCM: u8 = 0xE0;
}

/// A parsed VGM command
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum VgmCommand {
    /// Game Gear PSG stereo control
    GgStereo { data: u8 },
    /// SN76489 PSG write
    Sn76489Write { data: u8 },
    /// YM2413 (OPLL) write
    Ym2413Write { reg: u8, data: u8 },
    /// YM2612 (OPN2) write
    Ym2612Write { port: u8, reg: u8, data: u8 },
    /// YM2151 (OPM) write
    Ym2151Write { reg: u8, data: u8 },
    /// YM2203 (OPN) write
    Ym2203Write { reg: u8, data: u8 },
    /// YM2608 (OPNA) write
    Ym2608Write { port: u8, reg: u8, data: u8 },
    /// YM2610 (OPNB) write
    Ym2610Write { port: u8, reg: u8, data: u8 },
    /// YM3812 (OPL2) write
    Ym3812Write { reg: u8, data: u8 },
    /// YM3526 (OPL) write
    Ym3526Write { reg: u8, data: u8 },
    /// Y8950 (MSX-Audio) write
    Y8950Write { reg: u8, data: u8 },
    /// YMZ280B write
    Ymz280bWrite { reg: u8, data: u8 },
    /// YMF262 (OPL3) write
    Ymf262Write { port: u8, reg: u8, data: u8 },
    /// AY-3-8910 write
    Ay8910Write { reg: u8, data: u8 },
    /// Wait N samples
    Wait { samples: u32 },
    /// End of sound data
    End,
    /// Data block
    DataBlock {
        block_type: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<u32>,
    },
    /// PCM RAM write
    PcmRamWrite {
        chip_type: u8,
        read_offset: u32,
        write_offset: u32,
        size: u32,
    },
    /// YM2612 DAC write with wait
    Ym2612Dac { data: u8, wait: u8 },
    /// DAC stream setup
    DacStreamSetup {
        stream_id: u8,
        chip_type: u8,
        port: u8,
        reg: u8,
    },
    /// DAC stream set data
    DacStreamData {
        stream_id: u8,
        bank_id: u8,
        step_base: u8,
        step_size: u8,
    },
    /// DAC stream set frequency
    DacStreamFreq { stream_id: u8, frequency: u32 },
    /// DAC stream start
    DacStreamStart {
        stream_id: u8,
        data_start: u32,
        length_mode: u8,
        data_length: u32,
    },
    /// DAC stream stop
    DacStreamStop { stream_id: u8 },
    /// DAC stream fast start
    DacStreamFast {
        stream_id: u8,
        block_id: u16,
        flags: u8,
    },
    /// Seek to PCM data bank position
    SeekPcm { offset: u32 },
    /// RF5C68 write
    Rf5c68Write { reg: u8, data: u8 },
    /// RF5C164 write
    Rf5c164Write { reg: u8, data: u8 },
    /// PWM write
    PwmWrite { reg: u8, data: u16 },
    /// GameBoy DMG write
    GbDmgWrite { reg: u8, data: u8 },
    /// NES APU write
    NesApuWrite { reg: u8, data: u8 },
    /// MultiPCM write
    MultiPcmWrite { reg: u8, data: u8 },
    /// uPD7759 write
    Upd7759Write { reg: u8, data: u8 },
    /// OKIM6258 write
    Okim6258Write { reg: u8, data: u8 },
    /// OKIM6295 write
    Okim6295Write { reg: u8, data: u8 },
    /// K051649 (SCC) write
    K051649Write { reg: u8, data: u8 },
    /// K054539 write
    K054539Write { reg: u16, data: u8 },
    /// HuC6280 write
    Huc6280Write { reg: u8, data: u8 },
    /// C140 write
    C140Write { reg: u16, data: u8 },
    /// K053260 write
    K053260Write { reg: u8, data: u8 },
    /// Pokey write
    PokeyWrite { reg: u8, data: u8 },
    /// QSound write
    QsoundWrite { reg: u8, data: u16 },
    /// SCSP write
    ScspWrite { reg: u16, data: u8 },
    /// WonderSwan write
    WonderSwanWrite { reg: u8, data: u8 },
    /// VSU write
    VsuWrite { reg: u8, data: u8 },
    /// SAA1099 write
    Saa1099Write { reg: u8, data: u8 },
    /// ES5503 write
    Es5503Write { reg: u8, data: u8 },
    /// ES5506 write
    Es5506Write { reg: u8, data: u16 },
    /// X1-010 write
    X1010Write { reg: u16, data: u8 },
    /// C352 write
    C352Write { reg: u16, data: u16 },
    /// GA20 write
    Ga20Write { reg: u8, data: u8 },
    /// Mikey write
    MikeyWrite { reg: u8, data: u8 },
    /// YMF278B (OPL4) write
    Ymf278Write { port: u8, reg: u8, data: u8 },
    /// YMF271 (OPX) write
    Ymf271Write { port: u8, reg: u8, data: u8 },
    /// Unknown command
    Unknown { opcode: u8, bytes: Vec<u8> },
}

/// Get the number of bytes to read after the opcode for a command
pub fn command_size(opcode: u8) -> usize {
    match opcode {
        // 0 bytes after opcode
        0x62 | 0x63 | 0x66 => 0,
        // 1 byte after opcode
        0x4F | 0x50 => 1,
        // 2 bytes after opcode
        0x51 | 0x52 | 0x53 | 0x54 | 0x55 | 0x56 | 0x57 | 0x58 | 0x59 | 0x5A | 0x5B | 0x5C
        | 0x5D | 0x5E | 0x5F | 0x61 | 0xA0 | 0xB0..=0xBF => 2,
        // 3 bytes after opcode
        0xC0..=0xC8 => 3,
        // 4 bytes after opcode
        0xD0..=0xD6 | 0xE0 | 0xE1 => 4,
        // Short wait (0x70-0x7F) - 0 bytes
        0x70..=0x7F => 0,
        // YM2612 DAC (0x80-0x8F) - 0 bytes
        0x80..=0x8F => 0,
        // Variable length commands
        0x67 => 0, // Data block - size is in the data itself
        0x68 => 11, // PCM RAM write
        0x90 => 4, // DAC stream setup
        0x91 => 4, // DAC stream data
        0x92 => 5, // DAC stream freq
        0x93 => 10, // DAC stream start
        0x94 => 1, // DAC stream stop
        0x95 => 4, // DAC stream fast
        // Reserved/unknown
        _ => 0,
    }
}
