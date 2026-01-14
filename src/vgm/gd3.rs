//! GD3 (Game Description 3) tag handling

use crate::compiler::Gd3Metadata;

/// GD3 tag magic
const GD3_MAGIC: &[u8; 4] = b"Gd3 ";

/// GD3 version (1.0)
const GD3_VERSION: u32 = 0x00000100;

/// Generate GD3 tag data
pub fn generate_gd3(metadata: &Gd3Metadata) -> Vec<u8> {
    let mut data = Vec::new();

    // Write magic
    data.extend_from_slice(GD3_MAGIC);

    // Write version
    data.extend_from_slice(&GD3_VERSION.to_le_bytes());

    // Placeholder for data size (will be filled later)
    let size_offset = data.len();
    data.extend_from_slice(&0u32.to_le_bytes());

    let strings_start = data.len();

    // Write strings in order:
    // 0: Track name (English)
    // 1: Track name (Japanese)
    // 2: Game name (English)
    // 3: Game name (Japanese)
    // 4: System name (English)
    // 5: System name (Japanese)
    // 6: Track author (English)
    // 7: Track author (Japanese)
    // 8: Release date
    // 9: VGM converter
    // 10: Notes

    write_utf16_string(&mut data, &metadata.title_en);
    write_utf16_string(&mut data, &metadata.title_jp);
    write_utf16_string(&mut data, &metadata.game_en);
    write_utf16_string(&mut data, &metadata.game_jp);
    write_utf16_string(&mut data, &metadata.system_en);
    write_utf16_string(&mut data, &metadata.system_jp);
    write_utf16_string(&mut data, &metadata.composer_en);
    write_utf16_string(&mut data, &metadata.composer_jp);
    write_utf16_string(&mut data, &metadata.date);
    write_utf16_string(&mut data, &metadata.converter);
    write_utf16_string(&mut data, &metadata.notes);

    // Fill in size
    let strings_size = (data.len() - strings_start) as u32;
    data[size_offset..size_offset + 4].copy_from_slice(&strings_size.to_le_bytes());

    data
}

/// Write a UTF-16LE null-terminated string
fn write_utf16_string(data: &mut Vec<u8>, s: &str) {
    for c in s.chars() {
        let code = c as u32;
        if code <= 0xFFFF {
            // BMP character
            data.push((code & 0xFF) as u8);
            data.push(((code >> 8) & 0xFF) as u8);
        } else {
            // Surrogate pair for characters outside BMP
            let code = code - 0x10000;
            let high = 0xD800 + ((code >> 10) & 0x3FF);
            let low = 0xDC00 + (code & 0x3FF);
            data.push((high & 0xFF) as u8);
            data.push(((high >> 8) & 0xFF) as u8);
            data.push((low & 0xFF) as u8);
            data.push(((low >> 8) & 0xFF) as u8);
        }
    }
    // Null terminator
    data.push(0);
    data.push(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_ascii() {
        let mut data = Vec::new();
        write_utf16_string(&mut data, "ABC");
        assert_eq!(data, vec![0x41, 0x00, 0x42, 0x00, 0x43, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_utf16_japanese() {
        let mut data = Vec::new();
        write_utf16_string(&mut data, "あ");
        // U+3042 = hiragana A
        assert_eq!(data, vec![0x42, 0x30, 0x00, 0x00]);
    }
}
