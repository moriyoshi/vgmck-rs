//! VGM file writer

use super::delay;
use super::gd3;
use super::header::{offset, VgmHeader, VGM_HEADER_SIZE};
use crate::compiler::Gd3Metadata;
use crate::error::Result;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

/// VGM file writer
pub struct VgmWriter {
    file: File,
    header: VgmHeader,
    /// Current position in data section
    data_pos: u64,
    /// Loop offset (position where loop starts)
    loop_offset: Option<u64>,
}

impl VgmWriter {
    /// Create a new VGM writer
    pub fn new(path: &Path) -> Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            file,
            header: VgmHeader::new(),
            data_pos: VGM_HEADER_SIZE as u64,
            loop_offset: None,
        })
    }

    /// Write the VGM header (call at start)
    pub fn write_header(&mut self) -> Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(self.header.as_bytes())?;
        Ok(())
    }

    /// Set a chip clock in the header
    pub fn set_chip_clock(&mut self, offset: usize, clock: u32) {
        self.header.write_u32(offset, clock);
    }

    /// Set total samples
    pub fn set_total_samples(&mut self, samples: u32) {
        self.header.write_u32(offset::TOTAL_SAMPLES, samples);
    }

    /// Set loop samples
    pub fn set_loop_samples(&mut self, samples: u32) {
        self.header.write_u32(offset::LOOP_SAMPLES, samples);
    }

    /// Set recording rate
    pub fn set_rate(&mut self, rate: u32) {
        self.header.write_u32(offset::RATE, rate);
    }

    /// Set volume modifier
    pub fn set_volume_modifier(&mut self, vol: i8) {
        self.header.write_i8(offset::VOLUME_MODIFIER, vol);
    }

    /// Set loop base
    pub fn set_loop_base(&mut self, base: i8) {
        self.header.write_i8(offset::LOOP_BASE, base);
    }

    /// Set loop modifier
    pub fn set_loop_modifier(&mut self, modifier: u8) {
        self.header.write_u8(offset::LOOP_MODIFIER, modifier);
    }

    /// Mark current position as loop start
    pub fn mark_loop_start(&mut self) {
        self.loop_offset = Some(self.data_pos);
    }

    /// Write raw bytes to data section
    pub fn write_data(&mut self, data: &[u8]) -> Result<()> {
        self.file.seek(SeekFrom::Start(self.data_pos))?;
        self.file.write_all(data)?;
        self.data_pos += data.len() as u64;
        Ok(())
    }

    /// Write a single byte command
    pub fn write_byte(&mut self, byte: u8) -> Result<()> {
        self.write_data(&[byte])
    }

    /// Write a delay
    pub fn write_delay(&mut self, samples: u64) -> Result<()> {
        let commands = delay::generate_delay(samples);
        self.write_data(&commands)
    }

    /// Write end of data marker
    pub fn write_end(&mut self) -> Result<()> {
        self.write_byte(delay::cmd::END)
    }

    /// Write GD3 tag and finalize file
    pub fn finalize(&mut self, metadata: &Gd3Metadata) -> Result<()> {
        // Write end marker
        self.write_end()?;

        // Record GD3 offset
        let gd3_offset = self.data_pos;

        // Write GD3 data
        let gd3_data = gd3::generate_gd3(metadata);
        if !gd3_data.is_empty() {
            self.write_data(&gd3_data)?;
            // GD3 offset is relative to 0x14
            self.header
                .write_u32(offset::GD3_OFFSET, (gd3_offset - 0x14) as u32);
        }

        // Record end of file offset (relative to 0x04)
        let eof_offset = self.data_pos - 0x04;
        self.header.write_u32(offset::EOF_OFFSET, eof_offset as u32);

        // Set loop offset if we have one (relative to 0x1C)
        if let Some(loop_pos) = self.loop_offset {
            self.header
                .write_u32(offset::LOOP_OFFSET, (loop_pos - 0x1C) as u32);
        }

        // Rewrite header with updated values
        self.write_header()?;

        self.file.flush()?;
        Ok(())
    }

    /// Get current data position
    pub fn position(&self) -> u64 {
        self.data_pos
    }

    /// Get mutable reference to header
    pub fn header_mut(&mut self) -> &mut VgmHeader {
        &mut self.header
    }
}
