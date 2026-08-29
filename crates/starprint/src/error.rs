//! Error types for the `starprint` crate.

use std::fmt;
use std::io;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// From the transport.
    Io(io::Error),
    /// A payload or image breaks a constraint described in `reason`.
    InvalidData { reason: String },
    /// A payload or image exceeds what the command or head can take.
    DataTooLong { max: usize, actual: usize },
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
