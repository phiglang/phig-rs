use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub msg: String,
    pub pos: Option<usize>,
}

impl Error {
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

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(msg.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(msg.to_string())
    }
}
