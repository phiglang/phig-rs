use std::{fmt, io};

/// An error from parsing or serializing phig.
#[derive(Debug)]
pub enum Error {
    /// A syntax or semantic error with a human-readable message and optional byte position.
    Syntax {
        /// The error message.
        msg: String,
        /// Byte offset in the input where the error occurred, if applicable.
        pos: Option<usize>,
    },
    /// An IO error passed through from the underlying reader/writer.
    Io(io::Error),
}

impl Error {
    /// Create an error with no position.
    pub fn new(msg: impl Into<String>) -> Self {
        Error::Syntax {
            msg: msg.into(),
            pos: None,
        }
    }

    pub(crate) fn at(msg: impl Into<String>, pos: usize) -> Self {
        Error::Syntax {
            msg: msg.into(),
            pos: Some(pos),
        }
    }

    /// The error message, if this is a syntax error.
    pub fn msg(&self) -> Option<&str> {
        match self {
            Error::Syntax { msg, .. } => Some(msg),
            Error::Io(_) => None,
        }
    }

    /// Byte offset in the input where the error occurred, if applicable.
    pub fn pos(&self) -> Option<usize> {
        match self {
            Error::Syntax { pos, .. } => *pos,
            Error::Io(_) => None,
        }
    }

    /// If this error wraps an [`io::Error`], return it.
    pub fn into_io(self) -> Option<io::Error> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax {
                msg,
                pos: Some(pos),
            } => write!(f, "at position {}: {}", pos, msg),
            Error::Syntax { msg, pos: None } => write!(f, "{}", msg),
            Error::Io(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

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

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
