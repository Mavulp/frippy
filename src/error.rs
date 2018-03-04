//! Errors for `frippy` crate using `failure`.

use failure::Fail;

pub fn log_error(e: FrippyError) {
    let text = e.causes()
        .skip(1)
        .fold(format!("{}", e), |acc, err| format!("{}: {}", acc, err));
    error!("{}", text);
}

/// The main crate-wide error type.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
#[error = "FrippyError"]
pub enum ErrorKind {
    /// Connection error
    #[fail(display = "A connection error occured")]
    Connection,

    /// A Url error
    #[fail(display = "A Url error has occured")]
    Url,

    /// A Tell error
    #[fail(display = "A Tell error has occured")]
    Tell,

    /// A Factoids error
    #[fail(display = "A Factoids error has occured")]
    Factoids,
}
