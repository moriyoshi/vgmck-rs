//! Sample loading and processing

use crate::error::{Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Sample loader for PCM data
#[derive(Debug)]
pub struct SampleLoader {
    /// Sample ID
    pub id: u8,
    /// File handle (if loading from file)
    file: Option<File>,
    /// In-memory data (if generated)
    data: Option<Vec<u8>>,
    /// Bits per sample in file (8 or 16, negative for signed)
    pub bit_file: i8,
    /// Bits per sample for conversion
    pub bit_conv: i8,
    /// Endianness (false = little, true = big)
    pub big_endian: bool,
    /// Total sample count
    pub count: i64,
    /// Loop mode (0 = off, 1 = on, 2 = bidirectional)
    pub loop_mode: u8,
    /// Loop start point
    pub loop_start: i64,
    /// Loop end point
    pub loop_end: i64,
    /// Sample clock rate
    pub clock: u32,
    /// Header size
    pub header_size: u8,
    /// Data start offset in file
    data_start: i64,
    /// Header start offset
    #[allow(dead_code)]
    header_start: i64,
}

impl SampleLoader {
    /// Open a sample file
    pub fn open(path: &Path, clock: u32, bits: i8) -> Result<Self> {
        let file = File::open(path)?;
        let mut loader = Self {
            id: 0,
            file: Some(file),
            data: None,
            bit_file: bits,
            bit_conv: bits,
            big_endian: false,
            count: 0,
            loop_mode: 0,
            loop_start: 0,
            loop_end: 0,
            clock,
            header_size: 0,
            data_start: 0,
            header_start: 0,
        };
        loader.read_header()?;
        Ok(loader)
    }

    /// Create from in-memory data
    pub fn from_data(data: Vec<u8>, bits: i8) -> Self {
        let count = data.len() as i64 / if bits.abs() == 16 { 2 } else { 1 };
        Self {
            id: 0,
            file: None,
            data: Some(data),
            bit_file: bits,
            bit_conv: bits,
            big_endian: false,
            count,
            loop_mode: 0,
            loop_start: 0,
            loop_end: 0,
            clock: 0,
            header_size: 0,
            data_start: 0,
            header_start: 0,
        }
    }

    fn read_header(&mut self) -> Result<()> {
        let file = self.file.as_mut().ok_or_else(|| {
            Error::Sample("No file handle".to_string())
        })?;

        // Get file size
        let size = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(0))?;

        // For raw files, use the whole file
        self.count = size as i64;
        self.data_start = 0;

        // Adjust count for sample size
        if self.bit_file == 16 || self.bit_file == -16 {
            self.count /= 2;
        }

        Ok(())
    }

    /// Read samples from file
    pub fn read(&mut self, dest: &mut [u8], start: i64, count: i64) -> Result<()> {
        let sample_size = if self.bit_file.abs() == 16 { 2 } else { 1 };

        if let Some(file) = &mut self.file {
            file.seek(SeekFrom::Start(
                (self.data_start + start * sample_size) as u64,
            ))?;
            let bytes_to_read = (count * sample_size) as usize;
            file.read_exact(&mut dest[..bytes_to_read])?;
        } else if let Some(data) = &self.data {
            let start_byte = (start * sample_size) as usize;
            let end_byte = start_byte + (count * sample_size) as usize;
            dest[..(end_byte - start_byte)]
                .copy_from_slice(&data[start_byte..end_byte]);
        }

        Ok(())
    }
}

/// Generate sine wave sample data
pub fn generate_sine(length: usize, amplitudes: &[(f64, f64)], signed: bool) -> Vec<i16> {
    use std::f64::consts::TAU;

    let mut out = vec![0i16; length];

    for (amplitude, period) in amplitudes {
        let freq = TAU / period;
        for (i, sample) in out.iter_mut().enumerate() {
            let val = ((freq * i as f64).sin() * amplitude) as i16;
            *sample = sample.saturating_add(val);
        }
    }

    if !signed {
        for sample in &mut out {
            *sample ^= 0x8000u16 as i16;
        }
    }

    out
}
