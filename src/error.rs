//! Errors for `frippy` crate using `failure`.

use std::io::Error as IoError;
use std::str::Utf8Error;
use irc::error::IrcError;
use reqwest::Error as ReqwestError;
use r2d2::Error as R2d2Error;

/// The main crate-wide error type.
#[derive(Debug, Fail)]
pub enum FrippyError {
    /// A plugin error
    #[fail(display = "A plugin error occured")]
    Plugin(#[cause] PluginError),

    /// An IRC error
    #[fail(display = "An IRC error occured")]
    Irc(#[cause] IrcError),

    /// Missing config error
    #[fail(display = "No config file was found")]
    MissingConfig,

    /// A reqwest error
    #[fail(display = "A reqwest error occured")]
    Reqwest(#[cause] ReqwestError),

    /// An I/O error
    #[fail(display = "An I/O error occured")]
    Io(#[cause] IoError),

    /// A UTF8 error
    #[fail(display = "A UTF8 error occured")]
    Utf8(#[cause] Utf8Error),

    /// An r2d2 error
    #[fail(display = "An r2d2 error occured")]
    R2d2,

    /// Reached download limit error
    #[fail(display = "Reached download limit of {} KiB", limit)]
    DownloadLimit { limit: usize },
}

/// Errors related to plugins
#[derive(Debug, Fail)]
pub enum PluginError {
    /// A Url error
    #[fail(display = "A Url error occured")]
    Url(#[cause] UrlError),

    /// A Factoids error
    #[fail(display = "{}", error)]
    Factoids { error: String },
}

/// A URL plugin error
#[derive(Debug, Fail)]
pub enum UrlError {
    /// Missing URL error
    #[fail(display = "No URL was found")]
    MissingUrl,

    /// Missing title error
    #[fail(display = "No title was found")]
    MissingTitle,
}

impl From<UrlError> for FrippyError {
    fn from(e: UrlError) -> FrippyError {
        FrippyError::Plugin(PluginError::Url(e))
    }
}

impl From<PluginError> for FrippyError {
    fn from(e: PluginError) -> FrippyError {
        FrippyError::Plugin(e)
    }
}

impl From<IrcError> for FrippyError {
    fn from(e: IrcError) -> FrippyError {
        FrippyError::Irc(e)
    }
}

impl From<ReqwestError> for FrippyError {
    fn from(e: ReqwestError) -> FrippyError {
        FrippyError::Reqwest(e)
    }
}

impl From<IoError> for FrippyError {
    fn from(e: IoError) -> FrippyError {
        FrippyError::Io(e)
    }
}

impl From<Utf8Error> for FrippyError {
    fn from(e: Utf8Error) -> FrippyError {
        FrippyError::Utf8(e)
    }
}

impl From<R2d2Error> for FrippyError {
    fn from(e: R2d2Error) -> FrippyError {
        FrippyError::R2d2(e)
    }
}
