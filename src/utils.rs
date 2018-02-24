extern crate reqwest;

use std::str;
use std::io::{self, Read};

use self::reqwest::Client;
use self::reqwest::header::Connection;

pub fn download(max_kib: usize, url: &str) -> Option<String> {
    let response = Client::new().get(url).header(Connection::close()).send();

    match response {
        Ok(mut response) => {
            let mut body = String::new();

            // 500 kilobyte buffer
            let mut buf = [0; 500 * 1000];
            let mut written = 0;
            // Read until we reach EOF or max_kib KiB
            loop {
                let len = match response.read(&mut buf) {
                    Ok(0) => break,
                    Ok(len) => len,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        debug!("Download from {:?} failed: {}", url, e);
                        return None;
                    }
                };

                let slice = match str::from_utf8(&buf[..len]) {
                    Ok(slice) => slice,
                    Err(e) => {
                        debug!("Failed to read bytes from {:?} as UTF8: {}", url, e);
                        return None;
                    }
                };

                body.push_str(slice);
                written += len;

                // Check if the file is too large to download
                if written > max_kib * 1024 {
                    debug!(
                        "Stopping download - File from {:?} is larger than {} KiB",
                        url, max_kib
                    );
                    return None;
                }
            }
            Some(body)
        }
        Err(e) => {
            debug!("Bad response from {:?}: ({})", url, e);
            None
        }
    }
}
