use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "vgmck")]
#[command(version = "0.1.0")]
#[command(about = "MML to VGM compiler", long_about = None)]
struct Args {
    /// Output VGM file
    #[arg(required_unless_present = "list_chips")]
    output: Option<PathBuf>,

    /// Input MML file (reads from stdin if not specified)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// List available sound chips
    #[arg(short = 'L', long)]
    list_chips: bool,
}

fn main() -> Result<(), vgmck::Error> {
    let args = Args::parse();

    if args.list_chips {
        for name in vgmck::chips::list_chips() {
            println!("{}", name);
        }
        return Ok(());
    }

    let output = args.output.expect("output is required when not listing chips");

    let mut compiler = vgmck::Compiler::new();

    match &args.input {
        Some(path) => {
            // Use compile_file to properly resolve #INCLUDE paths
            compiler.compile_file(path, &output)?;
        }
        None => {
            // Read from stdin (no base path for includes)
            compiler.compile(std::io::stdin(), &output)?;
        }
    }

    Ok(())
}
