use std::fmt;

/// An error from parsing or serializing phig.
///
/// Contains a human-readable message and an optional byte position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The error message.
    pub msg: String,
    /// Byte offset in the input where the error occurred, if applicable.
    pub pos: Option<usize>,
}

impl Error {
    /// Create an error with no position.
    pub fn new(msg: impl Into<String>) -> Self {
        Error {
            msg: msg.into(),
            pos: None,
        }
    }

    pub(crate) fn at(msg: impl Into<String>, pos: usize) -> Self {
        Error {
            msg: msg.into(),
            pos: Some(pos),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.pos {
            Some(pos) => write!(f, "at position {}: {}", pos, self.msg),
            None => write!(f, "{}", self.msg),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(msg.to_string())
    }
}

#[cfg(feature = "serde")]
impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(msg.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::new(e.to_string())
    }
}
