use std::str;
use std::io::{self, Read};

use reqwest::Client;
use reqwest::header::Connection;

use error::FrippyError;

pub fn download(max_kib: usize, url: &str) -> Result<String, FrippyError> {
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
        if written > max_kib * 1024 {
            Err(FrippyError::DownloadLimit { limit: max_kib })?;
        }
    }
    let body = str::from_utf8(&bytes)?;

    Ok(body.to_string())
}
