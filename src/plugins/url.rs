extern crate regex;
extern crate reqwest;
extern crate select;

use irc::client::prelude::*;
use irc::error::Error as IrcError;

use self::regex::Regex;

use std::str;
use std::io::{self, Read};
use self::reqwest::Client;
use self::reqwest::header::Connection;

use self::select::document::Document;
use self::select::predicate::Name;

use plugin::*;

lazy_static! {
    static ref RE: Regex = Regex::new(r"(^|\s)(https?://\S+)").unwrap();
}

#[derive(PluginName, Debug)]
pub struct Url {
    max_kib: usize,
}

impl Url {
    /// If a file is larger than `max_kib` KiB the download is stopped
    pub fn new(max_kib: usize) -> Url {
        Url {max_kib: max_kib}
    }

    fn grep_url(&self, msg: &str) -> Option<String> {
        match RE.captures(msg) {
            Some(captures) => {
                debug!("Url captures: {:?}", captures);

                Some(captures.get(2).unwrap().as_str().to_string())
            }
            None => None,
        }
    }

    fn download(&self, url: &str) -> Option<String> {
        let response = Client::new()
            .get(url)
            .header(Connection::close())
            .send();

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
                    if written > self.max_kib * 1024 {
                        debug!("Stopping download - File from {:?} is larger than {} KiB", url, self.max_kib);
                        return None;
                    }

                }
                Some(body) // once told me
            }
            Err(e) => {
                debug!("Bad response from {:?}: ({})", url, e);
                return None;
            }
        }
    }

    fn url(&self, server: &IrcServer, message: &str, target: &str) -> Result<(), IrcError> {
        let url = match self.grep_url(message) {
            Some(url) => url,
            None => {
                return Ok(());
            }
        };


        match self.download(&url) {
            Some(body) => {

                let doc = Document::from(body.as_ref());
                if let Some(title) = doc.find(Name("title")).next() {
                    let text = title.children().next().unwrap();
                    let message = text.as_text().unwrap().trim().replace("\n", "|");
                    debug!("Title: {:?}", text);
                    debug!("Message: {:?}", message);

                    server.send_privmsg(target, &message)

                } else {
                    Ok(())
                }
            }
            None => Ok(()),
        }
    }
}

impl Plugin for Url {
    fn is_allowed(&self, _: &IrcServer, message: &Message) -> bool {
        match message.command {
            Command::PRIVMSG(_, ref msg) => RE.is_match(msg),
            _ => false,
        }
    }

    fn execute(&self, server: &IrcServer, message: &Message) -> Result<(), IrcError> {
        match message.command {
            Command::PRIVMSG(_, ref content) => {
                self.url(server, content, message.response_target().unwrap())
            }
            _ => Ok(()),
        }
    }

    fn command(&self, server: &IrcServer, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source,
                           "This Plugin does not implement any commands.")
    }
}

#[cfg(test)]
mod tests {}
