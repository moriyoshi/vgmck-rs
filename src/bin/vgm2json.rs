//! VGM to JSON converter

use clap::Parser;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use vgmck::vgm::{VgmJson, VgmReader};

#[derive(Parser, Debug)]
#[command(name = "vgm2json")]
#[command(version = "0.1.0")]
#[command(about = "Convert VGM/VGZ files to JSON", long_about = None)]
struct Args {
    /// Input VGM or VGZ file
    input: PathBuf,

    /// Output JSON file (writes to stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output compact JSON (default is pretty-printed)
    #[arg(short, long)]
    compact: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Read input file
    let data = read_vgm_file(&args.input)?;

    // Parse VGM
    let mut reader = VgmReader::new(&data);

    let header = reader.parse_header()?;
    let gd3 = reader.parse_gd3(&header)?;
    let commands = reader.parse_commands(&header)?;

    // Create JSON structure
    let vgm_json = VgmJson::new(&header, gd3.as_ref(), commands);

    // Serialize to JSON
    let json_string = if args.compact {
        serde_json::to_string(&vgm_json)?
    } else {
        serde_json::to_string_pretty(&vgm_json)?
    };

    // Write output
    match args.output {
        Some(path) => {
            let mut file = File::create(path)?;
            file.write_all(json_string.as_bytes())?;
            file.write_all(b"\n")?;
        }
        None => {
            println!("{}", json_string);
        }
    }

    Ok(())
}

/// Read a VGM or VGZ file, decompressing if necessary
fn read_vgm_file(path: &PathBuf) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;

    // Check if it's a gzip file by extension or magic
    let is_gzip = path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("vgz") || ext.eq_ignore_ascii_case("gz"))
        .unwrap_or(false);

    if is_gzip {
        // Decompress gzip data
        let mut decoder = GzDecoder::new(file);
        let mut data = Vec::new();
        decoder.read_to_end(&mut data)?;
        Ok(data)
    } else {
        // Read raw VGM data
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        // Check for gzip magic (0x1f 0x8b) even if extension doesn't indicate it
        if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
            let cursor = std::io::Cursor::new(data);
            let mut decoder = GzDecoder::new(cursor);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        } else {
            Ok(data)
        }
    }
}
