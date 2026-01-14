use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("VGM parse error: {0}")]
    VgmParse(String),

    #[error("Unknown chip: {0}")]
    UnknownChip(String),

    #[error("Channel '{0}' not declared before use")]
    UndeclaredChannel(char),

    #[error("Invalid channel: '{0}'")]
    InvalidChannel(char),

    #[error("Envelope error: {0}")]
    Envelope(String),

    #[error("Sample error: {0}")]
    Sample(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
