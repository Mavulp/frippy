extern crate regex;
extern crate select;

use irc::client::prelude::*;
use irc::error::IrcError;

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
        Url { max_kib: max_kib }
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

    fn url(&self, text: &str) -> Result<String, &str> {
        let url = match self.grep_url(text) {
            Some(url) => url,
            None => return Err("No Url was found."),
        };


        match utils::download(self.max_kib, &url) {
            Some(body) => {
                let doc = Document::from(body.as_ref());
                if let Some(title) = doc.find(Name("title")).next() {
                    let title = title.children().next().unwrap();
                    let title_text = title.as_text().unwrap().trim().replace("\n", "|");
                    debug!("Title: {:?}", title);
                    debug!("Text: {:?}", title_text);

                    Ok(title_text)
                } else {
                    Err("No title was found.")
                }
            }
            None => Err("Failed to download document."),
        }
    }
}

impl Plugin for Url {
    fn execute(&self, _: &IrcClient, message: &Message) -> ExecutionStatus {
        match message.command {
            Command::PRIVMSG(_, ref msg) => if RE.is_match(msg) {
                ExecutionStatus::RequiresThread
            } else {
                ExecutionStatus::Done
            },
            _ => ExecutionStatus::Done,
        }
    }

    fn execute_threaded(&self, client: &IrcClient, message: &Message) -> Result<(), IrcError> {
        match message.command {
            Command::PRIVMSG(_, ref content) => match self.url(content) {
                Ok(title) => client.send_privmsg(message.response_target().unwrap(), &title),
                Err(_) => Ok(()),
            },
            _ => Ok(()),
        }
    }

    fn command(&self, client: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        client.send_notice(
            &command.source,
            "This Plugin does not implement any commands.",
        )
    }

    fn evaluate(&self, _: &IrcClient, command: PluginCommand) -> Result<String, String> {
        self.url(&command.tokens[0]).map_err(String::from)
    }
}

#[cfg(test)]
mod tests {}
