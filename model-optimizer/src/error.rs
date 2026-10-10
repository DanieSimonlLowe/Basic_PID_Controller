use std::fmt;

/// All failure modes of the program.
#[derive(Debug)]
pub enum FitError {
    /// Could not read the structlog file.
    Log(String),
    /// The data is unusable (empty, non-finite, no input changes, ...).
    Data(String),
    /// Bad command-line / configuration values.
    Config(String),
    /// A calculation produced an invalid or non-finite result.
    Numeric(String),
    /// The optimiser failed or returned something unusable.
    Optimiser(String),
}

impl fmt::Display for FitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitError::Log(m) => write!(f, "log error: {m}"),
            FitError::Data(m) => write!(f, "data error: {m}"),
            FitError::Config(m) => write!(f, "configuration error: {m}"),
            FitError::Numeric(m) => write!(f, "numerical error: {m}"),
            FitError::Optimiser(m) => write!(f, "optimiser error: {m}"),
        }
    }
}

impl std::error::Error for FitError {}
