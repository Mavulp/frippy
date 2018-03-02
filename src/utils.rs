use std::str;
use std::io::{self, Read};

use reqwest::Client;
use reqwest::header::Connection;

use failure::Fail;
use error::FrippyError;

/// Downloads the file and converts it to a String.
/// Any invalid bytes are converted to a replacement character.
///
/// The error indicated either a failed download or that the DownloadLimit was reached
pub fn download(url: &str, max_kib: Option<usize>) -> Result<String, FrippyError> {
    let mut response = Client::new().get(url).header(Connection::close()).send()?;

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
            Err(e) => Err(e)?,
        };

        bytes.extend_from_slice(&buf);
        written += len;

        // Check if the file is too large to download
        if let Some(max_kib) = max_kib {
            if written > max_kib * 1024 {
                Err(FrippyError::DownloadLimit { limit: max_kib })?;
            }
        }
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}


pub fn log_error(e: FrippyError) {
    let mut causes = e.causes();

    error!("{}", causes.next().unwrap());
    for cause in causes {
        error!("caused by: {}", cause);
    }
}
