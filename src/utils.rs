use std::borrow::Cow;
use std::io::{self, Read};
use std::time::Duration;

use reqwest::header::{HeaderValue, ACCEPT_LANGUAGE, CONNECTION};
use reqwest::{Client, ClientBuilder};

use self::error::{DownloadError, ErrorKind};
use failure::ResultExt;

#[derive(Clone, Debug)]
pub struct Url<'a> {
    url: Cow<'a, str>,
    max_kib: Option<usize>,
    timeout: Option<Duration>,
}

impl<'a> From<String> for Url<'a> {
    fn from(url: String) -> Self {
        Url {
            url: Cow::from(url),
            max_kib: None,
            timeout: None,
        }
    }
}

impl<'a> From<&'a str> for Url<'a> {
    fn from(url: &'a str) -> Self {
        Url {
            url: Cow::from(url),
            max_kib: None,
            timeout: None,
        }
    }
}

impl<'a> Url<'a> {
    pub fn max_kib(mut self, limit: usize) -> Self {
        self.max_kib = Some(limit);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Downloads the file and converts it to a String.
    /// Any invalid bytes are converted to a replacement character.
    ///
    /// The error indicated either a failed download or
    /// that the limit set by max_kib() was reached.
    pub fn request(&self) -> Result<String, DownloadError> {
        let client = if let Some(timeout) = self.timeout {
            ClientBuilder::new().timeout(timeout).build().unwrap()
        } else {
            Client::new()
        };

        let mut response = client
            .get(self.url.as_ref())
            .header(CONNECTION, HeaderValue::from_static("close"))
            .header(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"))
            .send()
            .context(ErrorKind::Connection)?;

        // 100 kibibyte buffer
        let mut buf = [0; 100 * 1024];
        let mut written = 0;
        let mut bytes = Vec::new();

        // Read until we reach EOF or max_kib KiB
        loop {
            let len = match response.read(&mut buf) {
                Ok(0) => break,
                Ok(len) => len,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => Err(e).context(ErrorKind::Read)?,
            };

            bytes.extend_from_slice(&buf[..len]);
            written += len;

            // Check if the file is too large to download
            if let Some(max_kib) = self.max_kib {
                if written > max_kib * 1024 {
                    Err(ErrorKind::DownloadLimit)?;
                }
            }
        }

        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }
}

pub mod error {
    use failure::Fail;
    use frippy_derive::Error;

    #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail, Error)]
    #[error = "DownloadError"]
    pub enum ErrorKind {
        /// Connection Error
        #[fail(display = "A connection error has occured")]
        Connection,

        /// Read Error
        #[fail(display = "A read error has occured")]
        Read,

        /// Reached download limit error
        #[fail(display = "Reached download limit")]
        DownloadLimit,
    }
}
