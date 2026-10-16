//! Errors for format decoders. Decoders take `&[u8]` and return these instead of panicking.

use std::fmt;

/// Malformed, truncated or oversized input to a format decoder.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormatError {
    /// The input is shorter than the decoder needs.
    Truncated {
        what: &'static str,
        need: usize,
        have: usize,
    },
    /// The compression type byte is not one this crate knows.
    UnknownCodec(u8),
    /// The stream is internally inconsistent.
    Malformed(&'static str),
    /// The declared output size exceeds the caller's limit.
    TooLarge { size: usize, max: usize },
    /// A field holds a value the format does not allow.
    Invalid { what: &'static str, detail: String },
}

impl FormatError {
    /// Shorthand for [`FormatError::Invalid`].
    pub fn invalid(what: &'static str, detail: impl Into<String>) -> Self {
        FormatError::Invalid {
            what,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Truncated { what, need, have } => {
                write!(f, "{what}: need {need} bytes, have {have}")
            }
            FormatError::UnknownCodec(tag) => write!(f, "unknown compression type {tag:#04x}"),
            FormatError::Malformed(what) => write!(f, "malformed: {what}"),
            FormatError::TooLarge { size, max } => {
                write!(f, "declared size {size} exceeds limit {max}")
            }
            FormatError::Invalid { what, detail } => write!(f, "{what}: {detail}"),
        }
    }
}

impl std::error::Error for FormatError {}
