//! VGM header definitions and writing

/// VGM file version
pub const VGM_VERSION: u32 = 0x161;

/// Maximum header size in 32-bit words
pub const VGM_MAX_HEADER: usize = 48;

/// Header size in bytes
pub const VGM_HEADER_SIZE: usize = VGM_MAX_HEADER * 4;

/// VGM header offsets (in bytes)
pub mod offset {
    /// "Vgm " identifier
    pub const IDENT: usize = 0x00;
    /// End of file offset (relative to 0x04)
    pub const EOF_OFFSET: usize = 0x04;
    /// Version number
    pub const VERSION: usize = 0x08;
    /// SN76489 clock
    pub const SN76489_CLOCK: usize = 0x0C;
    /// YM2413 clock
    pub const YM2413_CLOCK: usize = 0x10;
    /// GD3 offset (relative to 0x14)
    pub const GD3_OFFSET: usize = 0x14;
    /// Total samples
    pub const TOTAL_SAMPLES: usize = 0x18;
    /// Loop offset (relative to 0x1C)
    pub const LOOP_OFFSET: usize = 0x1C;
    /// Loop samples
    pub const LOOP_SAMPLES: usize = 0x20;
    /// Recording rate
    pub const RATE: usize = 0x24;
    /// SN76489 feedback
    pub const SN76489_FEEDBACK: usize = 0x28;
    /// SN76489 shift register width
    pub const SN76489_SHIFT_WIDTH: usize = 0x2A;
    /// SN76489 flags
    pub const SN76489_FLAGS: usize = 0x2B;
    /// YM2612 clock
    pub const YM2612_CLOCK: usize = 0x2C;
    /// YM2151 clock
    pub const YM2151_CLOCK: usize = 0x30;
    /// VGM data offset (relative to 0x34)
    pub const DATA_OFFSET: usize = 0x34;
    /// Sega PCM clock
    pub const SEGA_PCM_CLOCK: usize = 0x38;
    /// Sega PCM interface register
    pub const SEGA_PCM_INTERFACE: usize = 0x3C;
    /// YM2203 clock
    pub const YM2203_CLOCK: usize = 0x44;
    /// YM2608 clock
    pub const YM2608_CLOCK: usize = 0x48;
    /// YM2610/B clock
    pub const YM2610_CLOCK: usize = 0x4C;
    /// YM3812 clock
    pub const YM3812_CLOCK: usize = 0x50;
    /// YM3526 clock
    pub const YM3526_CLOCK: usize = 0x54;
    /// Y8950 clock
    pub const Y8950_CLOCK: usize = 0x58;
    /// YMF262 clock
    pub const YMF262_CLOCK: usize = 0x5C;
    /// YMF278B clock
    pub const YMF278B_CLOCK: usize = 0x60;
    /// YMF271 clock
    pub const YMF271_CLOCK: usize = 0x64;
    /// YMZ280B clock
    pub const YMZ280B_CLOCK: usize = 0x68;
    /// RF5C164 clock
    pub const RF5C164_CLOCK: usize = 0x6C;
    /// PWM clock
    pub const PWM_CLOCK: usize = 0x70;
    /// AY8910 clock
    pub const AY8910_CLOCK: usize = 0x74;
    /// AY8910 chip type
    pub const AY8910_TYPE: usize = 0x78;
    /// AY8910 flags
    pub const AY8910_FLAGS: usize = 0x79;
    /// YM2203/YM2608 flags
    pub const YM2203_FLAGS: usize = 0x7A;
    /// YM2608 flags
    pub const YM2608_FLAGS: usize = 0x7B;
    /// Volume modifier
    pub const VOLUME_MODIFIER: usize = 0x7C;
    /// Loop base
    pub const LOOP_BASE: usize = 0x7E;
    /// Loop modifier
    pub const LOOP_MODIFIER: usize = 0x7F;
    /// GameBoy DMG clock
    pub const GB_DMG_CLOCK: usize = 0x80;
    /// NES APU clock
    pub const NES_APU_CLOCK: usize = 0x84;
    /// MultiPCM clock
    pub const MULTI_PCM_CLOCK: usize = 0x88;
    /// uPD7759 clock
    pub const UPD7759_CLOCK: usize = 0x8C;
    /// OKIM6258 clock
    pub const OKIM6258_CLOCK: usize = 0x90;
    /// OKIM6258 flags
    pub const OKIM6258_FLAGS: usize = 0x94;
    /// K051649 clock
    pub const K051649_CLOCK: usize = 0x98;
    /// K054539 clock
    pub const K054539_CLOCK: usize = 0x9C;
    /// HuC6280 clock
    pub const HUC6280_CLOCK: usize = 0xA0;
    /// C140 clock
    pub const C140_CLOCK: usize = 0xA4;
    /// K053260 clock
    pub const K053260_CLOCK: usize = 0xA8;
    /// Pokey clock
    pub const POKEY_CLOCK: usize = 0xAC;
    /// QSound clock
    pub const QSOUND_CLOCK: usize = 0xB0;
}

/// VGM header structure
#[derive(Debug, Clone)]
pub struct VgmHeader {
    data: [u8; VGM_HEADER_SIZE],
}

impl VgmHeader {
    pub fn new() -> Self {
        let mut header = Self {
            data: [0; VGM_HEADER_SIZE],
        };

        // Write magic
        header.data[0..4].copy_from_slice(b"Vgm ");

        // Write version
        header.write_u32(offset::VERSION, VGM_VERSION);

        // Write data offset (relative to 0x34)
        header.write_u32(offset::DATA_OFFSET, (VGM_HEADER_SIZE - 0x34) as u32);

        header
    }

    pub fn write_u8(&mut self, offset: usize, value: u8) {
        if offset < VGM_HEADER_SIZE {
            self.data[offset] = value;
        }
    }

    pub fn write_u16(&mut self, offset: usize, value: u16) {
        if offset + 1 < VGM_HEADER_SIZE {
            self.data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
    }

    pub fn write_u32(&mut self, offset: usize, value: u32) {
        if offset + 3 < VGM_HEADER_SIZE {
            self.data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    pub fn write_i8(&mut self, offset: usize, value: i8) {
        self.write_u8(offset, value as u8);
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Default for VgmHeader {
    fn default() -> Self {
        Self::new()
    }
}
