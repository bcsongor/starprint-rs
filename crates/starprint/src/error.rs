//! Error types for the `starprint` crate.

use std::fmt;
use std::io;

/// Convenience alias for results produced by this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The error type for all fallible operations in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An I/O error raised by the underlying transport (e.g. a TCP failure).
    Io(io::Error),
    /// Barcode or 2D-code payload contains bytes the selected symbology
    /// cannot encode.
    InvalidData {
        /// Human-readable description of the constraint that was violated.
        reason: String,
    },
    /// A payload exceeds the maximum length the command can carry.
    DataTooLong {
        /// Maximum number of bytes accepted by the command.
        max: usize,
        /// Actual number of bytes supplied.
        actual: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "transport I/O error: {err}"),
            Self::InvalidData { reason } => write!(f, "invalid data: {reason}"),
            Self::DataTooLong { max, actual } => {
                write!(
                    f,
                    "data too long: {actual} bytes exceeds the maximum of {max}"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}
