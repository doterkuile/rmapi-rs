use clap;
use rmapi;
use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Rmapi(rmapi::Error),
    Clap(clap::Error),
    TokenFileNotFound,
    TokenFileInvalid,
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref err) => err.fmt(f),
            Error::Rmapi(ref err) => err.fmt(f),
            Error::Clap(ref err) => err.fmt(f),
            Error::TokenFileNotFound => write!(
                f,
                "Token file not found. Please use rmapi register <code> to create one"
            ),
            Error::TokenFileInvalid => write!(
                f,
                "Token file is not valid. Please use rmapi register <code> to create one"
            ),
            Error::Message(ref msg) => write!(f, "{}", msg),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::Io(ref err) => Some(err),
            Error::Rmapi(ref err) => Some(err),
            Error::Clap(ref err) => Some(err),
            Error::TokenFileNotFound => None,
            Error::TokenFileInvalid => None,
            Error::Message(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<rmapi::Error> for Error {
    fn from(err: rmapi::Error) -> Error {
        Error::Rmapi(err)
    }
}

impl From<clap::Error> for Error {
    fn from(err: clap::Error) -> Error {
        Error::Clap(err)
    }
}
