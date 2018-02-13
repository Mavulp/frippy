extern crate regex;
extern crate select;

use irc::client::prelude::*;
use irc::error::Error as IrcError;

use self::regex::Regex;

use self::select::document::Document;
use self::select::predicate::Name;

use plugin::*;
use utils;

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

    fn url(&self, server: &IrcServer, message: &str, target: &str) -> Result<(), IrcError> {
        let url = match self.grep_url(message) {
            Some(url) => url,
            None => {
                return Ok(());
            }
        };


        match utils::download(self.max_kib, &url) {
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
