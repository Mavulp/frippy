use std::str;
use std::io::{self, Read};

use reqwest::Client;
use reqwest::header::Connection;

use failure::ResultExt;
use self::error::{DownloadError, ErrorKind};

/// Downloads the file and converts it to a String.
/// Any invalid bytes are converted to a replacement character.
///
/// The error indicated either a failed download or that the DownloadLimit was reached
pub fn download(url: &str, max_kib: Option<usize>) -> Result<String, DownloadError> {
    let mut response = Client::new()
        .get(url)
        .header(Connection::close())
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
        if let Some(max_kib) = max_kib {
            if written > max_kib * 1024 {
                Err(ErrorKind::DownloadLimit)?;
            }
        }
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub mod error {
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
