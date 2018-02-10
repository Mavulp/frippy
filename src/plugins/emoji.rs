extern crate unicode_names;

use std::fmt;

use irc::client::prelude::*;
use irc::error::IrcError;

use plugin::*;

struct EmojiHandle {
    symbol: char,
    count: i32,
}

impl fmt::Display for EmojiHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        let name = match unicode_names::name(self.symbol) {
            Some(sym) => sym.to_string().to_lowercase(),
            None => String::from("UNKNOWN"),
        };

        if self.count > 1 {
            write!(f, "{}x {}", self.count, name)
        } else {
            write!(f, "{}", name)
        }
    }
}

#[derive(PluginName, Default, Debug)]
pub struct Emoji;

impl Emoji {
    pub fn new() -> Emoji {
        Emoji {}
    }

    fn emoji(&self, content: &str) -> String {
        self.return_emojis(content)
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }

    fn return_emojis(&self, string: &str) -> Vec<EmojiHandle> {
        let mut emojis: Vec<EmojiHandle> = Vec::new();

        let mut current = EmojiHandle {
            symbol: ' ',
            count: 0,
        };


        for c in string.chars() {
            if !self.is_emoji(&c) {
                continue;
            }

            if current.symbol == c {
                current.count += 1;

            } else {
                if current.count > 0 {
                    emojis.push(current);
                }

                current = EmojiHandle {
                    symbol: c,
                    count: 1,
                }
            }
        }

        if current.count > 0 {
            emojis.push(current);
        }

        emojis
    }

    fn is_emoji(&self, c: &char) -> bool {
        // Emoji ranges from stackoverflow:
        // https://stackoverflow.com/questions/30757193/find-out-if-character-in-string-is-emoji
        match *c { '\u{1F600}'...'\u{1F64F}'     // Emoticons
            | '\u{1F300}'...'\u{1F5FF}'          // Misc Symbols and Pictographs
            | '\u{1F680}'...'\u{1F6FF}'          // Transport and Map
            | '\u{2600}' ...'\u{26FF}'           // Misc symbols
            | '\u{2700}' ...'\u{27BF}'           // Dingbats
            | '\u{FE00}' ...'\u{FE0F}'           // Variation Selectors
            | '\u{1F900}'...'\u{1F9FF}'          // Supplemental Symbols and Pictographs
            | '\u{20D0}' ...'\u{20FF}' => true,  // Combining Diacritical Marks for Symbols
            _ => false,
        }
    }
}

impl Plugin for Emoji {
    fn is_allowed(&self, _: &IrcClient, message: &Message) -> bool {
        match message.command {
            Command::PRIVMSG(_, _) => true,
            _ => false,
        }
    }

    fn execute(&self, server: &IrcClient, message: &Message) -> Result<(), IrcError> {
        match message.command {
            Command::PRIVMSG(_, ref content) => {
                server.send_privmsg(message.response_target().unwrap(),
                                    &self.emoji(content))
            }
            _ => Ok(()),
        }
    }

    fn command(&self, server: &IrcClient, command: PluginCommand) -> Result<(), IrcError> {
        server.send_notice(&command.source,
                           "This Plugin does not implement any commands.")
    }

    fn evaluate(&self, _: &IrcClient, command: PluginCommand) -> Result<String, String> {
        let emojis = self.emoji(&command.tokens[0]);
        if emojis.is_empty() {
            Ok(emojis)
        } else {
            Err(String::from("No emojis were found."))
        }
    }
}

#[cfg(test)]
mod tests {}
