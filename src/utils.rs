use std::str;
use std::io::{self, Read};

use reqwest::Client;
use reqwest::header::Connection;

use error::FrippyError;

pub fn download(max_kib: usize, url: &str) -> Result<String, FrippyError> {
    let mut response = Client::new().get(url).header(Connection::close()).send()?;

    let mut body = String::new();

    // 100 kibibyte buffer
    let mut buf = [0; 100 * 1024];
    let mut written = 0;
    let mut vec = Vec::new();
    let mut end_of_valid = None;

    // Read until we reach EOF or max_kib KiB
    loop {
        if let Some(eov) = end_of_valid {
            vec = vec[..eov].to_vec();
        }

        let len = match response.read(&mut buf) {
            Ok(0) => break,
            Ok(len) => len,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => Err(e)?,
        };
        vec.extend_from_slice(&buf);

        end_of_valid = None;
        let body_slice = match str::from_utf8(&vec[..len]) {
            Ok(slice) => slice,
            Err(e) => {
                let valid = e.valid_up_to();
                if valid == 0 {
                    Err(e)?;
                }
                end_of_valid = Some(valid);

                str::from_utf8(&buf[..valid])?
            }
        };

        body.push_str(body_slice);
        written += len;

        // Check if the file is too large to download
        if written > max_kib * 1024 {
            Err(FrippyError::DownloadLimit { limit: max_kib })?;
        }
    }

    Ok(body)
}
