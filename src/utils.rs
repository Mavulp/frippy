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
                    Err(e) => {
                        debug!("Download from {:?} failed: {}", url, e);
                        return None;
                    }
                };
                vec.extend_from_slice(&buf);

                end_of_valid = None;
                let body_slice = match str::from_utf8(&vec[..len]) {
                    Ok(slice) => slice,
                    Err(e) => {
                        let valid = e.valid_up_to();
                        if valid == 0 {
                            error!("Failed to read bytes from {:?} as UTF8: {}", url, e);
                            return None;
                        }
                        end_of_valid = Some(valid);

                        str::from_utf8(&buf[..valid]).unwrap()
                    }
                };

                body.push_str(body_slice);
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
