//! JSON serialization types for VGM data

use super::commands::VgmCommand;
use super::reader::{ChipInfo, Gd3Info, VgmHeader};
use serde::Serialize;
use std::collections::HashMap;

/// Top-level JSON structure for a VGM file
#[derive(Debug, Clone, Serialize)]
pub struct VgmJson {
    /// VGM version as a string (e.g., "1.61")
    pub version: String,
    /// Header information
    pub header: VgmHeaderJson,
    /// GD3 metadata (if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gd3: Option<Gd3Json>,
    /// VGM commands
    pub commands: Vec<VgmCommand>,
}

/// JSON representation of VGM header
#[derive(Debug, Clone, Serialize)]
pub struct VgmHeaderJson {
    /// Total samples in the file
    pub total_samples: u32,
    /// Loop offset (if looping)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_offset: Option<u32>,
    /// Number of samples in the loop
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_samples: Option<u32>,
    /// Playback rate (Hz)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u32>,
    /// Volume modifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_modifier: Option<i8>,
    /// Loop base
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_base: Option<i8>,
    /// Loop modifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_modifier: Option<u8>,
    /// Sound chips used in this file
    pub chips: HashMap<String, ChipJson>,
}

/// JSON representation of chip information
#[derive(Debug, Clone, Serialize)]
pub struct ChipJson {
    /// Clock frequency in Hz
    pub clock: u32,
    /// Whether this is a dual-chip configuration
    #[serde(skip_serializing_if = "is_false")]
    pub dual: bool,
    /// Extra chip-specific parameters
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, u32>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// JSON representation of GD3 metadata
#[derive(Debug, Clone, Serialize)]
pub struct Gd3Json {
    /// Track title (English)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Track title (Japanese)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title_jp: String,
    /// Game name (English)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub game: String,
    /// Game name (Japanese)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub game_jp: String,
    /// System name (English)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub system: String,
    /// System name (Japanese)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub system_jp: String,
    /// Composer name (English)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub composer: String,
    /// Composer name (Japanese)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub composer_jp: String,
    /// Release date
    #[serde(skip_serializing_if = "String::is_empty")]
    pub date: String,
    /// VGM converter/ripper
    #[serde(skip_serializing_if = "String::is_empty")]
    pub converter: String,
    /// Additional notes
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

impl VgmJson {
    /// Create a VgmJson from parsed VGM data
    pub fn new(header: &VgmHeader, gd3: Option<&Gd3Info>, commands: Vec<VgmCommand>) -> Self {
        Self {
            version: format_version(header.version),
            header: VgmHeaderJson::from(header),
            gd3: gd3.map(Gd3Json::from),
            commands,
        }
    }
}

impl From<&VgmHeader> for VgmHeaderJson {
    fn from(header: &VgmHeader) -> Self {
        let chips = header
            .chips
            .iter()
            .map(|(name, info)| (name.clone(), ChipJson::from(info)))
            .collect();

        Self {
            total_samples: header.total_samples,
            loop_offset: if header.loop_offset != 0 {
                Some(header.loop_offset)
            } else {
                None
            },
            loop_samples: if header.loop_samples != 0 {
                Some(header.loop_samples)
            } else {
                None
            },
            rate: if header.rate != 0 {
                Some(header.rate)
            } else {
                None
            },
            volume_modifier: if header.volume_modifier != 0 {
                Some(header.volume_modifier)
            } else {
                None
            },
            loop_base: if header.loop_base != 0 {
                Some(header.loop_base)
            } else {
                None
            },
            loop_modifier: if header.loop_modifier != 0 {
                Some(header.loop_modifier)
            } else {
                None
            },
            chips,
        }
    }
}

impl From<&ChipInfo> for ChipJson {
    fn from(info: &ChipInfo) -> Self {
        Self {
            clock: info.clock,
            dual: info.dual,
            extra: info.extra.clone(),
        }
    }
}

impl From<&Gd3Info> for Gd3Json {
    fn from(gd3: &Gd3Info) -> Self {
        Self {
            title: gd3.title.clone(),
            title_jp: gd3.title_jp.clone(),
            game: gd3.game.clone(),
            game_jp: gd3.game_jp.clone(),
            system: gd3.system.clone(),
            system_jp: gd3.system_jp.clone(),
            composer: gd3.composer.clone(),
            composer_jp: gd3.composer_jp.clone(),
            date: gd3.date.clone(),
            converter: gd3.converter.clone(),
            notes: gd3.notes.clone(),
        }
    }
}

/// Format a BCD version number as a string
fn format_version(version: u32) -> String {
    let major = (version >> 8) & 0xFF;
    let minor = version & 0xFF;
    format!("{}.{:02x}", major, minor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_version() {
        assert_eq!(format_version(0x161), "1.61");
        assert_eq!(format_version(0x150), "1.50");
        assert_eq!(format_version(0x100), "1.00");
        assert_eq!(format_version(0x171), "1.71");
    }
}
